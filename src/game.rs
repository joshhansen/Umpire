//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

pub mod ai;
pub mod city;
pub mod combat;
pub mod map;
pub mod move_;
pub mod obs;
pub mod unit;

use core::cell::RefCell;

use std::{
    collections::{BTreeSet,HashMap,HashSet},
    fmt,
    rc::Rc,
};

use crate::{
    color::{Colors,Colorized},
    game::{
        city::{CityID,City},
        combat::{CombatCapable},
        map::{
            MapData,
            NewUnitError,
            Tile,
            gen::generate_map,
            dijkstra::{
                AndFilter,
                NoCitiesButOursFilter,
                NoUnitsFilter,
                OverlaySource,
                ShortestPaths,
                Source,
                SourceMut,
                UnitMovementFilter,
                neighbors_terrain_only,
                shortest_paths
            },
        },
        obs::{Obs,Observer,ObsTracker,ObsTrackerI,OverlayObsTracker},
        unit::{
            UnitID,Unit,UnitType,
            orders::{
                Orders,
                OrdersError,
                OrdersStatus,
                OrdersOutcome,
                OrdersResult,
                ProposedOrdersResult,
                ProposedSetAndFollowOrders,
            },
        },
    },
    name::{Namer,ListNamer},
    util::{Dims,Dimensioned,Location,Wrap2d},
};

use self::move_::{
    MoveComponent,
    MoveError,
    MoveResult,
    ProposedMove,
    ProposedMoveResult,
};

/// A trait for types which are contemplated-but-not-carried-out actions. Associated type `Outcome` will result from carrying out the proposed action.
#[must_use = "All proposed actions issued by the game engine must be taken using `take`"]
pub trait ProposedAction {
    /// The result of carrying out the proposed action
    type Outcome;

    /// Carry out the proposed action
    fn take(self, game: &mut Game) -> Self::Outcome;
}

pub type TurnNum = u32;

pub type PlayerNum = u8;

#[derive(Copy,Clone,Debug,PartialEq,Hash,Eq)]
pub enum Alignment {
    Neutral,
    Belligerent { player: PlayerNum }
    // active neutral, chaotic, etc.
}

impl Colorized for Alignment {
    fn color(&self) -> Option<Colors> {
        Some(match self {
            Alignment::Neutral => Colors::Neutral,
            Alignment::Belligerent{player} => Colors::Player(*player),
        })
    }
}

impl fmt::Display for Alignment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Alignment::Neutral => write!(f, "Neutral"),
            Alignment::Belligerent{player} => write!(f, "Player {}", player),
        }
    }
}

pub trait Aligned : AlignedMaybe {
    fn alignment(&self) -> Alignment;
}

pub trait AlignedMaybe {
    fn alignment_maybe(&self) -> Option<Alignment>;

    fn belongs_to_player(&self, player: PlayerNum) -> bool {
        if let Some(alignment) = self.alignment_maybe() {
            if let Alignment::Belligerent{player:player_} = alignment {
                player==player_
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl <T:Aligned> AlignedMaybe for T {
    fn alignment_maybe(&self) -> Option<Alignment> {
        Some(self.alignment())
    }
}

#[must_use]
#[derive(Debug,PartialEq)]
pub struct ProposedTurnStart {
    pub turn: TurnNum,
    pub current_player: PlayerNum,
    pub proposed_orders_results: Vec<ProposedOrdersResult>,
    pub production_outcomes: Vec<UnitProductionOutcome>,
}

#[derive(Debug,PartialEq)]
pub struct TurnStart {
    pub turn: TurnNum,
    pub current_player: PlayerNum,
    pub orders_results: Vec<OrdersResult>,
    pub production_outcomes: Vec<UnitProductionOutcome>,
}

impl ProposedAction for ProposedTurnStart {
    type Outcome = TurnStart;

    fn take(mut self, game: &mut Game) -> Self::Outcome {
        TurnStart {
            turn: self.turn,
            current_player: self.current_player,
            orders_results: self.proposed_orders_results.drain(..).map(|proposed_orders_result| {
                proposed_orders_result.map(|proposed_orders| proposed_orders.take(game))
            }).collect(),
            production_outcomes: self.production_outcomes,
        }
    }
}

#[derive(Debug)]
pub enum GameError {
    NoSuchUnit { msg: String, id: UnitID },
    NoUnitAtLocation { msg: String, loc: Location },
    NoSuchCity { msg: String, id: CityID },
    NoCityAtLocation { msg: String, loc: Location },
    UnitNotControlledByCurrentPlayer { msg: String }
}

#[derive(Debug,PartialEq)]
pub enum UnitProductionOutcome {
    UnitProduced {
        /// A copy of the unit that was produced
        unit: Unit,

        /// A copy of the city that did the producing
        city: City,
    },

    UnitAlreadyPresent {
        /// A copy of the unit that's already present
        prior_unit: Unit,

        /// The type of unit the city is trying to produce
        unit_type_under_production: UnitType,

        /// A copy of the city that would have produced the unit if a unit weren't already present
        city: City,
    }
}

#[derive(Clone)]
pub struct Game {
    map: MapData,
    player_observations: HashMap<PlayerNum,ObsTracker>,
    turn: TurnNum,
    num_players: PlayerNum,
    current_player: PlayerNum,
    wrapping: Wrap2d,
    unit_namer: Rc<RefCell<dyn Namer>>,
    fog_of_war: bool,
}
impl Game {
    /// Creates a new game instance
    ///
    /// The Game that is returned will already have begun with the first player's turn
    /// A map with the specified dimensions will be generated
    /// If `fog_of_war` is `true` then players' view of the map will be limited to what they have previously
    /// observed, with observations growing stale over time.
    pub fn new(
            map_dims: Dims,
            mut city_namer: ListNamer,
            num_players: PlayerNum,
            fog_of_war: bool,
            unit_namer: Rc<RefCell<dyn Namer>>,
            wrapping: Wrap2d) -> Self {

        let map = generate_map(&mut city_namer, map_dims, num_players);
        Game::new_with_map(map, num_players, fog_of_war, unit_namer, wrapping)
    }

    pub(crate) fn new_with_map(map: MapData, num_players: PlayerNum,
            fog_of_war: bool, unit_namer: Rc<RefCell<dyn Namer>>,
            wrapping: Wrap2d) -> Self {

        let mut player_observations = HashMap::new();
        for player_num in 0..num_players {
            player_observations.insert(player_num, ObsTracker::new(map.dims()));
        }

        // log.log_message(format!("Starting new game with {} players, grid size {}, and fog of war {}",
        //                         num_players,
        //                         map.dims(),
        //                         if fog_of_war {"on"} else {"off"}
        // ));

        let mut game = Game {
            map,
            player_observations,
            turn: 0,
            num_players,
            current_player: 0,
            wrapping,
            unit_namer,
            fog_of_war,
        };

        game.begin_turn();
        game
    }

    fn produce_units(&mut self) -> Vec<UnitProductionOutcome> {
        for city in self.current_player_cities_with_production_target_mut() {
            city.production_progress += 1;
        }

        let producing_city_locs: Vec<Location> = self.current_player_cities_with_production_target()
            .filter(|city| {
                let unit_under_production = city.production().unwrap();

                city.production_progress >= unit_under_production.cost()
            })
            .map(|city| city.loc).collect()
        ;

        producing_city_locs.iter().cloned().map(|city_loc| {

            let (city_loc, city_alignment, unit_under_production) = {
                let city = self.map.city_by_loc_mut(city_loc).unwrap();
                let unit_under_production = city.production().unwrap();
                (city.loc, city.alignment, unit_under_production)
            };

            let name = self.unit_namer.borrow_mut().name();

            // Attempt to create the new unit

            let result = self.map.new_unit(city_loc, unit_under_production, city_alignment, name);

            match result {
                Ok(_new_unit_id) => {
                    // We know the unit will be at top-level because that's where freshly-minted units go
                    
                    let city = self.map.city_by_loc_mut(city_loc).unwrap();
                    city.production_progress = 0;

                    let city = city.clone();
                    let unit = self.map.toplevel_unit_by_loc(city_loc).unwrap().clone();

                    UnitProductionOutcome::UnitProduced {
                        city, unit,
                    }
                },
                Err(err) => match err {
                    NewUnitError::OutOfBounds{ loc, dims } => {
                        panic!(format!("Attempted to create a unit at {} outside the bounds {}", loc, dims))
                    },
                    NewUnitError::UnitAlreadyPresent{ prior_unit, unit_type_under_production, .. } => {
                        let city = self.map.city_by_loc(city_loc).unwrap();

                        UnitProductionOutcome::UnitAlreadyPresent {
                            prior_unit, unit_type_under_production, city: city.clone(),
                        }
                    }
                }
            }
        }).collect()
    }

    fn refresh_moves_remaining(&mut self) {
        self.current_player_units_deep_mutate(|unit: &mut Unit| unit.refresh_moves_remaining());
    }

    /// Begin a new turn
    /// 
    /// Returns the results of any pending orders carried out
    fn begin_turn(&mut self) -> TurnStart {
        self.propose_begin_turn().take(self)
    }

    /// Begin a new turn, but only simulate the orders following. These can be made real using `ProposedAction::take`.
    fn propose_begin_turn(&mut self) -> ProposedTurnStart {
        let production_outcomes = self.produce_units();

        self.refresh_moves_remaining();

        self.update_current_player_observations();

        let proposed_orders_results = self.propose_following_pending_orders();

        ProposedTurnStart {
            turn: self.turn,
            current_player: self.current_player,
            proposed_orders_results,
            production_outcomes,
        }
    }

    pub fn turn_is_done(&self) -> bool {
        self.production_set_requests().next().is_none() && self.unit_orders_requests().next().is_none()
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    /// 
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    pub fn victor(&self) -> Option<PlayerNum> {
        let mut possible: HashSet<PlayerNum> = (0..self.num_players).collect();

        for city in self.map.cities() {
            if let Alignment::Belligerent{player} = city.alignment {
                possible.remove(&player);
            }
            if possible.len() == 0 {
                return None;
            }
        }

        for unit in self.map.units() {
            if let Alignment::Belligerent{player} = unit.alignment {
                possible.remove(&player);
            }
            if possible.len() == 0 {
                return None;
            }
        }

        if possible.len() == 1 {
            return Some(*possible.iter().next().unwrap());// unwrap to assert something's there
        }

        None
    }

    /// End the current player's turn and begin the next player's turn
    ///
    /// Returns the number of the now-current player.
    /// Ok if the turn ended properly
    /// Err if not
    ///
    /// If the requirements for ending the turn weren't met, it will remain the turn of the player that was playing
    /// when this method was called.
    ///
    /// If the requirements for ending the turn were met the next player's turn will begin
    ///
    /// At the beginning of a turn, new units will be created as necessary, production counts will be reset as
    /// necessary, and production and movement requests will be created as necessary.
    ///
    /// At the end of a turn, production counts will be incremented.
    pub fn end_turn(&mut self) -> Result<TurnStart,PlayerNum> {
        if self.turn_is_done() {
            self.player_observations.get_mut(&self.current_player()).unwrap().archive();

            self.current_player = (self.current_player + 1) % self.num_players;
            if self.current_player == 0 {
                self.turn += 1;
            }

            Ok(self.begin_turn())
        } else {
            Err(self.current_player)
        }
    }

    pub fn propose_end_turn(&mut self) -> Result<ProposedTurnStart,PlayerNum> {
        if self.turn_is_done() {
            self.player_observations.get_mut(&self.current_player()).unwrap().archive();

            self.current_player = (self.current_player + 1) % self.num_players;
            if self.current_player == 0 {
                self.turn += 1;
            }

            Ok(self.propose_begin_turn())
        } else {
            Err(self.current_player)
        }
    }

    /// Register the current observations of current player units
    /// 
    /// This applies only to top-level units. Carried units (e.g. units in a transport or carrier) make no observations
    fn update_current_player_observations(&mut self) {
        let obs_tracker = self.player_observations.get_mut(&self.current_player).unwrap();

        for loc in self.map.dims().iter_locs() {
            let tile = self.map.tile(loc).unwrap();

            if self.fog_of_war {

                // With "fog of war" we only get updated observations where there are units and cities in the player's control
                
                if let Some(ref city) = tile.city {
                    if let Alignment::Belligerent{player} = city.alignment {
                        if player==self.current_player {
                            city.observe(&self.map, self.turn, self.wrapping, obs_tracker);
                        }
                    }
                }

                if let Some(ref unit) = tile.unit {
                    if let Alignment::Belligerent{player} = unit.alignment {
                        if player==self.current_player {
                            unit.observe(&self.map, self.turn, self.wrapping, obs_tracker);
                        }
                    }
                }

            } else {
                // Without "fog of war" we get updated observations everywhere

                obs_tracker.observe(loc, tile, self.turn);
            }
        }
    }

    /// The current player's most recent observation of the tile at location `loc`, if any
    pub fn current_player_tile(&self, loc: Location) -> Option<&Tile> {
        if let Obs::Observed{tile,..} = self.current_player_obs(loc) {
            Some(tile)
        } else {
            None
        }
    }

    /// The current player's observation at location `loc`
    pub fn current_player_obs(&self, loc: Location) -> &Obs {
        self.player_observations[&self.current_player()].get(loc)
    }

    pub fn current_player_observations(&self) -> &ObsTracker {
        self.player_observations.get(&self.current_player).unwrap()
    }

    // fn current_player_observations_mut(&mut self) -> &mut ObsTracker {
    //     self.player_observations.get_mut(&self.current_player).unwrap()
    // }

    /// Every city controlled by the current player
    pub fn current_player_cities(&self) -> impl Iterator<Item=&City> {
        self.map.player_cities(self.current_player)
    }

    /// All cities controlled by the current player which have a production target set
    pub fn current_player_cities_with_production_target(&self) -> impl Iterator<Item=&City> {
        self.map.player_cities_with_production_target(self.current_player)
    }

    /// All cities controlled by the current player which have a production target set, mutably
    fn current_player_cities_with_production_target_mut(&mut self) -> impl Iterator<Item=&mut City> {
        self.map.player_cities_with_production_target_mut(self.current_player)
    }

    /// Mutate all units controlled by the current player according to the callback `callback`
    fn current_player_units_deep_mutate<F:FnMut(&mut Unit)>(&mut self, callback: F) {
        self.map.player_units_deep_mutate(self.current_player(), callback);
    }

    /// The number of cities controlled by the current player which either have a production target or are NOT set to be ignored when requesting productions to be set
    /// 
    /// This basically lets us make sure a player doesn't set all their cities' productions to none since right now the UI has no way of getting out of that situation
    /// 
    /// FIXME Get rid of this and just make the UI smarter
    #[deprecated]
    pub fn player_cities_producing_or_not_ignored(&self) -> usize {
        self.current_player_cities().filter(|city| city.production().is_some() || !city.ignore_cleared_production()).count()
    }

    /// Every unit controlled by the current player
    pub fn current_player_units(&self) -> impl Iterator<Item=&Unit> {
        self.map.player_units(self.current_player)
    }

    /// Every unit controlled by the current player, mutably
    fn current_player_units_mut(&mut self) -> impl Iterator<Item=&mut Unit> {
        self.map.player_units_mut(self.current_player)
    }

    /// If the current player controls a city at location `loc`, return it
    pub fn current_player_city_by_loc(&self, loc: Location) -> Option<&City> {
        self.current_player_tile(loc).and_then(|tile| tile.city.as_ref())
    }

    /// If the current player controls a unit with ID `id`, return it
    pub fn current_player_unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.current_player_units().find(|unit| unit.id==id)
    }

    /// If the current player controls a unit with ID `id`, return it mutably
    fn current_player_unit_by_id_mut(&mut self, id: UnitID) -> Option<&mut Unit> {
        self.current_player_units_mut().find(|unit| unit.id==id)
    }

    /// If the current player controls a unit with ID `id`, return its location
    pub fn current_player_unit_loc(&self, id: UnitID) -> Option<Location> {
        self.current_player_unit_by_id(id).map(|unit| unit.loc)
    }

    /// If the current player controls the top-level unit at location `loc`, return it
    pub fn current_player_toplevel_unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        self.current_player_tile(loc).and_then(|tile| tile.unit.as_ref())
    }

    /// If the current player controls the top-level unit at location `loc`, return it mutably
    fn current_player_toplevel_unit_by_loc_mut(&mut self, loc: Location) -> Option<&mut Unit> {
        if self.current_player_toplevel_unit_by_loc(loc).is_some() {
            self.map.toplevel_unit_by_loc_mut(loc)
        } else {
            None
        }
        // self.current_player_tile_mut(loc).and_then(|tile| tile.unit.as_ref())
    }

    pub fn production_set_requests<'a>(&'a self) -> impl Iterator<Item=Location> + 'a {
        self.map.player_cities_lacking_production_target(self.current_player).map(|city| city.loc)
    }

    /// Which if the current player's units need orders?
    /// 
    /// In other words, which of the current player's units have no orders and have moves remaining?
    pub fn unit_orders_requests<'a>(&'a self) -> impl Iterator<Item=UnitID> + 'a {
        self.map.player_units_deep(self.current_player)
            .filter(|unit| unit.orders.is_none() && unit.moves_remaining() > 0)
            .map(|unit| unit.id)
    }

    pub fn units_with_pending_orders<'a>(&'a self) -> impl Iterator<Item=UnitID> + 'a {
        self.current_player_units()
            .filter(|unit| unit.moves_remaining() > 0 && unit.orders.is_some() && *unit.orders.as_ref().unwrap() != Orders::Sentry)
            .map(|unit| unit.id)
    }


    // Movement-related methods

    pub fn move_toplevel_unit_by_id(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        let src = self.map.unit_loc(unit_id).unwrap();
        self.move_toplevel_unit_by_loc(src, dest)
    }

    pub fn move_toplevel_unit_by_id_avoiding_combat(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        let src = self.map.unit_loc(unit_id).unwrap();
        self.move_toplevel_unit_by_loc_avoiding_combat(src, dest)
    }

    /*
        Errors:
        * If unit at `src` doesn't exist
        * If requested move requires more moves than the unit has remaining
        * If `dest` is unreachable from `src` (may be subsumed by previous)

        FIXME Make the unit observe at each point along its path

        FIXME This function checks two separate times whether a unit exists at src
    */
    pub fn move_toplevel_unit_by_loc(&mut self, src: Location, dest: Location) -> MoveResult {
        let shortest_paths = {
            let unit = self.map.toplevel_unit_by_loc(src).unwrap();
            shortest_paths(&self.map, src, &UnitMovementFilter::new(unit), self.wrapping)
        };
        self.move_toplevel_unit_by_loc_following_shortest_paths(src, dest, shortest_paths)
    }

    pub fn move_toplevel_unit_by_loc_avoiding_combat(&mut self, src: Location, dest: Location) -> MoveResult {
        let shortest_paths = {
            let unit = self.map.toplevel_unit_by_loc(src).unwrap();
                let unit_filter = AndFilter::new(
                    AndFilter::new(
                        NoUnitsFilter{},
                        NoCitiesButOursFilter{alignment: unit.alignment }
                    ),
                    UnitMovementFilter{unit}
                );
            shortest_paths(&self.map, src, &unit_filter, self.wrapping)
        };
        self.move_toplevel_unit_by_loc_following_shortest_paths(src, dest, shortest_paths)
    }

    fn move_toplevel_unit_by_loc_following_shortest_paths(&mut self, src: Location, dest: Location, shortest_paths: ShortestPaths) -> MoveResult {
        self.propose_move_toplevel_unit_by_loc_following_shortest_paths(src, dest, shortest_paths).map(|proposed_move| {
            proposed_move.take(self)
        })
    }

    fn propose_move_toplevel_unit_by_loc_following_shortest_paths(&self, src: Location, dest: Location, shortest_paths: ShortestPaths) -> ProposedMoveResult {
        let id: UnitID = self.map.toplevel_unit_id_by_loc(src).unwrap();
        self.propose_move_unit_by_loc_and_id_following_shortest_paths(src, id, dest, shortest_paths)
    }

    pub fn move_unit_by_id(&mut self, id: UnitID, dest: Location) -> MoveResult {
        self.propose_move_unit_by_id(id, dest).map(|proposed_move| proposed_move.take(self))
    }

    pub fn propose_move_unit_by_id(&self, id: UnitID, dest: Location) -> ProposedMoveResult {
        let (shortest_paths, src) = {
            let unit = self.map.unit_by_id(id).unwrap();
            (shortest_paths(&self.map, unit.loc, &UnitMovementFilter::new(unit), self.wrapping), unit.loc)
        };
        self.propose_move_unit_by_loc_and_id_following_shortest_paths(src, id, dest, shortest_paths)
    }

    pub fn move_unit_by_id_avoiding_combat(&mut self, id: UnitID, dest: Location) -> MoveResult {
        self.propose_move_unit_by_id_avoiding_combat(id, dest).map(|proposed_move| proposed_move.take(self))
    }

    pub fn propose_move_unit_by_id_avoiding_combat(&self, id: UnitID, dest: Location) -> ProposedMoveResult {
        let (shortest_paths, src) = {
            let unit = self.map.unit_by_id(id).unwrap();
            let unit_filter = AndFilter::new(
                AndFilter::new(
                    NoUnitsFilter{},
                    NoCitiesButOursFilter{alignment: unit.alignment }
                ),
                UnitMovementFilter{unit}
            );
            (shortest_paths(&self.map, unit.loc, &unit_filter, self.wrapping), unit.loc)
        };
        self.propose_move_unit_by_loc_and_id_following_shortest_paths(src, id, dest, shortest_paths)
    }

    /// Simulate and propose moving the unit at location `loc` with ID `id` to destination `dest`, guided by the shortest paths matrix in `shortest_paths`.
    /// 
    /// This does not actually carry out the move. The `ProposedMove` contained in an `Ok` result implements `ProposedAction` whose `take` method should be called
    /// to make the change real. This two-step process is designed to ease UI implementations. The UI should animate the proposed move against the background
    /// of the pre-move game state. After representing the move in the UI, the move should be committed to update the game state appopriately.
    /// 
    /// This is simpler to handle than when we would commit the move immediately. That left the UI to try to reverse-engineer the game state prior to the move,
    /// so the move could be animated accurately.
    fn propose_move_unit_by_loc_and_id_following_shortest_paths(&self, src: Location, id: UnitID, dest: Location, shortest_paths: ShortestPaths) -> ProposedMoveResult {
        if let Some(unit) = self.map.unit_by_loc_and_id(src, id) {

            self.propose_move_unit_following_shortest_paths(unit, dest, shortest_paths)

        } else {
            Err(MoveError::SourceUnitDoesNotExist{src_loc: src, id})
        }
    }

    /// Simulate and propose moving the unit provided to destination `dest`, guided by the shortest paths matrix in `shortest_paths`.
    /// 
    /// `unit` is the unit we're proposing the move for. It will be cloned and then the effects of the move on the unit simulated on the clone.
    /// 
    /// This does not actually carry out the move. The `ProposedMove` contained in an `Ok` result implements `ProposedAction` whose `take` method should be called
    /// to make the change real. This two-step process is designed to ease UI implementations. The UI should animate the proposed move against the background
    /// of the pre-move game state. After representing the move in the UI, the move should be committed to update the game state appopriately.
    /// 
    /// This is simpler to handle than when we would commit the move immediately. That left the UI to try to reverse-engineer the game state prior to the move,
    /// so the move could be animated accurately.
    fn propose_move_unit_following_shortest_paths(&self, unit: &Unit, dest: Location, shortest_paths: ShortestPaths) -> ProposedMoveResult {
        let obs_tracker = self.current_player_observations();
        let mut overlay = OverlayObsTracker::new(obs_tracker);
        self.propose_move_unit_following_shortest_paths_custom_tracker(unit, dest, shortest_paths, &mut overlay)
    }

    fn propose_move_unit_following_shortest_paths_custom_tracker<O:ObsTrackerI>(
        &self,
        unit: &Unit,
        dest: Location,
        shortest_paths: ShortestPaths,
        obs_tracker: &mut O
    ) -> ProposedMoveResult {

        if !obs_tracker.dims().contain(dest) {
            return Err(MoveError::DestinationOutOfBounds {
                dest,
                bounds: obs_tracker.dims(),
            });
        }

        // We copy the unit so we can simulate what will happen to it and send that version out with the ProposedMove
        let mut unit = unit.clone();

        // Keep a copy of the source location around
        let src = unit.loc;

        if let Some(distance) = shortest_paths.dist[dest] {
            if distance == 0 {
                return Err(MoveError::ZeroLengthMove);
            }

            if distance > unit.moves_remaining() {
                return Err(MoveError::RemainingMovesExceeded {
                    id: unit.id,
                    src,
                    dest,
                    intended_distance: distance,
                    moves_remaining: unit.moves_remaining(),
                });
            }


            let shortest_path: Vec<Location> = shortest_paths.shortest_path(dest);

            let mut moves = Vec::new();

            // Move along the shortest path to the destination
            // At each tile along the path, check if there's a unit there
            // If so, battle it
            // If we lose, this unit is destroyed
            // If we win, the opposing unit is destroyed and this unit continues its journey
            //     battling if necessary until it is either destroyed or reaches its destination
            //
            // Observe that the unit will either make it all the way to its destination, or
            // will be destroyed somewhere along the way. There will be no stopping midway.

            let mut move_complete = false;

            let mut it = shortest_path.iter();
            let first_loc = it.next().unwrap();// skip the source location
            debug_assert_eq!(src, *first_loc);
            for loc in it {
                let prev_loc = unit.loc;

                // Move our simulated unit along the path
                unit.loc = *loc;

                moves.push(MoveComponent::new(prev_loc, *loc));
                let mut move_ = moves.last_mut().unwrap();
                
                if let Obs::Observed{tile,..} = obs_tracker.get(*loc) {
                    if let Some(other_unit) = tile.unit.as_ref() {

                        if unit.is_friendly_to(other_unit) {
                            // the friendly unit must have space for us in its carrying capacity or else the
                            // path search wouldn't have included it
                            // We won't actually insert this unit in the space yet since it might move/get destroyed later
                            move_.carrier = Some(other_unit.id);
                        } else {
                            // On the other hand, we fight any unfriendly units
                            move_.unit_combat = Some(unit.fight(other_unit));
                        }
                    }
                }

                if let Some(ref outcome) = move_.unit_combat {
                    if outcome.destroyed() {
                        break;
                    }
                }

                if let Some(city) = self.map.city_by_loc(*loc) {
                    if city.alignment != unit.alignment {

                        // If there's a unit present, fight it and be done

                        // Otherwise, if this unit is able to occupy the city, fight the city


                        let has_moved = if let Some(other_unit) = self.map.toplevel_unit_by_loc(*loc) {
                            move_.unit_combat = Some(unit.fight(other_unit));

                            false

                        } else if unit.can_occupy_cities() {
                            let outcome = unit.fight(city);
                           
                            let victorious = outcome.victorious();

                            move_.city_combat = Some(outcome);

                            if victorious {
                                move_complete = true;
                            }

                            victorious

                        } else {
                            false
                        };

                        if !has_moved {
                            // Because the unit didn't actually move, we roll the move component's location
                            // back to previous.
                            move_.loc = prev_loc;
                            unit.loc = prev_loc;
                        }

                        break;// break regardless of outcome. Either conquer a city and stop, or be destroyed
                    }
                }
            }

            if move_complete {
                unit.movement_complete();
            } else {
                let distance_moved: usize = moves.iter().map(|move_| move_.distance_moved()).sum();
                unit.record_movement(distance_moved as u16).unwrap();
            }

            for move_ in moves.iter_mut() {
                if move_.moved_successfully() {
                    unit.loc = move_.loc;
                    
                    // Create an overlay with the unit at its new location. Also pretend it isn't at the source anymore.
                    let mut overlay = OverlaySource::new(&self.map);
                    let mut tile = self.map.tile(move_.loc).unwrap().clone();
                    // debug_assert!(tile.unit.is_none());

                    tile.unit = Some(unit.clone());//CLONE
                    overlay.put(move_.loc, &tile);

                    let mut src_tile = self.map.tile(src).unwrap().clone();

                    // Don't worry if there's no unit at `src`---Explore Mode does repeated moves but only re-places the
                    //                                           unit in the map after all moves are complete.
                    // debug_assert!(src_tile.unit.is_some());
                    // debug_assert_eq!(src_tile.unit.as_ref().unwrap().id, unit.id);

                    src_tile.unit = None;
                    overlay.put(src, &src_tile);

                    move_.observations_after_move = unit.observe(&overlay, self.turn, self.wrapping, obs_tracker);
                }
            }

            ProposedMove::new(unit, src, moves)
        } else {
            Err(MoveError::NoRoute{src, dest, id: unit.id})
        }
    }

    //FIXME Restrict to current player cities
    pub fn set_production(&mut self, loc: Location, production: UnitType) -> Result<(),String> {
        if let Some(city) = self.map.city_by_loc_mut(loc) {
            city.set_production(production);
            Ok(())
        } else {
            Err(format!(
                "Attempted to set production for city at location {} but there is no city at that location",
                loc
            ))
        }
    }

    //FIXME Restrict to current player cities
    pub fn clear_production_without_ignoring(&mut self, loc: Location) -> Result<(),String> {
        if let Some(city) = self.map.city_by_loc_mut(loc) {
            city.clear_production_without_ignoring();
            Ok(())
        } else {
            Err(format!(
                "Attempted to clear production for city at location {} but there is no city at that location",
                loc
            ))
        }
    }

    //FIXME Restrict to current player cities
    pub fn clear_production_and_ignore(&mut self, loc: Location) -> Result<(),String> {
        if let Some(city) = self.map.city_by_loc_mut(loc) {
            city.clear_production_and_ignore();
            Ok(())
        } else {
            Err(format!(
                "Attempted to clear production for city at location {} but there is no city at that location",
                loc
            ))
        }
    }

    pub fn turn(&self) -> TurnNum {
        self.turn
    }

    pub fn current_player(&self) -> PlayerNum {
        self.current_player
    }

    /// The logical dimensions of the game map
    pub fn dims(&self) -> Dims {
        self.map.dims()
    }

    pub fn wrapping(&self) -> Wrap2d {
        self.wrapping
    }

    /// Units that could be produced by a city located at the given location
    pub fn valid_productions(&self, loc: Location) -> BTreeSet<UnitType> {
        UnitType::values().iter()
        .cloned()
        .filter(|unit_type| {
            for neighb_loc in neighbors_terrain_only(&self.map, loc, *unit_type, self.wrapping) {
                let tile = self.map.tile(neighb_loc).unwrap();
                if unit_type.can_move_on_tile( &tile ) {
                    return true;
                }
            }
            false
        }).collect()
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    pub fn order_unit_sentry(&mut self, id: UnitID) -> OrdersResult {
        let orders = Orders::Sentry;

        self.current_player_unit_by_id_mut(id)
            .map(|unit| {        
                unit.orders = Some(orders);
                OrdersOutcome::completed_without_move(id, orders)
            })
            .ok_or(OrdersError::OrderedUnitDoesNotExist{id, orders})
    }

    pub fn order_unit_skip(&mut self, unit_id: UnitID) -> OrdersResult {
        let orders = Orders::Skip;
        self.set_orders(unit_id, Some(orders)).map(|_| OrdersOutcome::in_progress_without_move(unit_id, orders))
    }

    pub fn order_unit_go_to(&mut self, unit_id: UnitID, dest: Location) -> OrdersResult {
        self.propose_order_unit_go_to(unit_id, dest).take(self)
    }

    /// Simulate ordering the specified unit to go to the given location
    pub fn propose_order_unit_go_to(&mut self, unit_id: UnitID, dest: Location) -> ProposedSetAndFollowOrders {
        self.propose_set_and_follow_orders(unit_id, Orders::GoTo{dest})
    }

    pub fn order_unit_explore(&mut self, unit_id: UnitID) -> OrdersResult {
        self.propose_order_unit_explore(unit_id).take(self)
    }

    /// Simulate ordering the specified unit to explore.
    pub fn propose_order_unit_explore(&mut self, unit_id: UnitID) -> ProposedSetAndFollowOrders {
        self.propose_set_and_follow_orders(unit_id, Orders::Explore)
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    pub fn activate_unit_by_loc(&mut self, loc: Location) -> Result<(),GameError> {
        let current_player = self.current_player;
        if let Some(unit) = self.current_player_toplevel_unit_by_loc_mut(loc) {
            if unit.belongs_to_player(current_player) {
                unit.orders = None;
                for carried_unit in unit.carried_units_mut() {
                    carried_unit.orders = None;
                }
                Ok(())
            } else {
                Err(GameError::UnitNotControlledByCurrentPlayer{
                    msg: format!("Could not activate unit at location {} because it does not belong to current player {}", loc, self.current_player)
                })
            }
        } else {
            Err(GameError::NoUnitAtLocation{
                msg: format!("Could not activate unit at location {} because no such unit exists", loc),
                loc
            })
        }
    }

    /// If the current player controls a unit with ID `id`, set its orders to `orders`
    /// 
    /// # Errors
    /// `OrdersError::OrderedUnitDoesNotExist` if the order is not present under the control of the current player
    fn set_orders(&mut self, id: UnitID, orders: Option<Orders>) -> Result<(),OrdersError> {
        if let Some(ref mut unit) = self.current_player_unit_by_id_mut(id) {
            unit.orders = orders;
            Ok(())
        } else {
            Err(OrdersError::OrderedUnitDoesNotExist{id, orders: orders.unwrap()})
            // Err(format!("Attempted to give orders to a unit {:?} but no such unit exists", unit_id))
        }
    }

    /// Clear the orders of the unit controlled by the current player with ID `id`.
    fn clear_orders(&mut self, id: UnitID) -> Result<(),OrdersError> {
        self.set_orders(id, None)
    }

    fn follow_pending_orders(&mut self) -> Vec<OrdersResult> {
        let pending_orders: Vec<UnitID> = self.units_with_pending_orders().collect();

        pending_orders.iter()
            .map(|unit_id| self.follow_unit_orders(*unit_id))
            .collect()
    }

    fn propose_following_pending_orders(&mut self) -> Vec<ProposedOrdersResult> {
        let pending_orders: Vec<UnitID> = self.units_with_pending_orders().collect();

        pending_orders.iter()
            .map(|unit_id| self.propose_following_unit_orders(*unit_id))
            .collect()
    }

    /// Make the unit with ID `id` under the current player's control follow its orders
    /// 
    /// # Panics
    /// This will panic if the current player does not control such a unit.
    /// 
    fn follow_unit_orders(&mut self, id: UnitID) -> OrdersResult {
        let orders = self.current_player_unit_by_id(id).unwrap().orders.as_ref().unwrap();

        let result = orders.carry_out(id, self);

        // If the orders are already complete, clear them out
        if let Ok(OrdersOutcome{ status: OrdersStatus::Completed, .. }) = result {
            self.current_player_unit_by_id_mut(id).unwrap().orders = None;
        }
        
        result
    }

    fn propose_following_unit_orders(&mut self, id: UnitID) -> ProposedOrdersResult {
        let orders = self.current_player_unit_by_id(id).unwrap().orders.as_ref().unwrap();
        orders.propose(id, self)
    }

    /// Simulate setting the orders of unit with ID `id` to `orders` and then following them out.
    fn propose_set_and_follow_orders(&self, id: UnitID, orders: Orders) -> ProposedSetAndFollowOrders {
        ProposedSetAndFollowOrders {
            unit_id: id,
            orders,
            proposed_orders_result: orders.propose(id, self)
        }
    }
}

impl Dimensioned for Game {
    fn dims(&self) -> Dims {
        self.dims()
    }
}

impl Source<Tile> for Game {
    fn get(&self, loc: Location) -> &Tile {
        self.current_player_tile(loc).unwrap()
    }
}
impl Source<Obs> for Game {
    fn get(&self, loc: Location) -> &Obs {
        self.current_player_obs(loc)
    }
}
// impl Source<ResolvedObs> for Game {
//     fn get(&self, loc: Location) -> Option<&ResolvedObs> {
//         // self.current_player_obs(loc).map(|obs| match obs {
//         //     &Obs::Current =>
//         //         &ResolvedObs::Observation{tile: self.tile(loc).unwrap().clone(), turn: self.turn()},
//         //     &Obs::Observed{tile: tile, turn: turn} =>
//         //         &ResolvedObs::Observation{tile: tile, turn: turn},
//         //     &Obs::Unobserved => &ResolvedObs::Unobserved
//         // })

//         None
//         // self.current_player_obs(loc).map(|obs| match obs {
//         //     Obs::Current =>
//         //         ResolvedObs::Observation{tile: self.tile(loc).unwrap().clone(), turn: self.turn()},
//         //     Obs::Observed{tile: tile, turn: turn} =>
//         //         ResolvedObs::Observation{tile: tile, turn: turn},
//         //     Obs::Unobserved => ResolvedObs::Unobserved
//         // })
//     }
//     fn dims(&self) -> Dims {
//         self.map_dims()
//     }
// }


#[cfg(test)]
mod test {
    use core::cell::RefCell;

    use std::{
        convert::TryFrom,
        rc::Rc,
    };

    use crate::{
        game::{
            Alignment,
            Game,
            map::{
                MapData,
                Terrain,
            },
            obs::Obs,
            unit::{
                UnitID,
                UnitType,
                orders::{
                    Orders,
                    OrdersStatus,
                },
            },
        },
        name::{
            IntNamer,
            Named,
            unit_namer,
        },
        test::{game_two_cities_two_infantry},
        util::{Location,Wrap2d},
    };

    #[test]
    fn test_game() {
        let mut game = game_two_cities_two_infantry();

        for player in 0..2 {
            assert_eq!(game.unit_orders_requests().count(), 1);
            let unit_id: UnitID = game.unit_orders_requests().next().unwrap();
            let loc = game.current_player_unit_loc(unit_id).unwrap();
            let new_x = (loc.x + 1) % game.dims().width;
            let new_loc = Location{x:new_x, y:loc.y};
            println!("Moving unit from {} to {}", loc, new_loc);

            match game.move_toplevel_unit_by_loc(loc, new_loc) {
                Ok(move_result) => {
                    println!("{:?}", move_result);
                },
                Err(msg) => {
                    panic!("Error during move: {}", msg);
                }
            }
            // if let Ok(move_result) = game.move_unit_by_loc(loc, new_loc) {
            //     println!("{}", move_result);
            // } else {
                
            // }
            // let move_result = game.move_unit_by_loc(loc, new_loc)
            // assert!(game.move_unit_by_loc(loc, new_loc).is_ok());
            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1-player);
        }
    }

    #[test]
    fn test_move_unit() {
        let map = MapData::try_from("--0-+-+-1--").unwrap();
        {
            let loc1 = Location{x:2, y:0};
            let loc2 = Location{x:8, y:0};

            let city1tile = map.tile(loc1).unwrap();
            let city2tile = map.tile(loc2).unwrap();
            assert_eq!(city1tile.terrain, Terrain::Land);
            assert_eq!(city2tile.terrain, Terrain::Land);

            let city1 = city1tile.city.as_ref().unwrap();
            let city2 = city2tile.city.as_ref().unwrap();
            assert_eq!(city1.alignment, Alignment::Belligerent{player:0});
            assert_eq!(city2.alignment, Alignment::Belligerent{player:1});
            assert_eq!(city1.loc, loc1);
            assert_eq!(city2.loc, loc2);
        }

        let mut game = Game::new_with_map(map, 2, false, Rc::new(RefCell::new(unit_namer())), Wrap2d::BOTH);
        assert_eq!(game.current_player, 0);

        let productions = vec![UnitType::Armor, UnitType::Carrier];
        let players = vec![1, 0];

        for i in 0..2 {
            let loc: Location = game.production_set_requests().next().unwrap();
            assert_eq!(game.set_production(loc, productions[i]), Ok(()));

            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, players[i]);
        }

        for _ in 0..11 {
            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1);

            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 0);
        }
        assert_eq!(game.end_turn(), Err(0));

        // Move the armor unit to the right until it attacks the opposing city
        for round in 0..3 {
            assert_eq!(game.unit_orders_requests().count(), 1);
            let unit_id: UnitID = game.unit_orders_requests().next().unwrap();
            let loc = {
                let unit = game.current_player_unit_by_id(unit_id).unwrap();
                assert_eq!(unit.type_, productions[0]);
                unit.loc
            };
            
            let dest_loc = Location{x: loc.x+2, y:loc.y};
            println!("Moving from {} to {}", loc, dest_loc);
            let move_result = game.move_toplevel_unit_by_loc(loc, dest_loc).unwrap();
            println!("Result: {:?}", move_result);
            
            assert_eq!(move_result.unit.type_, UnitType::Armor);
            assert_eq!(move_result.unit.alignment, Alignment::Belligerent{player:0});
            


            // Check the first move component
            assert_eq!(move_result.components.len(), 2);
            let move1 = move_result.components.get(0).unwrap();
            assert_eq!(move1.loc, Location{x:loc.x+1, y:loc.y});
            assert_eq!(move1.unit_combat, None);
            assert_eq!(move1.city_combat, None);


            if move_result.moved_successfully() {
                // the unit conquered the city

                assert_eq!(move_result.ending_loc().unwrap(), dest_loc);

                assert_eq!(move_result.unit.moves_remaining(), 0);

                // Check the second move component, only here because the unit wasn't destroyed
                let move2 = move_result.components.get(1).unwrap();
                assert_eq!(move2.loc, dest_loc);
                assert_eq!(move2.unit_combat, None);

                if round < 2 {
                    assert_eq!(move2.city_combat, None);
                } else {
                    assert!(move2.city_combat.is_some());

                    // Since the armor defeated the city, set its production so we can end the turn
                    let conquered_city = move_result.conquered_city().unwrap();
                    let production_set_result = game.set_production(conquered_city.loc, UnitType::Fighter);
                    assert_eq!(production_set_result, Ok(()));
                }

            } else {
                // The unit was destroyed
                assert_eq!(move_result.unit.moves_remaining(), 1);
            }

            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1);

            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 0);
        }
    }

    // #[test]
    // fn test_unit_stops_after_conquering_city {
    //
    // }

    #[test]
    fn test_unit_moves_onto_transport() {
        let map = MapData::try_from("---it   ").unwrap();
        let infantry_loc = Location{x: 3, y: 0};
        let transport_loc = Location{x: 4, y: 0};

        let transport_id: UnitID = map.toplevel_unit_id_by_loc(transport_loc).unwrap();

        let mut game = Game::new_with_map(map, 1, false, Rc::new(RefCell::new(unit_namer())), Wrap2d::BOTH);
        let move_result = game.move_toplevel_unit_by_loc(infantry_loc, transport_loc).unwrap();
        assert_eq!(move_result.starting_loc, infantry_loc);
        assert_eq!(move_result.ending_loc(), Some(transport_loc));
        assert!(move_result.moved_successfully());
        assert_eq!(move_result.ending_carrier(), Some(transport_id));
    }

    #[test]
    fn test_set_orders() {
        let unit_namer = IntNamer::new("abc");
        let map = MapData::try_from("i").unwrap();
        let mut game = Game::new_with_map(map, 1, false, Rc::new(RefCell::new(unit_namer)), Wrap2d::NEITHER);
        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();

        assert_eq!(game.current_player_unit_by_id(unit_id).unwrap().orders, None);
        assert_eq!(game.current_player_unit_by_id(unit_id).unwrap().name(), &String::from("Unit_0_0"));

        game.set_orders(unit_id, Some(Orders::Sentry)).unwrap();

        assert_eq!(game.current_player_unit_by_id(unit_id).unwrap().orders, Some(Orders::Sentry));
    }

     #[test]
    pub fn test_order_unit_explore() {
        let unit_namer = IntNamer::new("unit");
        let map = MapData::try_from("i--------------------").unwrap();
        let mut game = Game::new_with_map(map, 1, true, Rc::new(RefCell::new(unit_namer)), Wrap2d::NEITHER);

        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();
        
        let outcome = game.order_unit_explore(unit_id).unwrap();
        assert_eq!(outcome.ordered_unit_id, unit_id);
        assert_eq!(outcome.orders, Orders::Explore);
        assert_eq!(outcome.status, OrdersStatus::InProgress);
    }


    #[test]
    pub fn test_propose_move_unit_by_id() {
        // fn propose_move_unit_following_shortest_paths_custom_tracker<O:ObsTrackerI>(
        //     &self,
        //     unit: &Unit,
        //     dest: Location,
        //     shortest_paths: ShortestPaths,
        //     obs_tracker: &mut O
        // ) -> ProposedMoveResult

        let src = Location{x:0, y:0};
        let dest = Location{x:1, y:0};

        let game = game_two_cities_two_infantry();

        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();

        {
            let unit = game.current_player_unit_by_id(unit_id).unwrap();
            assert_eq!(unit.loc, src);
        }

        let proposed_move = game.propose_move_unit_by_id(unit_id, dest).unwrap();

        let component = proposed_move.0.components.get(0).unwrap();

        // Make sure the intended destination is now observed as containing this unit, and that no other observed tiles
        // are observed as containing it
        for located_obs in &component.observations_after_move {
            match located_obs.item {
                Obs::Observed{ref tile, turn, current} =>{
                    if located_obs.loc == dest {
                        let unit = tile.unit.as_ref().unwrap();
                        assert_eq!(unit.id, unit_id);
                        assert_eq!(turn, 6);
                        assert!(current);
                    } else if let Some(unit) = tile.unit.as_ref() {
                        assert_ne!(unit.id, unit_id);
                    }
                },
                Obs::Unobserved => panic!("This should be observed")
            }
        }
    }
}
