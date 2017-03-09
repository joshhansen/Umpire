//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

pub mod obs;

use std::collections::{HashMap,HashSet};

use rand::{thread_rng, Rng};

use game::obs::{FogOfWarTracker,Obs,ObsTracker,UniversalVisibilityTracker};
use map::{Tile,LocationGrid};
use map::gen::MapGenerator;
use map::dijkstra::shortest_paths;
use unit::{Alignment,City,Observer,PlayerNum,Unit,UnitType};
use unit::combat::{CombatCapable,CombatOutcome,CombatParticipant};
use util::{Dims,Location,Wrap,Wrap2d};



pub type TurnNum = u32;

pub struct Game {
    pub map_dims: Dims,
    tiles: LocationGrid<Tile>, // tiles[col][row]
    player_observations: HashMap<PlayerNum,Box<ObsTracker>>,
    turn: TurnNum,
    num_players: PlayerNum,
    current_player: PlayerNum,
    production_set_requests: HashSet<Location>,
    unit_move_requests: HashSet<Location>,
    wrapping: Wrap2d
}

impl Game {
    /// Creates a new game instance
    ///
    /// The Game that is returned will already have begun with the first player's turn
    /// A map with the specified dimensions will be generated
    /// If `fog_of_war` is `true` then players' view of the map will be limited to what they have previously
    /// observed, with observations growing stale over time.
    pub fn new<L:FnMut(String)>(map_dims: Dims, num_players: PlayerNum, fog_of_war: bool, log_listener: &mut L) -> Self {
        let mut map_generator = MapGenerator::new();
        let map = map_generator.generate(map_dims, num_players);

        Game::new_with_map(map, num_players, fog_of_war, log_listener)
    }

    fn new_with_map<L:FnMut(String)>(map: LocationGrid<Tile>, num_players: PlayerNum, fog_of_war: bool, log_listener: &mut L) -> Self {
        let mut player_observations = HashMap::new();
        for player_num in 0..num_players {
            let tracker: Box<ObsTracker> = if fog_of_war {
                Box::new(FogOfWarTracker::new(map.dims()))
            } else {
                Box::new(UniversalVisibilityTracker::new())
            };
            player_observations.insert(player_num, tracker);
        }

        log_listener(format!("Starting new game with {} players, grid size {}, and fog of war {}",
                                num_players,
                                map.dims(),
                                if fog_of_war {"on"} else {"off"}
        ));

        let mut game = Game {
            map_dims: map.dims(),
            tiles: map,
            player_observations: player_observations,
            turn: 0,
            num_players: num_players,
            current_player: 0,
            production_set_requests: HashSet::new(),
            unit_move_requests: HashSet::new(),
            wrapping: Wrap2d{horiz: Wrap::Wrapping, vert: Wrap::Wrapping}
        };

        game.begin_turn(log_listener);
        game
    }

    fn begin_turn<L:FnMut(String)>(&mut self, log_listener: &mut L) {
        log_listener(format!("Beginning turn {} for player {}", self.turn, self.current_player));

        for x in 0..self.map_dims.width {
            for y in 0..self.map_dims.height {
                let loc = Location{x:x, y:y};
                let tile: &mut Tile = &mut self.tiles[loc];

                if let Some(ref mut city) = tile.city {
                    if let Alignment::BELLIGERENT{player} = city.alignment {
                        if player==self.current_player {

                            if let Some(ref unit_under_production) = city.unit_under_production {
                                city.production_progress += 1;
                                if city.production_progress >= unit_under_production.cost() {
                                    let new_unit = Unit::new(*unit_under_production, city.alignment);
                                    tile.unit = Some(new_unit);
                                    city.production_progress = 0;
                                    log_listener(format!("{} produced {}", city, &tile.unit.unwrap()));
                                }
                            } else {
                                self.production_set_requests.insert(loc);
                            }
                        }
                    }
                }

                if let Some(ref mut unit) = tile.unit {
                    unit.moves_remaining = unit.movement_per_turn();
                    if !unit.sentry {
                        self.unit_move_requests.insert(loc);
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
    pub fn end_turn<L:FnMut(String)>(&mut self, log_listener: &mut L) -> Result<PlayerNum,PlayerNum> {
        if self.production_set_requests.is_empty() && self.unit_move_requests.is_empty() {
            self.current_player = (self.current_player + 1) % self.num_players;
            if self.current_player == 0 {
                self.turn += 1;
            }

            self.begin_turn(log_listener);

            Ok(self.current_player)
        } else {
            Err(self.current_player)
        }
    }

    // fn next_player(&self) -> PlayerNum {
    //     if let Some(current_player) = self.current_player {
    //         (current_player + 1) % self.num_players
    //     } else {
    //         0
    //     }
    // }
    //
    // /// Returns the number of the player whose turn has just begun, or an error if the previous
    // /// turn wasn't done yet.
    // pub fn begin_next_player_turn(&mut self) -> Result<PlayerNum,PlayerNum> {
    //     if self.production_set_requests.is_empty() && self.unit_move_requests.is_empty() {
    //         let next_player = self.next_player();
    //
    //         if !self.current_player.is_none() && next_player==0 {
    //             self.turn += 1;
    //         }
    //
    //         self.current_player = Some(next_player);
    //         self.begin_player_turn(next_player);
    //         return Ok(next_player);
    //     }
    //     Err(self.current_player.unwrap())
    // }

    // fn begin_player_turn(&mut self, player_num: PlayerNum) {
    //     for x in 0..self.map_dims.width {
    //         for y in 0..self.map_dims.height {
    //             let loc = Location{x:x, y:y};
    //             let tile: &mut Tile = &mut self.tiles[loc];
    //
    //             if let Some(ref mut city) = tile.city {
    //                 if let Alignment::BELLIGERENT{player} = city.alignment {
    //                     if player==player_num {
    //
    //                         if let Some(ref unit_under_production) = city.unit_under_production {
    //                             city.production_progress += 1;
    //                             if city.production_progress >= unit_under_production.cost() {
    //                                 let new_unit = Unit::new(*unit_under_production, city.alignment);
    //                                 tile.unit = Some(new_unit);
    //                                 city.production_progress = 0;
    //                             }
    //                         } else {
    //                             self.production_set_requests.insert(loc);
    //                         }
    //                     }
    //                 }
    //             }
    //
    //             if let Some(ref mut unit) = tile.unit {
    //                 unit.moves_remaining += unit.movement_per_turn();
    //                 if !unit.sentry {
    //                     self.unit_move_requests.insert(loc);
    //                 }
    //             }
    //         }
    //     }
    //
    //     self.update_current_player_observations();
    // }

    fn update_current_player_observations(&mut self) {
        let mut obs_tracker: &mut Box<ObsTracker> = self.player_observations.get_mut(&self.current_player).unwrap();

        for tile in self.tiles.iter() {
            if let Some(ref city) = tile.city {
                if let Alignment::BELLIGERENT{player} = city.alignment {
                    if player==self.current_player {
                        city.observe(tile.loc, &self.tiles, self.turn, &self.wrapping, obs_tracker);
                    }
                }
            }

            if let Some(ref unit) = tile.unit {
                if let Alignment::BELLIGERENT{player} = unit.alignment {
                    if player==self.current_player {
                        unit.observe(tile.loc, &self.tiles, self.turn, &self.wrapping, obs_tracker);
                    }
                }
            }
        }
    }

    // fn current_player_obs_tracker_mut(&mut self) -> &mut Box<ObsTracker> {
    //     self.player_observations.get_mut(&self.current_player.unwrap()).unwrap()
    // }

    fn tile<'a>(&'a self, loc: Location) -> Option<&'a Tile> {
        self.tiles.get(&loc)
    }

    pub fn current_player_tile<'a>(&'a self, loc: Location) -> Option<&'a Tile> {
        let obs_tracker: &Box<ObsTracker> = self.player_observations.get(&self.current_player).unwrap();

        let obs: &Obs = obs_tracker.get(loc).unwrap();

        match *obs {
            Obs::CURRENT => self.tile(loc),
            Obs::OBSERVED{ref tile,turn:_turn} => Some(tile),
            Obs::UNOBSERVED => None
        }
    }

    // pub fn tile_mut<'a>(&'a mut self, loc: Location) -> Option<&'a mut Tile> {
    //     self.tiles.get_mut(&loc)
    // }

    pub fn city<'b>(&'b self, loc: Location) -> Option<&'b City> {
        if let Some(tile) = self.tile(loc) {
            if let Some(ref city) = tile.city {
                return Some(city);
            }
        }
        None
    }

    pub fn unit<'a>(&'a self, loc: Location) -> Option<&'a Unit> {
        if let Some(tile) = self.tile(loc) {
            if let Some(ref unit) = tile.unit {
                return Some(unit);
            }
        }
        None
    }

    pub fn production_set_requests(&self) -> &HashSet<Location> {
        &self.production_set_requests
    }

    pub fn unit_move_requests(&self) -> &HashSet<Location> {
        &self.unit_move_requests
    }

    // fn request_unit_move(&mut self, location: Location) {
    //     self.unit_move_requests.insert(location);
    // }

    pub fn move_unit(&mut self, src: Location, dest: Location) -> Result<Vec<CombatOutcome>,String> {
        let unit = self.tiles[src].unit.unwrap();
        let shortest_paths = shortest_paths(&self.tiles, &src, &unit, &self.wrapping);

        if let Some(distance) = shortest_paths.dist[dest] {
            println!("Dist: {}", distance);
            let unit = self.tiles[src].pop_unit();
            if let Some(mut unit) = unit {
                if distance > unit.moves_remaining {
                    Err(format!("Ordered move of unit {} from {} to {} spans a distance ({}) greater than the number of moves remaining ({})",
                                unit, src, dest, distance, unit.moves_remaining))
                } else {

                    // We're here because a route exists to the destination and a unit existed at the source

                    let shortest_path: Vec<Location> = shortest_paths.shortest_path(dest);


                    let mut combat_outcomes = Vec::new();

                    // Move along the shortest path to the destination
                    // At each tile along the path, check if there's a unit there
                    // If so, battle it
                    // If we lose, this unit is destroyed
                    // If we win, the opposing unit is destroyed and this unit continues its journey
                    //     battling if necessary until it is either destroyed or reaches its destination
                    //
                    // Observe that the unit will either make it all the way to its destination, or
                    // will be destroyed somewhere along the way. There will be no stopping midway.

                    let mut destroyed = false;
                    for _loc in shortest_path.iter().skip(1) {// skip the source location
                        if let Some(other_unit) = self.tiles[dest].unit {

                            let outcome = unit.fight(&other_unit);

                            destroyed |= *outcome.victor() != CombatParticipant::Attacker;

                            combat_outcomes.push(outcome);

                            if destroyed {
                                break;
                            }
                        }
                    }


                    self.unit_move_requests.remove(&src);

                    if !destroyed {
                        unit.moves_remaining -= distance;

                        if unit.moves_remaining > 0 {
                            self.unit_move_requests.insert(dest);
                        }

                        self.tiles[dest].set_unit(unit);


                        let mut obs_tracker: &mut Box<ObsTracker> = self.player_observations.get_mut(&self.current_player).unwrap();
                        unit.observe(dest, &self.tiles, self.turn, &self.wrapping, obs_tracker);
                    }
                    Ok(combat_outcomes)
                }
            } else {
                Err(format!("No unit found at source location {}", src))
            }
        } else {
            return Err(format!("No route to {} from {}", dest, src));
        }
    }

    pub fn set_production(&mut self, location: &Location, production: &UnitType) -> Result<(),String> {
        if let Some(ref mut city) = self.tiles[*location].city {
            city.unit_under_production = Some(*production);
            self.production_set_requests.remove(location);
            Ok(())
        } else {
            Err(format!(
                "Attempted to set production for city at location {}
but there is no city at that location",
                *location
            ))
        }
    }

    pub fn turn(&self) -> TurnNum {
        self.turn
    }

    pub fn current_player(&self) -> PlayerNum {
        self.current_player
    }

    pub fn tiles(&self) -> &LocationGrid<Tile> {
        &self.tiles
    }
}



#[cfg(test)]
mod test {
    use game::Game;
    use map::{LocationGrid,Terrain,Tile};
    use unit::{Alignment,City,UnitType};
    use util::{Dims,Location};

    #[test]
    fn test_game() {
        let mut log_listener = |msg:String| println!("{}", msg);

        let dims = Dims{width: 10, height: 10};
        let players = 2;
        let fog_of_war = true;

        let map: LocationGrid<Tile> = LocationGrid::new(&dims, |loc| {
            let mut tile = Tile::new(Terrain::LAND, *loc);
            if loc.x == 0 {
                if loc.y == 0 {
                    tile.city = Some(City::new("Machang", Alignment::BELLIGERENT{player:0}, *loc));
                } else if loc.y == 1 {
                    tile.city = Some(City::new("Zanzibar", Alignment::BELLIGERENT{player:1}, *loc));
                }
            }
            tile
        });
        let mut game = Game::new_with_map(map, players, fog_of_war, &mut log_listener);

        let loc = *game.production_set_requests().iter().next().unwrap();

        println!("Setting production at {:?} to infantry", loc);
        game.set_production(&loc, &UnitType::INFANTRY).unwrap();

        let player = game.end_turn(&mut log_listener).unwrap();
        assert_eq!(player, 1);

        let loc = *game.production_set_requests().iter().next().unwrap();
        println!("Setting production at {:?} to infantry", loc);
        game.set_production(&loc, &UnitType::INFANTRY).unwrap();

        let player = game.end_turn(&mut log_listener).unwrap();
        assert_eq!(player, 0);



        for _ in 0..5 {
            let player = game.end_turn(&mut log_listener).unwrap();
            assert_eq!(player, 1);
            let player = game.end_turn(&mut log_listener).unwrap();
            assert_eq!(player, 0);
        }

        assert_eq!(game.end_turn(&mut log_listener), Err(0));
        assert_eq!(game.end_turn(&mut log_listener), Err(0));

        for player in 0..2 {
            let loc = *game.unit_move_requests().iter().next().unwrap();
            let new_x = (loc.x + 1) % game.tiles().dims().width;
            let new_loc = Location{x:new_x, y:loc.y};
            println!("Moving unit from {} to {}", loc, new_loc);

            assert!(game.move_unit(loc, new_loc).is_ok());
            assert_eq!(game.end_turn(&mut log_listener), Ok(1-player));
        }

        assert!(false);

    }
}
