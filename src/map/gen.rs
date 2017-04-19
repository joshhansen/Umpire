//!
//! Map generation
//!

// use std::num::Zero;

use rand::{Rng, ThreadRng, thread_rng};

use conf;
use map::{LocationGrid,Terrain,Tile,TileSource};
use map::dijkstra::{TerrainFilter,neighbors,RELATIVE_NEIGHBORS_CARDINAL,RELATIVE_NEIGHBORS_DIAGONAL};
use name::{ListNamer,Namer};
use unit::{Alignment,City,PlayerNum};
use util::{Dims,Location,WRAP_NEITHER};

fn land_cardinal_neighbors<T:TileSource>(tiles: &T, loc: Location) -> u16 {
    neighbors(tiles, loc, RELATIVE_NEIGHBORS_CARDINAL.iter(), &TerrainFilter{terrain: Terrain::Land}, WRAP_NEITHER).len() as u16
}

fn land_diagonal_neighbors<T:TileSource>(tiles: &T, loc: Location) -> u16 {
    neighbors(tiles, loc, RELATIVE_NEIGHBORS_DIAGONAL.iter(), &TerrainFilter{terrain: Terrain::Land}, WRAP_NEITHER).len() as u16
}

// fn land_neighbors<T:TileSource>(tiles: &T, loc: Location) -> u16 {
//     neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &TerrainFilter{terrain: Terrain::Land}, WRAP_NEITHER).len() as u16
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
