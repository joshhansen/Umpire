//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

pub mod city;
pub mod combat;
pub mod map;
pub mod obs;
pub mod unit;


use std::{
    collections::{BTreeSet,HashMap},
    fmt,
};

use failure::{
    Fail,
};

use crate::{
    color::{Colors,Colorized},
    game::{
        city::{CityID,City},
        combat::{CombatCapable,CombatOutcome},
        map::{
            MapData,
            NewUnitError,
            Tile,
            gen::generate_map,
            dijkstra::{
                AndFilter,
                NoCitiesButOursFilter,
                NoUnitsFilter,
                ShortestPaths,
                Source,
                UnitMovementFilter,
                neighbors_terrain_only,
                shortest_paths
            },
        },
        obs::{Obs,Observer,ObsTracker},
        unit::{
            UnitID,Unit,UnitType,
            orders::{
                Orders,
                OrdersError,
                OrdersStatus,
                OrdersOutcome,
                OrdersResult,
            },
        },
    },
    name::{Namer,CompoundNamer,ListNamer,WeightedNamer},
    util::{Dims,Location,Wrap2d},
};


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

#[derive(Debug,PartialEq)]
pub struct Move {
    unit: Unit,
    starting_loc: Location,
    moves: Vec<MoveComponent>
}
impl Move {
    /// unit represents the unit _after_ the move is completed
    fn new(unit: Unit, starting_loc: Location, moves: Vec<MoveComponent>) -> MoveResult {
        if moves.is_empty() {
            Err(MoveError::ZeroLengthMove)
        } else {
            Ok(Self{unit, starting_loc, moves})
        }
    }
    pub fn unit(&self) -> &Unit {
        &self.unit
    }

    pub fn moves(&self) -> &Vec<MoveComponent> {
        &self.moves
    }

    pub fn starting_loc(&self) -> Location {
        self.starting_loc
    }

    pub fn moved_successfully(&self) -> bool {
        self.moves.iter().map(MoveComponent::moved_successfully).all(|success| success)
    }

    /// The city conquered at the end of this move, if any
    pub fn conquered_city(&self) -> Option<&City> {
        if let Some(move_) = self.moves.last() {
            if let Some(city_combat) = move_.city_combat.as_ref() {
                if city_combat.victorious() {
                    return Some(city_combat.defender());
                }
            }
        }

        None
    }

    /// If the unit survived to the end of the move, its destination
    pub fn ending_loc(&self) -> Option<Location> {
        if self.moved_successfully() {
            self.moves.last().map(|move_| move_.loc)
            // Some(self.moves.last().unwrap().loc)
        } else {
            None
        }
    }

    /// If the unit survived to the end of the move, which (if any) unit ended up carrying it?
    pub fn ending_carrier(&self) -> Option<UnitID> {
        if self.moved_successfully() {
            if let Some(move_) = self.moves.last() {
                move_.carrier
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug,PartialEq)]
pub struct MoveComponent {
    loc: Location,
    /// Was the unit carried by another unit? If so, which one?
    carrier: Option<UnitID>,
    unit_combat: Option<CombatOutcome<Unit,Unit>>,
    city_combat: Option<CombatOutcome<Unit,City>>
}
impl MoveComponent {
    fn new(loc: Location) -> Self {
        MoveComponent {
            loc,
            carrier: None,
            unit_combat: None,
            city_combat: None
        }
    }

    pub fn moved_successfully(&self) -> bool {
        if let Some(ref combat) = self.unit_combat {
            if combat.destroyed() {
                return false;
            }
        }
        if let Some(ref combat) = self.city_combat {
            if combat.destroyed() {
                return false;
            }
        }
        true
    }

    pub fn unit_combat(&self) -> &Option<CombatOutcome<Unit,Unit>> {
        &self.unit_combat
    }

    pub fn city_combat(&self) -> &Option<CombatOutcome<Unit,City>> {
        &self.city_combat
    }

    pub fn loc(&self) -> Location {
        self.loc
    }
}

#[derive(Debug,Fail,PartialEq)]
pub enum MoveError {
    #[fail(display="Cannot execute a move of length zero")]
    ZeroLengthMove,

    #[fail(display="Ordered move of unit with ID {:?} from {} to {} spans a distance ({}) greater than the number of moves remaining ({})",
                    id, src, dest, intended_distance, moves_remaining)]
    RemainingMovesExceeded {
        id: UnitID,
        src: Location,
        dest: Location,
        intended_distance: u16,
        moves_remaining: u16,
    },

    #[fail(display="Cannot move unit at source location {} with ID {:?} because none exists", src_loc, id)]
    SourceUnitDoesNotExist {
        src_loc: Location,
        id: UnitID,
    },

    #[fail(display="No route from {} to {} for unit with ID {:?}", src, dest, id)]
    NoRoute {
        id: UnitID,
        src: Location,
        dest: Location,
    },

    #[fail(display="Destination {} lies outside of bounds {}", dest, bounds)]
    DestinationOutOfBounds {
        dest: Location,
        bounds: Dims,
    }
}

pub type MoveResult = Result<Move,MoveError>;


// //FIXME merge turn start and end because they're really the same thing
// pub struct TurnStart {
//     turn: TurnNum,
//     player: PlayerNum,
//     carried_out_orders: Vec<OrdersResult>,
// }

#[derive(Debug,PartialEq)]
pub struct TurnStart {
    pub turn: TurnNum,
    pub current_player: PlayerNum,
    pub carried_out_orders: Vec<OrdersResult>,
    pub production_outcomes: Vec<UnitProductionOutcome>,
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



pub struct Game {
    // tiles: LocationGrid<Tile>, // tiles[col][row]
    map: MapData,
    player_observations: HashMap<PlayerNum,ObsTracker>,
    turn: TurnNum,
    num_players: PlayerNum,
    current_player: PlayerNum,
    wrapping: Wrap2d,
    unit_namer: Box<dyn Namer>,
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
            unit_namer: Box<dyn Namer>,
            wrapping: Wrap2d) -> Self {

        let map = generate_map(&mut city_namer, map_dims, num_players);
        Game::new_with_map(map, num_players, fog_of_war, unit_namer, wrapping)
    }

    pub(crate) fn new_with_map(map: MapData, num_players: PlayerNum,
            fog_of_war: bool, unit_namer: Box<dyn Namer>,
            wrapping: Wrap2d) -> Self {

        let mut player_observations = HashMap::new();
        for player_num in 0..num_players {
            // let tracker: ObsTracker = if fog_of_war {
            //     ObsTracker::new_fog_of_war(map.dims())
            // } else {
            //     ObsTracker::UniversalVisibility
            // };
            // player_observations.insert(player_num, tracker);
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

            let name = self.unit_namer.name();

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
        // log.log_message(Message {
        //     text: format!("Beginning turn {} for player {}", self.turn, self.current_player),
        //     mark: None,
        //     fg_color: Some(Colors::Notice),
        //     bg_color: None,
        //     source: Some(MessageSource::Game)
        // });

        let production_outcomes = self.produce_units();

        self.refresh_moves_remaining();

        self.update_current_player_observations();

        let carried_out_orders = self.follow_pending_orders();

        TurnStart {
            turn: self.turn,
            current_player: self.current_player,
            carried_out_orders,
            production_outcomes,
        }
    }

    pub fn turn_is_done(&self) -> bool {
        self.production_set_requests().next().is_none() && self.unit_orders_requests().next().is_none()
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

            // // A new turn now begins:

            // let production_outcomes = self.produce_units();

            // self.refresh_moves_remaining();

            // self.update_current_player_observations();

            // let carried_out_orders = self.follow_pending_orders();

            // Ok(TurnStart {
            //     current_player: self.current_player,
            //     carried_out_orders,
            //     production_outcomes,
            // })
        } else {
            Err(self.current_player)
        }
    }

    // /// End the current turn if possible and begin a new one
    // /// 
    // /// This is a synonym for begin_turn since, if you think hard about it, they're the same thing
    // pub fn end_turn(&mut self) -> Result<TurnStart,PlayerNum> {
    //     self.begin_turn()
    // }

    /// Register the current observations of current player units
    /// 
    /// This applies only to top-level units. Carried units (e.g. units in a transport or carrier) make no observations
    fn update_current_player_observations(&mut self) {
        // let obs_tracker: &mut dyn ObsTracker = &mut ** (self.player_observations.get_mut(&self.current_player).unwrap()) ;
        let obs_tracker = self.player_observations.get_mut(&self.current_player).unwrap();

        for loc in self.map.dims().iter_locs() {
            let tile = self.map.tile(loc).unwrap();

            if self.fog_of_war {

                // With "fog of war" we only get updated observations where there are units and cities in the player's control
                
                if let Some(ref city) = tile.city {
                    if let Alignment::Belligerent{player} = city.alignment {
                        if player==self.current_player {
                            city.observe(tile.loc, &self.map, self.turn, self.wrapping, obs_tracker);
                        }
                    }
                }

                if let Some(ref unit) = tile.unit {
                    if let Alignment::Belligerent{player} = unit.alignment {
                        if player==self.current_player {
                            unit.observe(tile.loc, &self.map, self.turn, self.wrapping, obs_tracker);
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
        let id: UnitID = self.map.toplevel_unit_id_by_loc(src).unwrap();
        self.move_unit_by_loc_and_id_following_shortest_paths(src, id, dest, shortest_paths)
    }

    pub fn move_unit_by_id(&mut self, id: UnitID, dest: Location) -> MoveResult {
        let (shortest_paths, src) = {
            let unit = self.map.unit_by_id(id).unwrap();
            (shortest_paths(&self.map, unit.loc, &UnitMovementFilter::new(unit), self.wrapping), unit.loc)
        };
        self.move_unit_by_loc_and_id_following_shortest_paths(src, id, dest, shortest_paths)
    }

    pub fn move_unit_by_id_avoiding_combat(&mut self, id: UnitID, dest: Location) -> MoveResult {
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
        self.move_unit_by_loc_and_id_following_shortest_paths(src, id, dest, shortest_paths)
    }

    fn move_unit_by_loc_and_id_following_shortest_paths(&mut self, src: Location, id: UnitID, dest: Location, shortest_paths: ShortestPaths) -> MoveResult {
        if !self.dims().contain(dest) {
            return Err(MoveError::DestinationOutOfBounds {
                dest,
                bounds: self.dims(),
            });
        }

        if let Some(distance) = shortest_paths.dist[dest] {
            if distance == 0 {
                return Err(MoveError::ZeroLengthMove);
            }

            if let Some(unit) = self.map.unit_by_loc_and_id(src, id) {
                if distance > unit.moves_remaining() {
                    return Err(MoveError::RemainingMovesExceeded {
                        id: unit.id,
                        src,
                        dest,
                        intended_distance: distance,
                        moves_remaining: unit.moves_remaining(),
                    });
                    // return Err(format!("Ordered move of unit {} from {} to {} spans a distance ({}) greater than the number of moves remaining ({}) for unit with ID {:?}",
                    //             unit, src, dest, distance, unit.moves_remaining(), id));
                }

                let mut unit = self.map.pop_unit_by_loc_and_id(src, id).unwrap();

                // We're here because a route exists to the destination and a unit existed at the source

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

                let mut conquered_city = false;

                let mut it = shortest_path.iter();
                let first_loc = it.next().unwrap();// skip the source location
                debug_assert_eq!(src, *first_loc);
                for loc in it {
                    moves.push(MoveComponent::new(*loc));
                    let mut move_ = moves.last_mut().unwrap();

                    // let mut dest_tile = &mut self.tiles[*loc];
                    // debug_assert_eq!(dest_tile.loc, *loc);
                    if let Some(ref other_unit) = self.map.toplevel_unit_by_loc(*loc) {
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
                    if let Some(ref outcome) = move_.unit_combat {
                        if outcome.destroyed() {
                            break;
                        } else {
                            self.map.pop_toplevel_unit_by_loc(*loc).unwrap();// eliminate the unit we conquered
                        }
                    }

                    if let Some(city) = self.map.city_by_loc_mut(*loc) {
                        if city.alignment != unit.alignment {
                            let outcome = unit.fight(city);

                            if outcome.victorious() {
                                city.alignment = unit.alignment;
                                city.clear_production_without_ignoring();
                            }

                            move_.city_combat = Some(outcome);

                            conquered_city = true;

                            break;// break regardless of outcome. Either conquer a city and stop, or be destroyed
                        }
                    }
                }

                if conquered_city {
                    unit.movement_complete();
                } else {
                    unit.record_movement(moves.len() as u16).unwrap();
                }

                if let Some(move_) = moves.last() {
                    if move_.moved_successfully() {
                        if let Some(carrier_unit_id) = move_.carrier {
                            self.map.carry_unit(carrier_unit_id, unit.clone()).unwrap();
                        } else {
                            self.map.set_unit(dest, unit.clone());
                        }
                    }
                }

                for move_ in moves.iter() {
                    if move_.moved_successfully() {
                        let mut obs_tracker = self.player_observations.get_mut(&self.current_player).unwrap();
                        unit.observe(move_.loc(), &self.map, self.turn, self.wrapping, &mut obs_tracker);
                    }
                }

                Move::new(unit, src, moves)
            } else {
                // Err(format!("Cannot move unit at source location {} with ID {:?} because none exists", src, id))
                Err(MoveError::SourceUnitDoesNotExist{src_loc: src, id})
            }
        } else {
            // Err(format!("No route from {} to {} for unit with ID {:?}", src, dest, id))
            Err(MoveError::NoRoute{src, dest, id})
        }
    }

    pub fn move_toplevel_unit_by_id(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        let src = self.map.unit_loc(unit_id).unwrap();
        self.move_toplevel_unit_by_loc(src, dest)
    }

    pub fn move_toplevel_unit_by_id_avoiding_combat(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        let src = self.map.unit_loc(unit_id).unwrap();
        self.move_toplevel_unit_by_loc_avoiding_combat(src, dest)
    }

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
        // if !self.dims().in_bounds(dest) {
        //     return Err(OrdersError::Move{id: unit_id, dest, map_dims: self.dims()});
        //     // return Err(format!("Cannot order unit with ID {:?} to go to {} because {} is out of bounds", unit_id, dest, dest));
        // }

        self.set_orders(unit_id, Some(Orders::GoTo{dest}))?;
        self.follow_unit_orders(unit_id)
    }

    pub fn order_unit_explore(&mut self, unit_id: UnitID) -> OrdersResult {
        self.set_orders(unit_id, Some(Orders::Explore))?;
        self.follow_unit_orders(unit_id)
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

    fn follow_pending_orders(&mut self) -> Vec<OrdersResult> {
        let pending_orders: Vec<UnitID> = self.units_with_pending_orders().collect();

        pending_orders.iter()
            .map(|unit_id| self.follow_unit_orders(*unit_id))
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
}

impl Source<Tile> for Game {
    fn get(&self, loc: Location) -> &Tile {
        self.current_player_tile(loc).unwrap()
    }
    fn dims(&self) -> Dims {
        self.dims()
    }
}
impl Source<Obs> for Game {
    fn get(&self, loc: Location) -> &Obs {
        self.current_player_obs(loc)
    }
    fn dims(&self) -> Dims {
        self.dims()
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
    use std::convert::TryFrom;

    use crate::{
        game::{
            Alignment,
            Game,
            map::{
                MapData,
                Terrain,
            },
            unit::{UnitID,UnitType},
        },
        name::unit_namer,
        util::{Dims,Location,Wrap2d},
    };

    /// 10x10 grid of land only with two cities:
    /// * Player 0's Machang at 0,0
    /// * Player 1's Zanzibar at 0,1
    fn map1() -> MapData {
        let dims = Dims{width: 10, height: 10};
        let mut map = MapData::new(dims, |_loc| Terrain::Land);
        map.new_city(Location{x:0,y:0}, Alignment::Belligerent{player:0}, "Machang").unwrap();
        map.new_city(Location{x:0,y:1}, Alignment::Belligerent{player:1}, "Zanzibar").unwrap();
        // LocationGrid::new(dims, |loc| {
        //     let mut tile = Tile::new(Terrain::Land, loc);
        //     if loc.x == 0 {
        //         if loc.y == 0 {
        //             tile.city = Some(City::new(Alignment::Belligerent{player:0}, loc, "Machang"));
        //         } else if loc.y == 1 {
        //             tile.city = Some(City::new(Alignment::Belligerent{player:1}, loc, "Zanzibar"));
        //         }
        //     }
        //     tile
        // })
        map
    }

    fn game1() -> Game {
        let players = 2;
        let fog_of_war = true;

        let map = map1();
        let unit_namer = unit_namer();
        Game::new_with_map(map, players, fog_of_war, Box::new(unit_namer), Wrap2d::BOTH)
    }

    #[test]
    fn test_game() {
        let mut game = game1();

        let loc: Location = game.production_set_requests().next().unwrap();

        println!("Setting production at {:?} to infantry", loc);
        game.set_production(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn().unwrap().current_player;
        assert_eq!(player, 1);

        let loc: Location = game.production_set_requests().next().unwrap();
        println!("Setting production at {:?} to infantry", loc);
        game.set_production(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn().unwrap().current_player;
        assert_eq!(player, 0);


        for _ in 0..5 {
            let player = game.end_turn().unwrap().current_player;
            assert_eq!(player, 1);
            let player = game.end_turn().unwrap().current_player;
            assert_eq!(player, 0);
        }

        assert_eq!(game.end_turn(), Err(0));
        assert_eq!(game.end_turn(), Err(0));

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

        let mut game = Game::new_with_map(map, 2, false, Box::new(unit_namer()), Wrap2d::BOTH);
        assert_eq!(game.current_player, 0);

        let loc: Location = game.production_set_requests().next().unwrap();
        assert_eq!(game.set_production(loc, UnitType::Armor), Ok(()));

        let result = game.end_turn();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().current_player, 1);

        let loc: Location = game.production_set_requests().next().unwrap();
        assert_eq!(game.set_production(loc, UnitType::Carrier), Ok(()));

        let result = game.end_turn();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().current_player, 0);


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
            let loc = game.current_player_unit_loc(unit_id).unwrap();
            let dest_loc = Location{x: loc.x+2, y:loc.y};
            println!("Moving from {} to {}", loc, dest_loc);
            let move_result = game.move_toplevel_unit_by_loc(loc, dest_loc).unwrap();
            println!("Result: {:?}", move_result);
            assert_eq!(move_result.unit().type_, UnitType::Armor);
            assert_eq!(move_result.unit().alignment, Alignment::Belligerent{player:0});
            assert_eq!(move_result.unit().moves_remaining(), 0);

            assert_eq!(move_result.moves().len(), 2);
            let move1 = move_result.moves().get(0).unwrap();
            assert_eq!(move1.loc, Location{x:loc.x+1, y:loc.y});
            assert_eq!(move1.unit_combat, None);
            assert_eq!(move1.city_combat, None);

            let move2 = move_result.moves().get(1).unwrap();
            assert_eq!(move2.loc, dest_loc);
            assert_eq!(move2.unit_combat, None);
            if round < 2 {
                assert_eq!(move2.city_combat, None);
            } else {
                assert!(move2.city_combat.is_some());

                // If by chance the armor defeats the city, be sure to set its production so we can end the turn
                if let Some(conquered_city) = move_result.conquered_city() {
                    let production_set_result = game.set_production(conquered_city.loc, UnitType::Fighter);
                    assert_eq!(production_set_result, Ok(()));
                }
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

        let mut game = Game::new_with_map(map, 1, false, Box::new(unit_namer()), Wrap2d::BOTH);
        let move_result = game.move_toplevel_unit_by_loc(infantry_loc, transport_loc).unwrap();
        assert_eq!(move_result.starting_loc(), infantry_loc);
        assert_eq!(move_result.ending_loc(), Some(transport_loc));
        assert!(move_result.moved_successfully());
        assert_eq!(move_result.ending_carrier(), Some(transport_id));
    }
}
