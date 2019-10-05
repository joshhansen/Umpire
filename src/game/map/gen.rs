//!
//! Map generation
//!

use rand::{
    thread_rng,
    Rng,
    rngs::ThreadRng,
};

use crate::{
    conf,
    game::{
        Alignment,
        PlayerNum,
    },
    name::Namer,
    util::{Dims,Location,Wrap2d},
};

use super::{
    MapData,
    Terrain,
    Tile,
    dijkstra::{Source,TerrainFilter,neighbors,RELATIVE_NEIGHBORS_CARDINAL,RELATIVE_NEIGHBORS_DIAGONAL},
};


fn land_cardinal_neighbors<T:Source<Tile>>(tiles: &T, loc: Location) -> u16 {
    neighbors(tiles, loc, RELATIVE_NEIGHBORS_CARDINAL.iter(), &TerrainFilter{terrain: Terrain::Land}, Wrap2d::NEITHER).len() as u16
}

fn land_diagonal_neighbors<T:Source<Tile>>(tiles: &T, loc: Location) -> u16 {
    neighbors(tiles, loc, RELATIVE_NEIGHBORS_DIAGONAL.iter(), &TerrainFilter{terrain: Terrain::Land}, Wrap2d::NEITHER).len() as u16
}

// fn land_neighbors<T:TileSource>(tiles: &T, loc: Location) -> u16 {
//     neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &TerrainFilter{terrain: Terrain::Land}, WRAP_NEITHER).len() as u16
// }

const INITIAL_TERRAIN: Terrain = Terrain::Water;

fn rand_loc(rng: &mut ThreadRng, map_dims: Dims) -> Location {
    Location{
        x: rng.gen_range(0, map_dims.width),
        y: rng.gen_range(0, map_dims.height)
    }
}

pub fn generate_map<N:Namer>(city_namer: &mut N, map_dims: Dims, num_players: PlayerNum) -> MapData {
    let mut map = MapData::new(map_dims, |_loc| INITIAL_TERRAIN);

    let mut rng = thread_rng();

    // Seed the continents/islands
    for _ in 0..conf::LANDMASSES {
        let loc = rand_loc(&mut rng, map_dims);

        // This might overwrite an already-set terrain but it doesn't matter
        map.set_terrain(loc, Terrain::Land);
    }

    // Grow landmasses
    for _iteration in 0..conf::GROWTH_ITERATIONS {
        for loc in map_dims.iter_locs() {
            match map.terrain(loc).unwrap() {
                Terrain::Land => {

                    // for x2 in safe_minus_one(x)..(safe_plus_one(x, self.map_dims.width)+1) {
                    //     for y2 in safe_minus_one(y)..(safe_plus_one(y, self.map_dims.height)+1) {
                    //         if x2 != x && y2 != y {
                    //             if rng.next_f32() <= GROWTH_PROB {
                    //                 self.tiles[x2 as usize][y2 as usize].terrain = Terrain::Land;
                    //             }
                    //         }
                    //     }
                    // }
                },
                Terrain::Water => {
                    let cardinal_growth_prob = f32::from(land_cardinal_neighbors(&map, loc)) / (4_f32 + conf::GROWTH_CARDINAL_LAMBDA);
                    let diagonal_growth_prob = f32::from(land_diagonal_neighbors(&map, loc)) / (4_f32 + conf::GROWTH_DIAGONAL_LAMBDA);

                    if rng.gen::<f32>() <= cardinal_growth_prob || rng.gen::<f32>() <= diagonal_growth_prob {
                        // Might overwrite something here
                        map.set_terrain(loc, Terrain::Land);
                    }
                }
            }
        }
    }

    // Populate player cities
    let mut player_num = 0;
    while player_num < num_players {
        let loc = rand_loc(&mut rng, map_dims);

        if *map.terrain(loc).unwrap() == Terrain::Land && map.city_by_loc(loc).is_none() {
            map.new_city(loc, Alignment::Belligerent{ player: player_num }, city_namer.name()).unwrap();
            player_num += 1;
        }
    }

    // Populate neutral cities
    for loc in map_dims.iter_locs() {
        if *map.terrain(loc).unwrap() == Terrain::Land && map.city_by_loc(loc).is_none() && rng.gen::<f32>() <= conf::NEUTRAL_CITY_DENSITY {
            map.new_city(loc, Alignment::Neutral, city_namer.name()).unwrap();
        }
    }

    map
}