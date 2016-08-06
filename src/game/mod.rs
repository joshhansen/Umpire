//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

use std::collections::HashMap;

use map::Tile;
use map::gen::generate_map;
use unit::{Alignment,PlayerNum,Unit,cost as production_cost};
use util::Dims;

type Turn = u32;

/// What a particular player knows about a tile
enum Obs {
    OBSERVED{tile: Tile, turn: Turn},
    UNOBSERVED
}

pub struct Game {
    pub map_dims: Dims,
    pub tiles: Vec<Vec<Tile>>, // tiles[col][row]
    player_maps: HashMap<PlayerNum,Vec<Vec<Obs>>>,
    turn: Turn,
}

impl Game {
    pub fn new(map_dims: Dims) -> Self {
        Game {
            map_dims: map_dims,
            tiles: generate_map(map_dims),
            player_maps: HashMap::new(),
            turn: 0,
        }
    }

    pub fn run_turn(&mut self) {
        for x in 0..self.map_dims.width {
            for y in 0..self.map_dims.height {
                let tile = &mut self.tiles[x as usize][y as usize];

                match tile.city {
                    Some(ref mut city) => {
                        match city.alignment {
                            Alignment::BELLIGERENT{player} => {

                                match city.unit_under_production {
                                    None => {
                                        println!("Need to set production for city at {},{}", x, y);
                                    },
                                    Some(unit_under_production) => {
                                        city.production_progress += 1;
                                        if city.production_progress >= production_cost(unit_under_production) {
                                            let new_unit = Unit::new(unit_under_production, city.alignment, x, y);
                                            tile.units.push(new_unit);
                                        }
                                    }
                                }

                            },
                            Alignment::NEUTRAL => {}
                        }
                    },
                    None => {}
                }
            }
        }
    }

    fn player_map(&self, player: PlayerNum) -> Option<&Vec<Vec<Obs>>> {
        self.player_maps.get(&player)
    }
}
