//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

use std::collections::HashSet;
use std::ops::{Index,IndexMut};

use map::Tile;
use map::gen::generate_map;
use unit::{Alignment,City,PlayerNum,Unit,UnitType};
use util::{Dims,Location};

pub type TurnNum = u32;

/// What a particular player knows about a tile
#[derive(Clone)]
enum Obs {
    OBSERVED{tile: Tile, turn: TurnNum},
    UNOBSERVED
}

pub type Tiles = Vec<Vec<Tile>>;

impl Index<Location> for Tiles {
    type Output = Tile;
    fn index<'a>(&'a self, location: Location) -> &'a Tile {
        &self[location.x as usize][location.y as usize]
    }
}
impl IndexMut<Location> for Tiles {
    fn index_mut<'a>(&'a mut self, location: Location) -> &'a mut Tile {
        let col:  &mut Vec<Tile> = self.get_mut(location.x as usize).unwrap();
        col.get_mut(location.y as usize).unwrap()
    }
}

pub struct Game {
    pub map_dims: Dims,
    pub tiles: Tiles, // tiles[col][row]
    // player_maps: HashMap<PlayerNum,Vec<Vec<Obs>>>,
    pub turn: TurnNum,
    num_players: PlayerNum,
    current_player: Option<PlayerNum>,
    production_set_requests: HashSet<Location>,
    unit_move_requests: HashSet<Location>
}

impl Game {
    pub fn new(map_dims: Dims, num_players: PlayerNum) -> Self {
        Game {
            map_dims: map_dims,
            tiles: generate_map(map_dims),
            // player_maps: player_maps,
            turn: 0,
            num_players: num_players,
            current_player: None,
            production_set_requests: HashSet::new(),
            unit_move_requests: HashSet::new()
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
        if let Some(col) = self.tiles.get(loc.x as usize) {
            if let Some(ref tile) = col.get(loc.y as usize) {
                return Some(tile);
            }
        }
        None
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
        let distance = src.distance(&dest);

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
