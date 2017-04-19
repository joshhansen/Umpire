//!
//! Map generation
//!

// use std::num::Zero;

use rand::{Rng, ThreadRng, thread_rng};

use conf;
use map::{LocationGrid,Terrain,Tile,TileSource};
use name::{ListNamer,Namer};
use unit::{Alignment,City,PlayerNum};
use util::{Dims,Location};

fn is_land<T:TileSource>(tiles: &T, loc: Location) -> bool {
    tiles.get(loc).unwrap().terrain == Terrain::Land
}

fn land_cardinal_neighbors<T:TileSource>(tiles: &T, loc: Location) -> u16 {
    let map_dims = tiles.dims();

    let mut land_cardinal_neighbors = 0;

    // left
    if loc.x > 0 && is_land(tiles, Location{x: loc.x-1, y: loc.y}) {
        land_cardinal_neighbors += 1;
    }
    // right
    if loc.x < map_dims.width - 1 && is_land(tiles, Location{x: loc.x+1, y: loc.y}) {
        land_cardinal_neighbors += 1;
    }
    // up
    if loc.y > 0 && is_land(tiles, Location{x: loc.x, y: loc.y-1}) {
        land_cardinal_neighbors += 1;
    }
    // down
    if loc.y < map_dims.height - 1 && is_land(tiles, Location{x: loc.x, y: loc.y+1}) {
        land_cardinal_neighbors += 1;
    }

    land_cardinal_neighbors
}

fn land_diagonal_neighbors<T:TileSource>(tiles: &T, loc: Location) -> u16 {
    let map_dims = tiles.dims();

    let x_low_room = loc.x > 0;
    let y_low_room = loc.y > 0;
    let x_high_room = loc.x < map_dims.width - 1;
    let y_high_room = loc.y < map_dims.height - 1;

    let mut land_neighbors = 0;

    if x_low_room && y_low_room && is_land(tiles, Location{x: loc.x-1, y: loc.y-1}) {
        land_neighbors += 1;
    }
    if x_low_room && y_high_room && is_land(tiles, Location{x: loc.x-1, y: loc.y+1}) {
        land_neighbors += 1;
    }
    if x_high_room && y_low_room && is_land(tiles, Location{x: loc.x+1, y: loc.y-1}) {
        land_neighbors += 1;
    }
    if x_high_room && y_high_room && is_land(tiles, Location{x: loc.x+1, y: loc.y+1}) {
        land_neighbors += 1;
    }
    land_neighbors
}

// fn _land_neighbors(&self, x:u16, y:u16) -> u16 {
//     let mut land_nearby = 0;
//     for x2 in safe_minus_one(x)..(safe_plus_one(x, self.map_dims.width)+1) {
//         for y2 in safe_minus_one(y)..(safe_plus_one(y, self.map_dims.height)+1) {
//             if x2 != x && y2 != y {
//                 if self.tiles[x2 as usize][y2 as usize].terrain == Terrain::Land {
//                     land_nearby += 1;
//                 }
//             }
//         }
//     }
//     land_nearby
// }

pub struct MapGenerator {
    city_namer: ListNamer
}

fn rand_loc(rng: &mut ThreadRng, map_dims: Dims) -> Location {
    Location{
        x: rng.gen_range(0, map_dims.width),
        y: rng.gen_range(0, map_dims.height)
    }
}

impl MapGenerator {
    pub fn new(city_namer: ListNamer) -> Self {
        MapGenerator{ city_namer: city_namer }
    }

    pub fn generate(&mut self, map_dims: Dims, num_players: PlayerNum) -> LocationGrid<Tile> {
        let mut tiles = LocationGrid::new(map_dims, |loc| Tile::new(Terrain::Water, loc));

        let mut rng = thread_rng();

        // Seed the continents/islands
        for _ in 0..conf::LANDMASSES {
            let loc = rand_loc(&mut rng, map_dims);
            tiles[loc].terrain = Terrain::Land;
        }

        // Grow landmasses
        for _iteration in 0..conf::GROWTH_ITERATIONS {
            let mut loc = Location{x: 0, y: 0};
            for x in 0..map_dims.width {
                loc.x = x;
                for y in 0..map_dims.height {
                    loc.y = y;
                    match tiles[loc].terrain {
                        // Terrain::Land => {
                        //
                        //     for x2 in safe_minus_one(x)..(safe_plus_one(x, self.map_dims.width)+1) {
                        //         for y2 in safe_minus_one(y)..(safe_plus_one(y, self.map_dims.height)+1) {
                        //             if x2 != x && y2 != y {
                        //                 if rng.next_f32() <= GROWTH_PROB {
                        //                     self.tiles[x2 as usize][y2 as usize].terrain = Terrain::Land;
                        //                 }
                        //             }
                        //         }
                        //     }
                        // },
                        Terrain::Water => {
                            let cardinal_growth_prob = land_cardinal_neighbors(&tiles, loc) as f32 / (4_f32 + conf::GROWTH_CARDINAL_LAMBDA);
                            let diagonal_growth_prob = land_diagonal_neighbors(&tiles, loc) as f32 / (4_f32 + conf::GROWTH_DIAGONAL_LAMBDA);

                            if rng.next_f32() <= cardinal_growth_prob || rng.next_f32() <= diagonal_growth_prob {
                                tiles[loc].terrain = Terrain::Land;
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
                let loc = Location{x:x, y:y};
                let tile = &mut tiles[loc];
                if tile.terrain == Terrain::Land {
                    if rng.next_f32() <= conf::NEUTRAL_CITY_DENSITY {
                        tile.city = Some(City::new(Alignment::Neutral, loc, self.city_namer.name()));
                    }
                }
            }
        }

        // Populate player cities
        let mut player_num = 0;
        while player_num < num_players {
            let loc = rand_loc(&mut rng, map_dims);

            let tile = &mut tiles[loc];

            if tile.terrain == Terrain::Land {
                if tile.city.is_none() {
                    tile.city = Some(City::new(Alignment::Belligerent{ player: player_num }, loc, self.city_namer.name()));
                    player_num += 1;
                }
            }
        }

        tiles
    }
}
