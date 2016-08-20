//!
//! Map generation
//!

extern crate rand;

// use std::num::Zero;

use rand::Rng;

use conf;
use map::{Terrain,TerrainType,Tile};
use unit::{Alignment,City};
use util::{Dims,Location};

fn is_land(tiles: &Vec<Vec<Tile>>, x:u16, y:u16) -> bool {
    return tiles[x as usize][y as usize].terrain.type_ == TerrainType::LAND;
}

fn land_cardinal_neighbors(tiles: &Vec<Vec<Tile>>, loc: Location, map_dims: Dims) -> u16 {
    let mut land_cardinal_neighbors = 0;

    // left
    if loc.x > 0 && is_land(tiles, loc.x-1, loc.y) {
        land_cardinal_neighbors += 1;
    }
    // right
    if loc.x < map_dims.width - 1 && is_land(tiles, loc.x+1, loc.y) {
        land_cardinal_neighbors += 1;
    }
    // up
    if loc.y > 0 && is_land(tiles, loc.x, loc.y-1) {
        land_cardinal_neighbors += 1;
    }
    // down
    if loc.y < map_dims.height - 1 && is_land(tiles, loc.x, loc.y+1) {
        land_cardinal_neighbors += 1;
    }

    land_cardinal_neighbors
}

fn land_diagonal_neighbors(tiles: &Vec<Vec<Tile>>, loc: Location, map_dims: Dims) -> u16 {
    let x_low_room = loc.x > 0;
    let y_low_room = loc.y > 0;
    let x_high_room = loc.x < map_dims.width - 1;
    let y_high_room = loc.y < map_dims.height - 1;

    let mut land_neighbors = 0;

    if x_low_room && y_low_room && is_land(tiles, loc.x-1, loc.y-1) {
        land_neighbors += 1;
    }
    if x_low_room && y_high_room && is_land(tiles, loc.x-1, loc.y+1) {
        land_neighbors += 1;
    }
    if x_high_room && y_low_room && is_land(tiles, loc.x+1, loc.y-1) {
        land_neighbors += 1;
    }
    if x_high_room && y_high_room && is_land(tiles, loc.x+1, loc.y+1) {
        land_neighbors += 1;
    }
    land_neighbors
}

// fn _land_neighbors(&self, x:u16, y:u16) -> u16 {
//     let mut land_nearby = 0;
//     for x2 in safe_minus_one(x)..(safe_plus_one(x, self.map_dims.width)+1) {
//         for y2 in safe_minus_one(y)..(safe_plus_one(y, self.map_dims.height)+1) {
//             if x2 != x && y2 != y {
//                 if self.tiles[x2 as usize][y2 as usize].terrain.type_ == TerrainType::LAND {
//                     land_nearby += 1;
//                 }
//             }
//         }
//     }
//     land_nearby
// }

pub fn generate_map(map_dims: Dims) -> Vec<Vec<Tile>> {
    let mut tiles = Vec::new();

    for x in 0..map_dims.width {
        let mut col = Vec::new();
        for y in 0..map_dims.height {
            col.push(Tile::new(Terrain::water(), Location{x:x,y:y}));
        }

        tiles.push(col);
    }



    let mut rng = rand::thread_rng();

    // Seed the continents/islands
    for _ in 0..conf::LANDMASSES {
        let x = rng.gen_range(0, map_dims.width);
        let y = rng.gen_range(0, map_dims.height);

        let terrain = &mut tiles[x as usize][y as usize].terrain;
        // let terrain = &mut tile.terrain;
        terrain.type_ = TerrainType::LAND;
    }

    // Grow landmasses
    for _iteration in 0..conf::GROWTH_ITERATIONS {
        for x in 0..map_dims.width {
            for y in 0..map_dims.height {

                match tiles[x as usize][y as usize].terrain.type_ {
                    // TerrainType::LAND => {
                    //
                    //     for x2 in safe_minus_one(x)..(safe_plus_one(x, self.map_dims.width)+1) {
                    //         for y2 in safe_minus_one(y)..(safe_plus_one(y, self.map_dims.height)+1) {
                    //             if x2 != x && y2 != y {
                    //                 if rng.next_f32() <= GROWTH_PROB {
                    //                     self.tiles[x2 as usize][y2 as usize].terrain.type_ = TerrainType::LAND;
                    //                 }
                    //             }
                    //         }
                    //     }
                    // },
                    TerrainType::WATER => {
                        let loc = Location{x: x, y: y};
                        let cardinal_growth_prob = land_cardinal_neighbors(&tiles, loc, map_dims) as f32 / (4_f32 + conf::GROWTH_CARDINAL_LAMBDA);
                        let diagonal_growth_prob = land_diagonal_neighbors(&tiles, loc, map_dims) as f32 / (4_f32 + conf::GROWTH_DIAGONAL_LAMBDA);

                        if rng.next_f32() <= cardinal_growth_prob || rng.next_f32() <= diagonal_growth_prob {
                            tiles[x as usize][y as usize].terrain.type_ = TerrainType::LAND;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Populate neutral cities
    for x in 0..map_dims.width {
        for y in 0..map_dims.height {
            let tile = &mut tiles[x as usize][y as usize];
            if tile.terrain.type_ == TerrainType::LAND {
                if rng.next_f32() <= conf::NEUTRAL_CITY_DENSITY {
                    tile.city = Some(City::new(Alignment::NEUTRAL, Location{x:x, y:y}));
                }
            }
        }
    }

    // Populate player cities
    let mut player_num = 0;
    while player_num < conf::NUM_PLAYERS {
        let loc = Location{
            x: rng.gen_range(0, map_dims.width),
            y: rng.gen_range(0, map_dims.height)
        };

        let tile = &mut tiles[loc];

        match tile.terrain.type_ {
            TerrainType::LAND => {
                if tile.city.is_none() {
                    tile.city = Some(City::new(Alignment::BELLIGERENT{ player: player_num }, loc));
                    player_num += 1;
                }
            },
            _ => {}
        }
    }

    tiles
}
