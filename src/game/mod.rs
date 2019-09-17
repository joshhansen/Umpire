//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

pub mod map;
pub mod obs;
pub mod unit;


use std::collections::{BTreeSet,HashMap};

use crate::{
    color::{Colors,Colorized},
    game::{
        map::{
            Tile,
            gen::MapGenerator,
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
            newmap::{CityID,MapData,NewUnitError,UnitID},
        },
        obs::{Obs,Observer,ObsTracker},
        unit::{
            City,Unit,UnitType,
            combat::{CombatCapable,CombatOutcome},
            orders::{
                Orders,
                OrdersStatus,
                OrdersOutcome,
                OrdersResult,
            },
        },
    },
    log::{LogTarget,Message,MessageSource},
    name::{Namer,CompoundNamer,ListNamer,WeightedNamer},
    util::{Dims,Location,Wrap,Wrap2d},
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

#[derive(Debug)]
pub struct MoveResult {
    unit: Unit,
    starting_loc: Location,
    moves: Vec<MoveComponent>
}
impl MoveResult {
    /// unit represents the unit _after_ the move is completed
    fn new(unit: Unit, starting_loc: Location, moves: Vec<MoveComponent>) -> Result<Self,String> {
        if moves.is_empty() {
            Err(String::from("Attempted to create MoveResult with no moves"))
        } else {
            Ok(MoveResult{unit, starting_loc, moves})
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

    pub fn ending_loc(&self) -> Option<Location> {
        if self.moved_successfully() {
            self.moves.last().map(|move_| move_.loc)
            // Some(self.moves.last().unwrap().loc)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct MoveComponent {
    loc: Location,
    unit_combat: Option<CombatOutcome<Unit,Unit>>,
    city_combat: Option<CombatOutcome<Unit,City>>
}
impl MoveComponent {
    fn new(loc: Location) -> Self {
        MoveComponent {
            loc,
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

#[derive(Debug)]
pub enum GameError {
    NoSuchUnit { msg: String, id: UnitID },
    NoUnitAtLocation { msg: String, loc: Location },
    NoSuchCity { msg: String, id: CityID },
    NoCityAtLocation { msg: String, loc: Location },
    UnitNotControlledByCurrentPlayer { msg: String }
}

pub struct Game {
    // tiles: LocationGrid<Tile>, // tiles[col][row]
    map: MapData,
    player_observations: HashMap<PlayerNum,ObsTracker>,
    turn: TurnNum,
    num_players: PlayerNum,
    current_player: PlayerNum,
    wrapping: Wrap2d,
    unit_namer: CompoundNamer<WeightedNamer<f64>,WeightedNamer<u32>>,
    fog_of_war: bool,
}
impl Game {
    /// Creates a new game instance
    ///
    /// The Game that is returned will already have begun with the first player's turn
    /// A map with the specified dimensions will be generated
    /// If `fog_of_war` is `true` then players' view of the map will be limited to what they have previously
    /// observed, with observations growing stale over time.
    pub fn new<L:LogTarget>(
            map_dims: Dims,
            city_namer: ListNamer,
            num_players: PlayerNum,
            fog_of_war: bool,
            unit_namer: CompoundNamer<WeightedNamer<f64>,WeightedNamer<u32>>,
            log: &mut L) -> Self {

        let mut map_generator = MapGenerator::new(city_namer);
        let map = map_generator.generate(map_dims, num_players);
        Game::new_with_map(map, num_players, fog_of_war, unit_namer, log)
    }

    fn new_with_map<L:LogTarget>(map: MapData, num_players: PlayerNum,
            fog_of_war: bool, unit_namer: CompoundNamer<WeightedNamer<f64>,WeightedNamer<u32>>,
            log: &mut L) -> Self {

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

        log.log_message(format!("Starting new game with {} players, grid size {}, and fog of war {}",
                                num_players,
                                map.dims(),
                                if fog_of_war {"on"} else {"off"}
        ));

        let mut game = Game {
            map,
            player_observations,
            turn: 0,
            num_players,
            current_player: 0,
            wrapping: Wrap2d{horiz: Wrap::Wrapping, vert: Wrap::Wrapping},
            unit_namer,
            fog_of_war,
        };

        game.begin_turn(log);
        game
    }

    fn player_cities(&self) -> impl Iterator<Item=&City> {
        self.map.player_cities(self.current_player)
    }

    fn player_cities_with_production_target(&self) -> impl Iterator<Item=&City> {
        self.map.player_cities_with_production_target(self.current_player)
    }

    fn player_cities_with_production_target_mut(&mut self) -> impl Iterator<Item=&mut City> {
        self.map.player_cities_with_production_target_mut(self.current_player)
    }

    fn player_units(&self) -> impl Iterator<Item=&Unit> {
        self.map.player_units(self.current_player)
    }

    fn player_units_mut(&mut self) -> impl Iterator<Item=&mut Unit> {
        self.map.player_units_mut(self.current_player)
    }

    fn produce_units<L:LogTarget>(&mut self, log: &mut L) {
        for city in self.player_cities_with_production_target_mut() {
            city.production_progress += 1;
        }

        let producing_city_locs: Vec<Location> = self.player_cities_with_production_target()
            .filter(|city| {
                let unit_under_production = city.production().unwrap();

                city.production_progress >= unit_under_production.cost()
            }).map(|city| city.loc).collect()
        ;

        for city_loc in producing_city_locs {

            let (city_loc, city_alignment, city_desc, unit_under_production) = {
                let city = self.map.mut_city_by_loc(city_loc).unwrap();
                let unit_under_production = city.production().unwrap();
                (city.loc, city.alignment, format!("{}", city), unit_under_production)
            };

            let name = self.unit_namer.name();

            // Attempt to create the new unit

            let result = self.map.new_unit(city_loc, unit_under_production, city_alignment, name);

            match result {
                Ok(new_unit_id) => {
                    {
                        let city = self.map.mut_city_by_loc(city_loc).unwrap();
                        city.production_progress = 0;
                    };

                    let new_unit = self.map.unit_by_id(new_unit_id).unwrap();
                    
                    log.log_message(format!("{} produced {}", city_desc, new_unit));
                },
                Err(err) => match err {
                    NewUnitError::OutOfBounds{ loc, dims } => {
                        panic!(format!("Attempted to create a unit at {} outside the bounds {}", loc, dims))
                    },
                    NewUnitError::UnitAlreadyPresent{ loc:_loc, prior_unit } => {
                        log.log_message(Message {
                            text: format!(
                                "{} would have produced {} but {} was already garrisoned",
                                city_desc,
                                unit_under_production,
                                prior_unit
                            ),
                            mark: None,
                            fg_color: Some(Colors::Notice),
                            bg_color: None,
                            source: Some(MessageSource::Game)
                        });
                    }
                }
            }
        }
    }

    fn refresh_moves_remaining(&mut self) {
        for unit in self.player_units_mut() {
            // unit.moves_remaining = unit.movement_per_turn();
            unit.refresh_moves_remaining();
        }
    }

    fn begin_turn<L:LogTarget>(&mut self, log: &mut L) {
        log.log_message(Message {
            text: format!("Beginning turn {} for player {}", self.turn, self.current_player),
            mark: None,
            fg_color: Some(Colors::Notice),
            bg_color: None,
            source: Some(MessageSource::Game)
        });

        self.produce_units(log);

        self.refresh_moves_remaining();

        self.update_current_player_observations();
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
    pub fn end_turn<L:LogTarget>(&mut self, log: &mut L) -> Result<PlayerNum,PlayerNum> {
        if self.turn_is_done() {

            self.player_observations.get_mut(&self.current_player()).unwrap().archive();

            self.current_player = (self.current_player + 1) % self.num_players;
            if self.current_player == 0 {
                self.turn += 1;
            }

            self.begin_turn(log);

            Ok(self.current_player)
        } else {
            Err(self.current_player)
        }
    }

    fn update_current_player_observations(&mut self) {
        // let obs_tracker: &mut dyn ObsTracker = &mut ** (self.player_observations.get_mut(&self.current_player).unwrap()) ;
        let obs_tracker = self.player_observations.get_mut(&self.current_player).unwrap();

        let mut loc = Location{x: 0, y: 0};
        for x in 0..self.map.dims().width {
            loc.x = x;
            for y in 0..self.map.dims().height {
                loc.y = y;

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
    }

    pub fn current_player_tile(&self, loc: Location) -> Option<&Tile> {
        if let Obs::Observed{tile,..} = self.current_player_obs(loc) {
            Some(tile)
        } else {
            None
        }
    }

    pub fn current_player_obs(&self, loc: Location) -> &Obs {
        self.player_observations[&self.current_player()].get(loc)
    }

    pub fn city_by_loc(&self, loc: Location) -> Option<&City> {
        self.map.city_by_loc(loc)
    }

    pub fn unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        self.map.unit_by_loc(loc)
    }

    fn mut_unit_by_loc(&mut self, loc: Location) -> Option<&mut Unit> {
        self.map.mut_unit_by_loc(loc)
    }

    pub fn unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.map.unit_by_id(id)
    }

    // fn mut_unit_by_loc(&mut self, loc: Location) -> Option<&mut Unit> {
    //     self.map.mut_unit_by_loc(loc)
    // }

    fn mut_unit_by_id(&mut self, id: UnitID) -> Option<&mut Unit> {
        self.map.mut_unit_by_id(id)
    }

    pub fn unit_loc(&self, id: UnitID) -> Option<Location> {
        self.map.unit_loc(id)
    }

    pub fn production_set_requests<'a>(&'a self) -> impl Iterator<Item=Location> + 'a {
        self.map.player_cities_lacking_production_target(self.current_player).map(|city| city.loc)
    }

    /// Which if the current player's units need orders?
    /// 
    /// In other words, which of the current player's units have no orders and have moves remaining?
    pub fn unit_orders_requests<'a>(&'a self) -> impl Iterator<Item=UnitID> + 'a {
        self.map.player_units(self.current_player)
            .filter(|unit| unit.orders.is_none() && unit.moves_remaining() > 0)
            .map(|unit| unit.id)
    }

    pub fn units_with_pending_orders<'a>(&'a self) -> impl Iterator<Item=UnitID> + 'a {
        self.player_units()
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
    pub fn move_unit_by_loc(&mut self, src: Location, dest: Location) -> Result<MoveResult,String> {
        let shortest_paths = {
            let unit = self.map.unit_by_loc(src).unwrap();
            shortest_paths(&self.map, src, &UnitMovementFilter::new(unit), self.wrapping)
        };
        self.move_unit_by_loc_following_shortest_paths(src, dest, shortest_paths)
    }

    pub fn move_unit_by_loc_avoiding_combat(&mut self, src: Location, dest: Location) -> Result<MoveResult,String> {
        let shortest_paths = {
            let unit = self.map.unit_by_loc(src).unwrap();
                let unit_filter = AndFilter::new(
                    AndFilter::new(
                        NoUnitsFilter{},
                        NoCitiesButOursFilter{alignment: unit.alignment }
                    ),
                    UnitMovementFilter{unit}
                );
            shortest_paths(&self.map, src, &unit_filter, self.wrapping)
        };
        self.move_unit_by_loc_following_shortest_paths(src, dest, shortest_paths)
    }

    fn move_unit_by_loc_following_shortest_paths(&mut self, src: Location, dest: Location, shortest_paths: ShortestPaths) -> Result<MoveResult,String> {
        if let Some(distance) = shortest_paths.dist[dest] {
            if let Some(unit) = self.map.unit_by_loc(src) {
                if distance > unit.moves_remaining() {
                    return Err(format!("Ordered move of unit {} from {} to {} spans a distance ({}) greater than the number of moves remaining ({})",
                                unit, src, dest, distance, unit.moves_remaining()));
                }

                let mut unit = self.map.pop_unit_by_loc(src).unwrap();

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
                    if let Some(ref other_unit) = self.map.unit_by_loc(*loc) {
                        move_.unit_combat = Some(unit.fight(other_unit));
                    }
                    if let Some(ref outcome) = move_.unit_combat {
                        if outcome.destroyed() {
                            break;
                        } else {
                            self.map.destroy_unit_by_loc(*loc);// eliminate the unit we conquered
                        }
                    }

                    if let Some(city) = self.map.mut_city_by_loc(*loc) {
                        if city.alignment != unit.alignment {
                            let outcome = unit.fight(city);

                            if outcome.victorious() {
                                city.alignment = unit.alignment;
                            }

                            move_.city_combat = Some(outcome);

                            conquered_city = true;

                            break;// break regardless of outcome. Either conquer a city and stop, or be destroyed
                        }
                    }
                }

                if conquered_city {
                    // unit.moves_remaining = 0;
                    unit.movement_complete();
                } else {
                    // unit.moves_remaining -= moves.len() as u16;
                    unit.record_movement(moves.len() as u16).unwrap();
                }

                if let Some(move_) = moves.last() {
                    if move_.moved_successfully() {
                        self.map.set_unit(dest, unit.clone());
                    }
                }

                for move_ in moves.iter() {
                    if move_.moved_successfully() {
                        let mut obs_tracker = self.player_observations.get_mut(&self.current_player).unwrap();
                        unit.observe(move_.loc(), &self.map, self.turn, self.wrapping, &mut obs_tracker);
                    }
                }

                MoveResult::new(unit, src, moves)
            } else {
                Err(format!("Cannot move unit at source location {} because none exists", src))
            }
        } else {
            Err(format!("No route from {} to {}", src, dest))
        }
    }

    pub fn move_unit_by_id(&mut self, unit_id: UnitID, dest: Location) -> Result<MoveResult,String> {
        let src = self.map.unit_loc(unit_id).unwrap();
        self.move_unit_by_loc(src, dest)
    }

    pub fn move_unit_by_id_avoiding_combat(&mut self, unit_id: UnitID, dest: Location) -> Result<MoveResult,String> {
        let src = self.map.unit_loc(unit_id).unwrap();
        self.move_unit_by_loc_avoiding_combat(src, dest)
    }

    pub fn set_production(&mut self, loc: Location, production: UnitType) -> Result<(),String> {
        if let Some(city) = self.map.mut_city_by_loc(loc) {
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
        if let Some(city) = self.map.mut_city_by_loc(loc) {
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
        if let Some(city) = self.map.mut_city_by_loc(loc) {
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

    pub fn map_dims(&self) -> Dims {
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

    pub fn order_unit_sentry(&mut self, unit_id: UnitID) -> OrdersResult {
        self.mut_unit_by_id(unit_id)
            .map(|unit| {
                unit.orders = Some(Orders::Sentry);
                OrdersOutcome::completed_without_move()
            })
            .ok_or(format!("Cannot order unit {:?} to sentry because unit does not exist", unit_id))
    }

    pub fn order_unit_skip(&mut self, unit_id: UnitID) -> OrdersResult {
        self.set_orders(unit_id, Some(Orders::Skip)).map(|_| OrdersOutcome::in_progress_without_move())
    }

    pub fn order_unit_go_to(&mut self, unit_id: UnitID, dest: Location) -> OrdersResult {
        self.set_orders(unit_id, Some(Orders::GoTo{dest}))?;
        self.follow_orders(unit_id)
    }

    pub fn order_unit_explore(&mut self, unit_id: UnitID) -> OrdersResult {
        self.set_orders(unit_id, Some(Orders::Explore))?;
        self.follow_orders(unit_id)
    }

    /// If a unit at the location owned by the current player exists, activate it
    pub fn activate_unit_by_loc(&mut self, loc: Location) -> Result<(),GameError> {
        let current_player = self.current_player;
        if let Some(unit) = self.mut_unit_by_loc(loc) {
            if unit.belongs_to_player(current_player) {
                unit.orders = None;
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

    fn set_orders(&mut self, unit_id: UnitID, orders: Option<Orders>) -> Result<(),String> {
        if let Some(ref mut unit) = self.mut_unit_by_id(unit_id) {
            unit.orders = orders;
            Ok(())
        } else {
            Err(format!("Attempted to give orders to a unit {:?} but no such unit exists", unit_id))
        }
    }

    fn follow_orders(&mut self, unit_id: UnitID) -> OrdersResult {
        let orders = self.unit_by_id(unit_id).unwrap().orders.as_ref().unwrap().clone();//FIXME Do we really need this clone?

        let result = orders.carry_out(unit_id, self);

        // If the orders are already complete, clear them out
        if let Ok(OrdersOutcome{ status: OrdersStatus::Completed, .. }) = result {
            self.mut_unit_by_id(unit_id).unwrap().orders = None;
        }
        
        result
    }

    #[deprecated]
    fn give_orders(&mut self, unit_id: UnitID, orders: Option<Orders>, carry_out_now: bool) -> OrdersResult {
        self.set_orders(unit_id, orders)?;

        if carry_out_now {
            self.follow_orders(unit_id)
        } else {
            Ok(OrdersOutcome::completed_without_move())
        }
    }

    pub fn player_cities_producing_or_not_ignored(&self) -> usize {
        self.player_cities().filter(|city| city.production().is_some() || !city.ignore_cleared_production()).count()
    }
}

impl Source<Tile> for Game {
    fn get(&self, loc: Location) -> &Tile {
        self.current_player_tile(loc).unwrap()
    }
    fn dims(&self) -> Dims {
        self.map_dims()
    }
}
impl Source<Obs> for Game {
    fn get(&self, loc: Location) -> &Obs {
        self.current_player_obs(loc)
    }
    fn dims(&self) -> Dims {
        self.map_dims()
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
                Terrain,
                newmap::{MapData,UnitID},
            },
            unit::{UnitType},
        },
        log::{DefaultLog,LogTarget},
        name::unit_namer,
        util::{Dims,Location},
    };

    /// 10x10 grid of land only with two cities:
    /// * Player 0's Machang at 0,0
    /// * Player 1's Zanzibar at 0,1
    fn map1() -> MapData {
        let dims = Dims{width: 10, height: 10};
        let mut map = MapData::new(dims, |loc| Terrain::Land);
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

    fn game1<L:LogTarget>(log: &mut L) -> Game {
        let players = 2;
        let fog_of_war = true;

        let map = map1();
        let unit_namer = unit_namer();
        Game::new_with_map(map, players, fog_of_war, unit_namer, log)
    }

    #[test]
    fn test_game() {
        let mut log = DefaultLog;
        let mut game = game1(&mut log);

        let loc: Location = game.production_set_requests().next().unwrap();

        println!("Setting production at {:?} to infantry", loc);
        game.set_production(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn(&mut log).unwrap();
        assert_eq!(player, 1);

        let loc: Location = game.production_set_requests().next().unwrap();
        println!("Setting production at {:?} to infantry", loc);
        game.set_production(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn(&mut log).unwrap();
        assert_eq!(player, 0);


        for _ in 0..5 {
            let player = game.end_turn(&mut log).unwrap();
            assert_eq!(player, 1);
            let player = game.end_turn(&mut log).unwrap();
            assert_eq!(player, 0);
        }

        assert_eq!(game.end_turn(&mut log), Err(0));
        assert_eq!(game.end_turn(&mut log), Err(0));

        for player in 0..2 {
            assert_eq!(game.unit_orders_requests().count(), 1);
            let unit_id: UnitID = game.unit_orders_requests().next().unwrap();
            let loc = game.unit_loc(unit_id).unwrap();
            let new_x = (loc.x + 1) % game.map_dims().width;
            let new_loc = Location{x:new_x, y:loc.y};
            println!("Moving unit from {} to {}", loc, new_loc);

            match game.move_unit_by_loc(loc, new_loc) {
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
            assert_eq!(game.end_turn(&mut log), Ok(1-player));
        }
    }

    #[test]
    fn test_move_unit() {
        let map = MapData::try_from("--0-+-+-1--").unwrap();
        {
            let loc1 = Location{x:2, y:0};
            let loc2 = Location{x:8, y:0};

            let ref city1tile = map.tile(loc1).unwrap();
            let ref city2tile = map.tile(loc2).unwrap();
            assert_eq!(city1tile.terrain, Terrain::Land);
            assert_eq!(city2tile.terrain, Terrain::Land);

            let city1 = city1tile.city.as_ref().unwrap();
            let city2 = city2tile.city.as_ref().unwrap();
            assert_eq!(city1.alignment, Alignment::Belligerent{player:0});
            assert_eq!(city2.alignment, Alignment::Belligerent{player:1});
            assert_eq!(city1.loc, loc1);
            assert_eq!(city2.loc, loc2);
        }

        let mut log = DefaultLog;
        let mut game = Game::new_with_map(map, 2, false, unit_namer(), &mut log);

        let loc: Location = game.production_set_requests().next().unwrap();
        assert_eq!(game.set_production(loc, UnitType::Armor), Ok(()));
        assert_eq!(game.end_turn(&mut log), Ok(1));

        let loc: Location = game.production_set_requests().next().unwrap();
        assert_eq!(game.set_production(loc, UnitType::Carrier), Ok(()));
        assert_eq!(game.end_turn(&mut log), Ok(0));

        for _ in 0..11 {
            assert_eq!(game.end_turn(&mut log), Ok(1));
            assert_eq!(game.end_turn(&mut log), Ok(0));
        }
        assert_eq!(game.end_turn(&mut log), Err(0));

        // Move the armor unit to the right until it attacks the opposing city
        for round in 0..3 {
            assert_eq!(game.unit_orders_requests().count(), 1);
            let unit_id: UnitID = game.unit_orders_requests().next().unwrap();
            let loc = game.unit_loc(unit_id).unwrap();
            let dest_loc = Location{x: loc.x+2, y:loc.y};
            println!("Moving from {} to {}", loc, dest_loc);
            let move_result = game.move_unit_by_loc(loc, dest_loc).unwrap();
            println!("Result: {:?}", move_result);
            assert_eq!(move_result.unit().type_, UnitType::Armor);
            assert_eq!(move_result.unit().alignment, Alignment::Belligerent{player:0});
            assert_eq!(move_result.unit().moves_remaining(), 0);

            assert_eq!(move_result.moves().len(), 2);
            let ref move1 = move_result.moves()[0];
            assert_eq!(move1.loc, Location{x:loc.x+1, y:loc.y});
            assert_eq!(move1.unit_combat, None);
            assert_eq!(move1.city_combat, None);

            let ref move2 = move_result.moves()[1];
            assert_eq!(move2.loc, dest_loc);
            assert_eq!(move2.unit_combat, None);
            if round < 2 {
                assert_eq!(move2.city_combat, None);
            } else {
                assert!(move2.city_combat.is_some());
            }

            assert_eq!(game.end_turn(&mut log), Ok(1));
            assert_eq!(game.end_turn(&mut log), Ok(0));
        }
    }

    // #[test]
    // fn test_unit_stops_after_conquering_city {
    //
    // }
}
