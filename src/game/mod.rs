//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

use std::collections::HashSet;

use map::{Tile,LocationGrid};
use map::gen::MapGenerator;
use map::dijkstra::shortest_paths;
use unit::{Alignment,City,PlayerNum,Unit,UnitType};
use util::{Dims,Location,Wrap,Wrap2d};

pub type TurnNum = u32;

/// What a particular player knows about a tile
// #[derive(Clone)]
// enum Obs {
//     OBSERVED{tile: Tile, turn: TurnNum},
//     UNOBSERVED
// }

pub struct Game {
    pub map_dims: Dims,
    pub tiles: LocationGrid<Tile>, // tiles[col][row]
    // player_maps: HashMap<PlayerNum,Vec<Vec<Obs>>>,
    pub turn: TurnNum,
    num_players: PlayerNum,
    current_player: Option<PlayerNum>,
    production_set_requests: HashSet<Location>,
    unit_move_requests: HashSet<Location>,
    wrapping: Wrap2d
}

impl Game {
    pub fn new(map_dims: Dims, num_players: PlayerNum) -> Self {
        let mut map_generator = MapGenerator::new();
        let map = map_generator.generate(map_dims);
        Game {
            map_dims: map_dims,
            tiles: map,
            // player_maps: player_maps,
            turn: 0,
            num_players: num_players,
            current_player: None,
            production_set_requests: HashSet::new(),
            unit_move_requests: HashSet::new(),
            wrapping: Wrap2d{horiz: Wrap::Wrapping, vert: Wrap::Wrapping}
        }
    }

    fn next_player(&self) -> PlayerNum {
        if let Some(current_player) = self.current_player {
            (current_player + 1) % self.num_players
        } else {
            0
        }
    }

    /// Returns the number of the player whose turn has just begun, or an error if the previous
    /// turn wasn't done yet.
    pub fn begin_next_player_turn(&mut self) -> Result<PlayerNum,PlayerNum> {
        if self.production_set_requests.is_empty() && self.unit_move_requests.is_empty() {
            // let player = self.next_player;
            // self.next_player = (self.next_player + 1) % self.num_players;
            let next_player = self.next_player();
            self.current_player = Some(next_player);
            self.begin_player_turn(next_player);
            return Ok(next_player);
        }
        Err(self.current_player.unwrap())
    }

    fn begin_player_turn(&mut self, player_num: PlayerNum) {
        for x in 0..self.map_dims.width {
            for y in 0..self.map_dims.height {
                let loc = Location{x:x, y:y};
                let tile: &mut Tile = &mut self.tiles[loc];

                if let Some(ref mut city) = tile.city {
                    if let Alignment::BELLIGERENT{player} = city.alignment {
                        if player==player_num {

                            if let Some(ref unit_under_production) = city.unit_under_production {
                                city.production_progress += 1;
                                if city.production_progress >= unit_under_production.cost() {
                                    let new_unit = Unit::new(*unit_under_production, city.alignment);
                                    tile.unit = Some(new_unit);
                                    city.production_progress = 0;
                                }
                            } else {
                                self.production_set_requests.insert(loc);
                            }
                        }
                    }
                }

                if let Some(ref mut unit) = tile.unit {
                    unit.moves_remaining += unit.movement_per_turn();
                    if !unit.sentry {
                        self.unit_move_requests.insert(loc);
                    }
                }
            }
        }
    }

    // fn player_map(&self, player: PlayerNum) -> Option<&Vec<Vec<Obs>>> {
    //     self.player_maps.get(&player)
    // }

    pub fn tile<'a>(&'a self, loc: Location) -> Option<&'a Tile> {
        self.tiles.get(&loc)
    }

    pub fn tile_mut<'a>(&'a mut self, loc: Location) -> Option<&'a mut Tile> {
        self.tiles.get_mut(&loc)
    }

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

    fn request_unit_move(&mut self, location: Location) {
        self.unit_move_requests.insert(location);
    }

    pub fn move_unit(&mut self, src: Location, dest: Location) -> Result<(),String> {
        let shortest_paths = shortest_paths(&self.tiles, &src, &self.wrapping);

        if let Some(distance) = shortest_paths.dist[dest] {
            println!("Dist: {}", distance);
            let unit = self.tiles[src].pop_unit();
            if let Some(mut unit) = unit {
                if distance > unit.moves_remaining {
                    Err(format!("Ordered move of unit {} from {} to {} spans a distance ({}) greater than the number of moves remaining ({})",
                                unit, src, dest, distance, unit.moves_remaining))
                } else {

                    self.unit_move_requests.remove(&src);
                    unit.moves_remaining -= distance;

                    if unit.moves_remaining > 0 {
                        self.unit_move_requests.insert(dest);
                    }

                    self.tiles[dest].set_unit(unit);

                    Ok(())
                }
            } else {
                Err(format!("No unit found at source location {}", src))
            }
        } else {
            return Err(format!("No route to {} from {}", dest, src));
        }
    }

    fn request_set_production(&mut self, location: Location) {
        self.production_set_requests.insert(location);
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
}

#[test]
fn test_game() {
    let game = Game::new(Dims{width:10, height:10}, 2);



}
