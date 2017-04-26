//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

pub mod obs;

use std::collections::{BTreeSet,HashMap,HashSet};

use game::obs::{FogOfWarTracker,Obs,Observer,ObsTracker,ResolvedObs,UniversalVisibilityTracker};
use log::{LogTarget,Message,MessageSource,Rgb};
use map::{Tile,LocationGrid};
use map::gen::MapGenerator;
use map::dijkstra::{Source,UnitMovementFilter,neighbors_terrain_only,shortest_paths};
use name::{Namer,CompoundNamer,ListNamer,WeightedNamer};
use unit::{Alignment,City,PlayerNum,Unit,UnitType};
use unit::combat::{CombatCapable,CombatOutcome};
use unit::orders::Orders;
use util::{Dims,Location,Wrap,Wrap2d};


pub type TurnNum = u32;

#[derive(Debug)]
pub struct MoveResult {
    unit: Unit,
    starting_loc: Location,
    moves: Vec<MoveComponent>
}
impl MoveResult {
    fn new(unit: Unit, starting_loc: Location, moves: Vec<MoveComponent>) -> Result<Self,String> {
        if moves.is_empty() {
            Err(String::from("Attempted to create MoveResult with no moves"))
        } else {
            Ok(MoveResult{unit:unit, starting_loc:starting_loc, moves:moves})
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
        self.moves.iter().map(|move_| move_.moved_successfully()).fold(true, |acc, x| acc && x)
    }

    pub fn ending_loc(&self) -> Option<Location> {
        if self.moved_successfully() {
            Some(self.moves.last().unwrap().loc)
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
            loc: loc,
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

pub struct Game {
    tiles: LocationGrid<Tile>, // tiles[col][row]
    player_observations: HashMap<PlayerNum,Box<ObsTracker>>,
    turn: TurnNum,
    num_players: PlayerNum,
    current_player: PlayerNum,
    production_set_requests: HashSet<Location>,
    unit_orders_requests: HashSet<Location>,
    wrapping: Wrap2d,
    unit_namer: CompoundNamer<WeightedNamer<f64>,WeightedNamer<u32>>
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

    fn new_with_map<L:LogTarget>(map: LocationGrid<Tile>, num_players: PlayerNum,
            fog_of_war: bool, unit_namer: CompoundNamer<WeightedNamer<f64>,WeightedNamer<u32>>,
            log: &mut L) -> Self {

        let mut player_observations = HashMap::new();
        for player_num in 0..num_players {
            let tracker: Box<ObsTracker> = if fog_of_war {
                Box::new(FogOfWarTracker::new(map.dims()))
            } else {
                Box::new(UniversalVisibilityTracker::new())
            };
            player_observations.insert(player_num, tracker);
        }

        log.log_message(format!("Starting new game with {} players, grid size {}, and fog of war {}",
                                num_players,
                                map.dims(),
                                if fog_of_war {"on"} else {"off"}
        ));

        let mut game = Game {
            tiles: map,
            player_observations: player_observations,
            turn: 0,
            num_players: num_players,
            current_player: 0,
            production_set_requests: HashSet::new(),
            unit_orders_requests: HashSet::new(),
            wrapping: Wrap2d{horiz: Wrap::Wrapping, vert: Wrap::Wrapping},
            unit_namer: unit_namer
        };

        game.begin_turn(log);
        game
    }

    fn begin_turn<L:LogTarget>(&mut self, log: &mut L) {
        log.log_message(Message {
            text: format!("Beginning turn {} for player {}", self.turn, self.current_player),
            mark: None,
            fg_color: Some(Rgb(255,140,0)),
            bg_color: None,
            source: Some(MessageSource::Game)
        });

        for x in 0..self.map_dims().width {
            for y in 0..self.map_dims().height {
                let loc = Location{x:x, y:y};
                let tile: &mut Tile = &mut self.tiles[loc];

                if let Some(ref mut city) = tile.city {
                    if let Alignment::Belligerent{player} = city.alignment {
                        if player==self.current_player {

                            if let Some(ref unit_under_production) = city.unit_under_production {
                                city.production_progress += 1;
                                if city.production_progress >= unit_under_production.cost() {
                                    let new_unit = Unit::new(*unit_under_production, city.alignment, self.unit_namer.name());
                                    log.log_message(format!("{} produced {}", city, new_unit));
                                    tile.unit = Some(new_unit);
                                    city.production_progress = 0;
                                }
                            } else {
                                log.log_message(format!("Queueing production set request for {}", city));
                                self.production_set_requests.insert(loc);
                            }
                        }
                    }
                }

                if let Some(ref mut unit) = tile.unit {
                    if let Alignment::Belligerent{player} = unit.alignment {
                        if player==self.current_player {
                            unit.moves_remaining = unit.movement_per_turn();
                            if unit.orders().is_none() {
                                log.log_message(format!("Queueing unit orders request for {}", unit));
                                self.unit_orders_requests.insert(loc);
                            }
                        }
                    }
                }
            }
        }

        self.update_current_player_observations();
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
        if self.production_set_requests.is_empty() && self.unit_orders_requests.is_empty() {
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
        let mut obs_tracker: &mut Box<ObsTracker> = self.player_observations.get_mut(&self.current_player).unwrap();

        for tile in self.tiles.iter() {
            if let Some(ref city) = tile.city {
                if let Alignment::Belligerent{player} = city.alignment {
                    if player==self.current_player {
                        city.observe(tile.loc, &self.tiles, self.turn, self.wrapping, obs_tracker);
                    }
                }
            }

            if let Some(ref unit) = tile.unit {
                if let Alignment::Belligerent{player} = unit.alignment {
                    if player==self.current_player {
                        unit.observe(tile.loc, &self.tiles, self.turn, self.wrapping, obs_tracker);
                    }
                }
            }
        }
    }

    fn tile<'a>(&'a self, loc: Location) -> Option<&'a Tile> {
        self.tiles.get(loc)
    }

    fn tile_mut<'a>(&'a mut self, loc: Location) -> Option<&'a mut Tile> {
        self.tiles.get_mut(loc)
    }

    pub fn current_player_tile<'a>(&'a self, loc: Location) -> Option<&'a Tile> {
        let obs = self.current_player_obs(loc).unwrap();

        match *obs {
            Obs::Current => self.tile(loc),
            Obs::Observed{ref tile,turn:_turn} => Some(tile),
            Obs::Unobserved => None
        }
    }

    pub fn current_player_obs(&self, loc: Location) -> Option<&Obs> {
        let obs_tracker: &Box<ObsTracker> = self.player_observations.get(&self.current_player).unwrap();
        obs_tracker.get(loc)
    }

    pub fn city<'b>(&'b self, loc: Location) -> Option<&'b City> {
        if let Some(tile) = self.tile(loc) {
            tile.city.as_ref()
        } else {
            None
        }
    }

    pub fn unit<'a>(&'a self, loc: Location) -> Option<&'a Unit> {
        if let Some(tile) = self.tile(loc) {
            tile.unit.as_ref()
        } else {
            None
        }
    }

    fn unit_mut<'a>(&'a mut self, loc: Location) -> Option<&'a mut Unit> {
        if let Some(tile) = self.tile_mut(loc) {
            tile.unit.as_mut()
        } else {
            None
        }
    }

    pub fn production_set_requests(&self) -> &HashSet<Location> {
        &self.production_set_requests
    }

    //FIXME Make it easy for UI clients to process unit move requests in unit order by returning these locations in a consistent unit order
    //      This means if unit A just moved and still has moves remaining, unit A will be the first in line to move again regardless of where
    //      they are located
    pub fn unit_orders_requests(&self) -> &HashSet<Location> {
        &self.unit_orders_requests
    }

    //FIXME Make the unit observe at each point along its path
    //FIXME This function checks two separate times whether a unit exists at src
    pub fn move_unit(&mut self, src: Location, dest: Location) -> Result<MoveResult,String> {
        let shortest_paths = {
            let unit = self.tiles[src].unit.as_ref().unwrap();
            shortest_paths(&self.tiles, src, &UnitMovementFilter::new(unit), self.wrapping)
        };
        if let Some(distance) = shortest_paths.dist[dest] {
            if let Some(mut unit) = self.tiles[src].pop_unit() {
                if distance > unit.moves_remaining {
                    Err(format!("Ordered move of unit {} from {} to {} spans a distance ({}) greater than the number of moves remaining ({})",
                                unit, src, dest, distance, unit.moves_remaining))
                } else {

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

                        let mut dest_tile = &mut self.tiles[*loc];
                        debug_assert_eq!(dest_tile.loc, *loc);
                        if let Some(ref other_unit) = dest_tile.unit {
                            move_.unit_combat = Some(unit.fight(other_unit));
                        }
                        if let Some(ref outcome) = move_.unit_combat {
                            if outcome.destroyed() {
                                break;
                            } else {
                                dest_tile.unit = None;// eliminate the unit we conquered
                            }
                        }

                        if let Some(ref mut city) = dest_tile.city {
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

                    debug_assert!(self.unit_orders_requests.remove(&src));

                    unit.moves_remaining -= moves.len() as u16;

                    if let Some(move_) = moves.last() {
                        if move_.moved_successfully() {
                            if !conquered_city && unit.moves_remaining > 0 {
                                self.unit_orders_requests.insert(dest);
                            }

                            self.tiles[dest].set_unit(unit.clone());
                        }

                        let mut obs_tracker: &mut Box<ObsTracker> = self.player_observations.get_mut(&self.current_player).unwrap();
                        unit.observe(dest, &self.tiles, self.turn, self.wrapping, obs_tracker);
                    }

                    MoveResult::new(unit, src, moves)
                }
            } else {
                Err(format!("No unit found at source location {}", src))
            }
        } else {
            Err(format!("No route to {} from {}", dest, src))
        }
    }

    //FIXME Make set_production return Err rather than panic if location is out of bounds
    pub fn set_production(&mut self, location: Location, production: UnitType) -> Result<(),String> {
        if let Some(ref mut city) = self.tiles[location].city {
            city.unit_under_production = Some(production);
            self.production_set_requests.remove(&location);
            Ok(())
        } else {
            Err(format!(
                "Attempted to set production for city at location {}
but there is no city at that location",
                location
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
        self.tiles.dims()
    }

    pub fn wrapping(&self) -> Wrap2d {
        self.wrapping
    }

    /// Units that could be produced by a city located at the given location
    pub fn valid_productions(&self, loc: Location) -> BTreeSet<UnitType> {
        UnitType::values().iter()
        .map(|unit_type| *unit_type)
        .filter(|unit_type| {
            for neighb_loc in neighbors_terrain_only(&self.tiles, loc, *unit_type, self.wrapping) {
                let ref neighb_tile = self.tiles[neighb_loc];
                if unit_type.can_move_on_terrain( &neighb_tile.terrain ) {
                    return true;
                }
            }
            return false;
        }).collect()
    }

    pub fn give_orders<U>(&mut self, loc: Location, orders: Option<Orders>, ui: &U) -> Result<(),String> {
        if let Some(unit) = self.unit_mut(loc) {
            unit.give_orders(orders);

            Ok(())
        } else {
            Err(format!("Attempted to give orders to a unit a {} but no such unit exists", loc))
        }
    }
}

impl Source<Tile> for Game {
    fn get(&self, loc: Location) -> Option<&Tile> {
        self.current_player_tile(loc)
    }
    fn dims(&self) -> Dims {
        self.map_dims()
    }
}
impl Source<Obs> for Game {
    fn get(&self, loc: Location) -> Option<&Obs> {
        self.current_player_obs(loc)
    }
    fn dims(&self) -> Dims {
        self.map_dims()
    }
}
impl Source<ResolvedObs> for Game {
    fn get(&self, loc: Location) -> Option<&ResolvedObs> {
        // self.current_player_obs(loc).map(|obs| match obs {
        //     &Obs::Current =>
        //         &ResolvedObs::Observation{tile: self.tile(loc).unwrap().clone(), turn: self.turn()},
        //     &Obs::Observed{tile: tile, turn: turn} =>
        //         &ResolvedObs::Observation{tile: tile, turn: turn},
        //     &Obs::Unobserved => &ResolvedObs::Unobserved
        // })

        None
        // self.current_player_obs(loc).map(|obs| match obs {
        //     Obs::Current =>
        //         ResolvedObs::Observation{tile: self.tile(loc).unwrap().clone(), turn: self.turn()},
        //     Obs::Observed{tile: tile, turn: turn} =>
        //         ResolvedObs::Observation{tile: tile, turn: turn},
        //     Obs::Unobserved => ResolvedObs::Unobserved
        // })
    }
    fn dims(&self) -> Dims {
        self.map_dims()
    }
}


#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    use game::Game;
    use log::{DefaultLog,LogTarget};
    use map::{LocationGrid,Terrain,Tile};
    use name::{test_unit_namer};
    use unit::{Alignment,City,UnitType};
    use util::{Dims,Location};

    /// 10x10 grid of land only with two cities:
    /// * Player 0's Machang at 0,0
    /// * Player 1's Zanzibar at 0,1
    fn map1() -> LocationGrid<Tile> {
        let dims = Dims{width: 10, height: 10};
        LocationGrid::new(dims, |loc| {
            let mut tile = Tile::new(Terrain::Land, loc);
            if loc.x == 0 {
                if loc.y == 0 {
                    tile.city = Some(City::new(Alignment::Belligerent{player:0}, loc, "Machang"));
                } else if loc.y == 1 {
                    tile.city = Some(City::new(Alignment::Belligerent{player:1}, loc, "Zanzibar"));
                }
            }
            tile
        })
    }

    fn game1<L:LogTarget>(log: &mut L) -> Game {
        let players = 2;
        let fog_of_war = true;

        let map = map1();
        let unit_namer = test_unit_namer().unwrap();
        Game::new_with_map(map, players, fog_of_war, unit_namer, log)
    }

    #[test]
    fn test_game() {
        let mut log = DefaultLog;
        let mut game = game1(&mut log);

        let loc = *game.production_set_requests().iter().next().unwrap();

        println!("Setting production at {:?} to infantry", loc);
        game.set_production(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn(&mut log).unwrap();
        assert_eq!(player, 1);

        let loc = *game.production_set_requests().iter().next().unwrap();
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
            assert_eq!(game.unit_orders_requests().len(), 1);
            let loc = *game.unit_orders_requests().iter().next().unwrap();
            let new_x = (loc.x + 1) % game.map_dims().width;
            let new_loc = Location{x:new_x, y:loc.y};
            println!("Moving unit from {} to {}", loc, new_loc);

            assert!(game.move_unit(loc, new_loc).is_ok());
            assert_eq!(game.end_turn(&mut log), Ok(1-player));
        }
    }

    #[test]
    fn test_move_unit() {
        let map = LocationGrid::try_from("--0-+-+-1--").unwrap();
        {
            let loc1 = Location{x:2, y:0};
            let loc2 = Location{x:8, y:0};

            let ref city1tile = map[loc1];
            let ref city2tile = map[loc2];
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
        let mut game = Game::new_with_map(map, 2, false, test_unit_namer().unwrap(), &mut log);

        let loc = *game.production_set_requests().iter().next().unwrap();
        assert_eq!(game.set_production(loc, UnitType::Armor), Ok(()));
        assert_eq!(game.end_turn(&mut log), Ok(1));

        let loc = *game.production_set_requests().iter().next().unwrap();
        assert_eq!(game.set_production(loc, UnitType::Carrier), Ok(()));
        assert_eq!(game.end_turn(&mut log), Ok(0));

        for _ in 0..11 {
            assert_eq!(game.end_turn(&mut log), Ok(1));
            assert_eq!(game.end_turn(&mut log), Ok(0));
        }
        assert_eq!(game.end_turn(&mut log), Err(0));

        // Move the armor unit to the right until it attacks the opposing city
        for round in 0..3 {
            assert_eq!(game.unit_orders_requests().len(), 1);
            let loc = *game.unit_orders_requests().iter().next().unwrap();
            let dest_loc = Location{x: loc.x+2, y:loc.y};
            println!("Moving from {} to {}", loc, dest_loc);
            let move_result = game.move_unit(loc, dest_loc).unwrap();
            println!("Result: {:?}", move_result);
            assert_eq!(move_result.unit().type_, UnitType::Armor);
            assert_eq!(move_result.unit().alignment, Alignment::Belligerent{player:0});
            assert_eq!(move_result.unit().moves_remaining, 0);

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
