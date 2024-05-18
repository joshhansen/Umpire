//!
//! Map generation
//!

use rand::{distributions::Distribution, Rng, RngCore};

use crate::{
    conf,
    game::{Alignment, PlayerNum},
    name::Namer,
    util::{Dims, Location, Wrap2d},
};

use super::{
    dijkstra::{
        neighbors, Source, TerrainFilter, RELATIVE_NEIGHBORS_CARDINAL, RELATIVE_NEIGHBORS_DIAGONAL,
    },
    terrain::Terrainous,
    LocationGrid, MapData, Terrain,
};

fn land_cardinal_neighbors<T: Terrainous, S: Source<T>>(tiles: &S, loc: Location) -> u16 {
    neighbors(
        tiles,
        loc,
        RELATIVE_NEIGHBORS_CARDINAL.iter(),
        &TerrainFilter {
            terrain: Terrain::Land,
        },
        Wrap2d::NEITHER,
    )
    .len() as u16
}

fn land_diagonal_neighbors<T: Terrainous, S: Source<T>>(tiles: &S, loc: Location) -> u16 {
    neighbors(
        tiles,
        loc,
        RELATIVE_NEIGHBORS_DIAGONAL.iter(),
        &TerrainFilter {
            terrain: Terrain::Land,
        },
        Wrap2d::NEITHER,
    )
    .len() as u16
}

// fn land_neighbors<T:TileSource>(tiles: &T, loc: Location) -> u16 {
//     neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &TerrainFilter{terrain: Terrain::Land}, WRAP_NEITHER).len() as u16
// }

fn generate_continents<R: RngCore>(rng: &mut R, map_dims: Dims) -> LocationGrid<Terrain> {
    let mut grid = LocationGrid::new(map_dims, |_| Terrain::Water);

    // Seed the continents/islands
    for _ in 0..conf::LANDMASSES {
        let loc = map_dims.sample(rng);

        // This might overwrite an already-set terrain but it doesn't matter
        grid[loc] = Terrain::Land;
    }

    //FIXME by keeping an index of land locations and counts of cardinal/diagonal land neighbors this could probably be
    //      sped up substantially

    // Grow landmasses
    for _iteration in 0..conf::GROWTH_ITERATIONS {
        for loc in map_dims.iter_locs() {
            match grid[loc] {
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
                }
                Terrain::Water => {
                    let cardinal_growth_prob = f32::from(land_cardinal_neighbors(&grid, loc))
                        / (4_f32 + conf::GROWTH_CARDINAL_LAMBDA);
                    let diagonal_growth_prob = f32::from(land_diagonal_neighbors(&grid, loc))
                        / (4_f32 + conf::GROWTH_DIAGONAL_LAMBDA);

                    if rng.gen::<f32>() <= cardinal_growth_prob
                        || rng.gen::<f32>() <= diagonal_growth_prob
                    {
                        // Might overwrite something here
                        grid[loc] = Terrain::Land;
                    }
                }
            }
        }
    }

    grid
}

fn generate_transport_required(
    map_dims: Dims,
    left_continent_rightmosts: Vec<u16>,
    right_continent_leftmosts: Vec<u16>,
) -> LocationGrid<Terrain> {
    LocationGrid::new(map_dims, |loc| {
        let left_continent_rightmost = left_continent_rightmosts[loc.y as usize];
        let right_continent_leftmost = right_continent_leftmosts[loc.y as usize];
        if loc.x <= left_continent_rightmost || loc.x >= right_continent_leftmost {
            Terrain::Land
        } else {
            Terrain::Water
        }
    })
}

fn generate_random_terrain<R: RngCore>(
    rng: &mut R,
    map_dims: Dims,
    land_prob: f64,
) -> LocationGrid<Terrain> {
    LocationGrid::new(map_dims, |_| {
        if rng.gen_bool(land_prob) {
            Terrain::Land
        } else {
            Terrain::Water
        }
    })
}

fn populate_player_cities<N: Namer, R: RngCore>(
    rng: &mut R,
    map: &mut MapData,
    players: PlayerNum,
    city_namer: &mut N,
) {
    // Populate player cities
    let mut player_num = 0;
    while player_num < players {
        let loc = map.dims().sample(rng);

        if *map.terrain(loc).unwrap() == Terrain::Land && map.city_by_loc(loc).is_none() {
            map.new_city(
                loc,
                Alignment::Belligerent { player: player_num },
                city_namer.name(),
            )
            .unwrap();
            player_num += 1;
        }
    }
}

/// * land_only: Only place the cities on land
fn populate_neutral_cities<N: Namer, R: RngCore>(
    rng: &mut R,
    map: &mut MapData,
    city_namer: &mut N,
    land_only: bool,
) {
    // Populate neutral cities
    for loc in map.dims().iter_locs() {
        let land_ok = !land_only || map.terrain(loc).copied().unwrap() == Terrain::Land;
        if land_ok && map.city_by_loc(loc).is_none() && rng.gen_bool(conf::NEUTRAL_CITY_DENSITY) {
            map.new_city(loc, Alignment::Neutral, city_namer.name())
                .unwrap();
        }
    }
}

/// Populate the players' initial cities on the water on a transport-required type of map
fn populate_transport_required_cities<N: Namer>(
    map: &mut MapData,
    players: PlayerNum,
    city_namer: &mut N,
    left_continent_rightmosts: Vec<u16>,
    right_continent_leftmosts: Vec<u16>,
) {
    let height_inc = map.dims().height / players as u16;
    for player in 0..players {
        let y = height_inc * player as u16;
        let x = if player % 2 == 0 {
            left_continent_rightmosts[y as usize]
        } else {
            right_continent_leftmosts[y as usize]
        };
        let loc = Location::new(x, y);
        map.new_city(loc, Alignment::Belligerent { player }, city_namer.name())
            .unwrap();
    }
}

fn left_continent_rightmosts(left_continent_width: f64, map_dims: Dims) -> Vec<u16> {
    let base_rightmost = (left_continent_width * map_dims.width as f64) as u16;
    (0..map_dims.height)
        .enumerate()
        .map(|(i, _y)| {
            base_rightmost
                + match i % 7 {
                    0 => 4,
                    1 => 5,
                    2 => 0,
                    3 => 1,
                    4 => 3,
                    5 => 2,
                    6 => 0,
                    _ => panic!("Modular arithmetic failed us!"),
                }
        })
        .collect()
}

fn right_continent_leftmosts(right_continent_width: f64, map_dims: Dims) -> Vec<u16> {
    let base_leftmost = ((1f64 - right_continent_width) * map_dims.width as f64) as u16;
    (0..map_dims.height)
        .enumerate()
        .map(|(i, _y)| {
            base_leftmost
                - match i % 7 {
                    0 => 2,
                    1 => 5,
                    2 => 4,
                    3 => 1,
                    4 => 0,
                    5 => 2,
                    6 => 3,
                    _ => panic!("Modular arithmetic failed us!"),
                }
        })
        .collect()
}

#[derive(Copy, Clone)]
pub enum MapType {
    Continents,
    TransportRequired {
        /// Width as proportion of map width
        left_continent_width: f64,

        /// Width as proportion of map width
        right_continent_width: f64,
    },
    RandomTerrain {
        land_prob: f64,
    },
}
impl MapType {
    fn generate_terrain<R: RngCore>(&self, rng: &mut R, map_dims: Dims) -> LocationGrid<Terrain> {
        match self {
            Self::Continents => generate_continents(rng, map_dims),
            Self::TransportRequired {
                left_continent_width,
                right_continent_width,
            } => generate_transport_required(
                map_dims,
                left_continent_rightmosts(*left_continent_width, map_dims),
                right_continent_leftmosts(*right_continent_width, map_dims),
            ),
            Self::RandomTerrain { land_prob } => generate_random_terrain(rng, map_dims, *land_prob),
        }
    }

    fn initialize_cities<N: Namer, R: RngCore>(
        &self,
        rng: &mut R,
        map: &mut MapData,
        players: PlayerNum,
        city_namer: &mut N,
    ) {
        match self {
            Self::Continents => {
                populate_player_cities(rng, map, players, city_namer);
                populate_neutral_cities(rng, map, city_namer, true);
            }
            Self::TransportRequired {
                left_continent_width,
                right_continent_width,
            } => {
                populate_transport_required_cities(
                    map,
                    players,
                    city_namer,
                    left_continent_rightmosts(*left_continent_width, map.dims()),
                    right_continent_leftmosts(*right_continent_width, map.dims()),
                );
                populate_neutral_cities(rng, map, city_namer, true);
            }
            Self::RandomTerrain { .. } => {
                populate_player_cities(rng, map, players, city_namer);
                populate_neutral_cities(rng, map, city_namer, false);
            }
        }
    }

    pub fn generate<N: Namer, R: RngCore>(
        &self,
        rng: &mut R,
        map_dims: Dims,
        players: PlayerNum,
        city_namer: &mut N,
    ) -> MapData {
        let terrain = self.generate_terrain(rng, map_dims);

        let mut map = MapData::new(map_dims, |loc| terrain[loc]);

        self.initialize_cities(rng, &mut map, players, city_namer);

        map
    }
}

impl TryFrom<&str> for MapType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "c" => Ok(Self::Continents),
            "t" => Ok(Self::TransportRequired {
                left_continent_width: 0.3,
                right_continent_width: 0.3,
            }),
            "r" => Ok(Self::RandomTerrain { land_prob: 0.4 }),
            x => Err(format!("Unrecognized map type {}", x)),
        }
    }
}
