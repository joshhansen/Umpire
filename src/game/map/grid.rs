use std::convert::TryFrom;
use std::fmt;
use std::iter::FromIterator;
use std::ops::{Index,IndexMut};

use crate::{
    game::{
        Alignment,
        PlayerNum,
        city::{CityID,City},
        obs::Obs,
        unit::{
            Unit,
            UnitID,
            UnitType,
        },
    },
    util::{Dims,Dimensioned,Location},
};

use super::{
    Terrain,
    Tile,
    dijkstra::Source,
};

// NOTE This is a dense representation and really doesn't handle large maps well, e.g. 10000x10000
#[derive(Clone)]
pub struct LocationGrid<T> {
    grid: Vec<Vec<T>>,//grid[col i.e. x][row i.e. y]
    dims: Dims
}

impl<T> LocationGrid<T> {
    pub fn new_from_vec(dims: Dims, grid: Vec<Vec<T>>) -> Self {
        LocationGrid{ grid, dims }
    }

    pub fn new<I>(dims: Dims, initializer: I) -> Self
        where I : Fn(Location) -> T {
        let mut grid: Vec<Vec<T>> = Vec::new();

        let mut loc = Location{x:0, y:0};

        for x in 0..dims.width {
            loc.x = x;

            let mut col: Vec<T> = Vec::new();
            for y in 0..dims.height {
                loc.y = y;

                col.push(initializer(loc));
            }

            grid.push(col);
        }

        LocationGrid{ grid, dims }
    }

    pub fn get(&self, loc: Location) -> Option<&T> {
        self.grid.get(loc.x as usize).and_then(|col| col.get(loc.y as usize))
    }

    pub fn get_mut(&mut self, loc: Location) -> Option<&mut T> {
        self.grid.get_mut(loc.x as usize).and_then(|col| col.get_mut(loc.y as usize))
    }

    pub fn dims(&self) -> Dims {
        self.dims
    }

    pub fn iter(&self) -> impl Iterator<Item=&T> {
        self.grid.iter().flat_map(|item: &Vec<T>| item.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item=&mut T> {
        self.grid.iter_mut().flat_map(|item: &mut Vec<T>| item.iter_mut())
    }

    pub fn iter_locs(&self) -> impl Iterator<Item=Location> {
        self.dims.iter_locs()
    }
}

// impl <T> Source<T> for LocationGrid<T> {
//     fn get(&self, loc: Location) -> &T {
//         self.get(loc)
//     }
//     fn dims(&self) -> Dims {
//         self.dims
//     }
// }

impl <T> Dimensioned for LocationGrid<T> {
    fn dims(&self) -> Dims {
        self.dims
    }
}

impl Source<Tile> for LocationGrid<Tile> {
    fn get(&self, loc: Location) -> &Tile {
        &self[loc]
    }
}

impl Source<Obs> for LocationGrid<Obs> {
    fn get(&self, loc: Location) -> &Obs {
        if let Some(obs) = self.get(loc) {
            obs
        } else {
            &Obs::Unobserved
        }
    }
}

impl<T> Index<Location> for LocationGrid<T> {
    type Output = T;
    fn index(&self, location: Location) -> &T {
        &self.grid[location.x as usize][location.y as usize]
    }
}

impl<T> IndexMut<Location> for LocationGrid<T> {
    fn index_mut(&mut self, location: Location) -> &mut T {
        let col:  &mut Vec<T> = &mut self.grid[location.x as usize];
        &mut col[location.y as usize]
    }
}

impl <T:fmt::Debug> fmt::Debug for LocationGrid<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = write!(f, "");

        for y in 0..self.dims.height {
            for x in 0..self.dims.width {
                result = result.and(self[Location::new(x,y)].fmt(f));
            }
            result = result.and(writeln!(f));
        }

        result
    }
}

/// Convert a multiline string into a map
/// A convenience method
/// For example:
/// `LocationGrid::try_from(
/// "xx x x\
///  xI  xx\
///  x1  0a"
/// )`
/// would yield a location grid with tiles populated thus:
/// * numerals represent land terrain with a city belonging to the player of that number
///   i.e. character "3" becomes a city belonging to player 3 located on land.
/// * letters corresponding to a UnitType key represent a unit of that type belonging to player 0
/// * letters whose lowercase corresponds to a UnitType key represent a unit of that type belonging to player 1
/// * other non-whitespace characters correspond to empty land
/// * whitespace characters correspond to water
///
/// Error if there are no lines or if the lines aren't of equal length
impl TryFrom<String> for LocationGrid<Tile> {
    type Error = String;
    fn try_from(s: String) -> Result<LocationGrid<Tile>,String> {
        let lines: Vec<Vec<char>> = Vec::from_iter( s.lines().map(|line| Vec::from_iter( line.chars() )) );
        if lines.is_empty() {
            return Err(String::from("String contained no lines"));
        }

        let width = lines[0].len();
        if lines.len() == 1 && width == 0 {
            return Err(String::from("No map was provided (the string was empty)"));
        }

        for line in &lines {
            if line.len() != width {
                return Err(format!("Lines aren't all the same width. Expected {}, found {}", width, line.len()));
            }
        }

        let height = lines.len() as u16;

        let mut grid = LocationGrid::new(Dims::new(width as u16, height as u16), |loc| {
            let c = lines[loc.y as usize][loc.x as usize];
            Tile::new(
                if c==' ' {
                    Terrain::Water
                } else {
                    Terrain::Land
                },
                loc
            )
        });

        let mut next_city_id = CityID::default();
        let mut next_unit_id = UnitID::default();

        for (y,line) in lines.iter().enumerate() {
            for (x, c) in line.iter().enumerate() {
                let loc = Location::new(x as u16, y as u16);
                let tile = grid.get_mut(loc).unwrap();

                if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
                    tile.city = Some(City::new(
                        // CityID::new(id),
                        next_city_id,
                        Alignment::Belligerent{player: player_num},
                        loc,
                        format!("City_{}_{}", loc.x, loc.y)
                    ));
                    next_city_id = next_city_id.next();
                }
                if let Ok(unit_type) = UnitType::try_from_key(*c) {
                    tile.unit = Some({
                        let unit = Unit::new(
                            // UnitID::new(id),
                            next_unit_id,
                            loc,
                            unit_type,
                            Alignment::Belligerent{player: 0},
                            format!("Unit_{}_{}", loc.x, loc.y)
                        );
                        next_unit_id = next_unit_id.next();
                        unit
                    });

                    // Override the terrain to match the unit
                    tile.terrain = unit_type.default_terrain();

                } else if let Some(c_lower) = c.to_lowercase().next() {
                    if let Ok(unit_type) = UnitType::try_from_key(c_lower) {
                        tile.unit = Some({
                            let unit = Unit::new(
                                // UnitID::new(id),
                                next_unit_id,
                                loc,
                                unit_type,
                                Alignment::Belligerent{player: 1},
                                format!("Unit_{}_{}", loc.x, loc.y)
                            );
                            next_unit_id = next_unit_id.next();
                            unit
                        });

                        // Override the terrain to match the unit
                        tile.terrain = unit_type.default_terrain();
                    }
                }
            }
        }

        Ok(grid)

        // Ok(
        //     LocationGrid::new(
        //         Dims{width: width as u16, height: height as u16 },
        //         |loc| {
        //             let c = lines[loc.y as usize][loc.x as usize];
        //             let mut tile = Tile::new(
        //                 if c==' ' {
        //                     Terrain::Water
        //                 } else {
        //                     Terrain::Land
        //                 },
        //                 loc
        //             );
        //             // let id: u64 = u64::from(loc.x * height + loc.y);
        //             if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
        //                 tile.city = Some(City::new(
        //                     // CityID::new(id),
        //                     next_city_id,
        //                     Alignment::Belligerent{player: player_num},
        //                     loc,
        //                     format!("City_{}_{}", loc.x, loc.y)
        //                 ));
        //                 next_city_id = next_city_id.next();
        //             }
        //             if let Ok(unit_type) = UnitType::try_from_key(c) {
        //                 tile.unit = Some({
        //                     let unit = Unit::new(
        //                         // UnitID::new(id),
        //                         next_unit_id,
        //                         loc,
        //                         unit_type,
        //                         Alignment::Belligerent{player: 0},
        //                         format!("Unit_{}_{}", loc.x, loc.y)
        //                     );
        //                     next_unit_id = next_unit_id.next();
        //                     unit
        //                 });

        //                 // Override the terrain to match the unit
        //                 tile.terrain = unit_type.default_terrain();

        //             } else if let Some(c_lower) = c.to_lowercase().next() {
        //                 if let Ok(unit_type) = UnitType::try_from_key(c_lower) {
        //                     tile.unit = Some({
        //                         let unit = Unit::new(
        //                             // UnitID::new(id),
        //                             next_unit_id,
        //                             loc,
        //                             unit_type,
        //                             Alignment::Belligerent{player: 1},
        //                             format!("Unit_{}_{}", loc.x, loc.y)
        //                         );
        //                         next_unit_id = next_unit_id.next();
        //                         unit
        //                     });

        //                     // Override the terrain to match the unit
        //                     tile.terrain = unit_type.default_terrain();
        //                 }
        //             }

        //             tile
        //         }
        //     )
        // )
    }
}

impl TryFrom<&'static str> for LocationGrid<Tile> {
    type Error = String;
    fn try_from(s: &'static str) -> Result<Self,String> {
        LocationGrid::try_from(String::from(s))
    }
}

/// Convert a multiline string into a grid of observations
/// A convenience method
/// For example:
/// `LocationGrid::try_from(
/// "?x x x\
///  ?I  xx\
///  x1  0a"
/// )`
/// would yield a location grid with tiles populated thus:
/// * question marks represent unobserved tiles
/// * non-question-mark characters represent tiles observed in turn 0:
///  - numerals represent land terrain with a city belonging to the player of that number
///    i.e. character "3" becomes a city belonging to player 3 located on land.
///  - letters corresponding to a UnitType key represent a unit of that type belonging to player 0
///  - letters whose lowercase corresponds to a UnitType key represent a unit of that type belonging to player 1
///  - other non-whitespace characters correspond to empty land
///  - whitespace characters correspond to water
///
/// Error if there are no lines or if the lines aren't of equal length
impl TryFrom<String> for LocationGrid<Obs> {
    type Error = String;
    fn try_from(s: String) -> Result<LocationGrid<Obs>,String> {
        let lines = Vec::from_iter( s.lines().map(|line| Vec::from_iter( line.chars() )) );

        let tile_grid: LocationGrid<Tile> = LocationGrid::try_from(s).unwrap();

        let obs_vecs: Vec<Vec<Obs>> = tile_grid.grid.iter()
                                                               .map(|col: &Vec<Tile>| {
            col.iter().cloned().map(|tile| {
                let loc = tile.loc;
                let c = lines[loc.y as usize][loc.x as usize];
                if c == '?' {
                    Obs::Unobserved
                } else {
                    Obs::Observed{tile, turn: 0, current: false}
                }
            }).collect()
        }).collect();

        Ok(LocationGrid::new_from_vec(tile_grid.dims(), obs_vecs))

        // if lines.is_empty() {
        //     return Err(String::from("String contained no lines"));
        // }

        // let width = lines[0].len();
        // if lines.len() == 1 && width == 0 {
        //     return Err(String::from("No map was provided (the string was empty)"));
        // }

        // for line in &lines {
        //     if line.len() != width {
        //         return Err(format!("Lines aren't all the same width. Expected {}, found {}", width, line.len()));
        //     }
        // }

        // let height = lines.len() as u16;

        // Ok(
        //     LocationGrid::new(
        //         Dims{width: width as u16, height },
        //         |loc| {
        //             let c = lines[loc.y as usize][loc.x as usize];
        //             if c == '?' {
        //                 Obs::Unobserved
        //                 // None
        //             } else {
        //                 let mut tile = Tile::new(
        //                     if c==' ' {
        //                         Terrain::Water
        //                     } else {
        //                         Terrain::Land
        //                     },
        //                     loc
        //                 );

        //                 let id: u64 = u64::from(loc.x * height + loc.y);
        //                 if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
        //                     tile.city = Some(City::new(
        //                         CityID::new(id),
        //                         Alignment::Belligerent{player: player_num},
        //                         loc,
        //                         format!("City_{}_{}", loc.x, loc.y)
        //                     ));
        //                 }
        //                 if let Ok(unit_type) = UnitType::try_from_key(c) {
        //                     tile.unit = Some(
        //                         Unit::new(
        //                             UnitID::new(id),
        //                             loc,
        //                             unit_type,
        //                             Alignment::Belligerent{player: 0},
        //                             format!("Unit_{}_{}", loc.x, loc.y)
        //                         )
        //                     );

        //                     // Override the terrain to match the unit
        //                     tile.terrain = unit_type.default_terrain();

        //                 } else if let Some(c_lower) = c.to_lowercase().next() {
        //                     if let Ok(unit_type) = UnitType::try_from_key(c_lower) {
        //                         tile.unit = Some(
        //                             Unit::new(
        //                                 UnitID::new(id),
        //                                 loc,
        //                                 unit_type,
        //                                 Alignment::Belligerent{player: 1},
        //                                 format!("Unit_{}_{}", loc.x, loc.y)
        //                             )
        //                         );

        //                         // Override the terrain to match the unit
        //                         tile.terrain = unit_type.default_terrain();
                                
        //                     }
        //                 }

        //                 // Obs::Observed{tile, turn: 0}
        //                 Obs::Observed{ tile, turn: 0, current: false }
        //             }
        //         }
        //     )
        // )
    }
}

impl TryFrom<&'static str> for LocationGrid<Obs> {
    type Error = String;
    fn try_from(s: &'static str) -> Result<LocationGrid<Obs>,String> {
        LocationGrid::try_from(String::from(s))
    }
}

#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    use crate::{
        game::{
            obs::Obs,
            map::{Terrain,Tile},
        },
        util::{Dims,Location},
    };

    use super::LocationGrid;

    #[test]
    fn test_str_to_tile_map() {
        if let Ok(_map) = LocationGrid::<Tile>::try_from("") {
            panic!("Empty string should be an error");
        }

        match LocationGrid::<Tile>::try_from("   \n   ") {
            Err(err) => {
                panic!("Error parsing grid string: {}", err);
            },
            Ok(map) => {
                assert_eq!(map.dims, Dims{width: 3, height: 2});
            }
        }

        match LocationGrid::<Tile>::try_from(
            ".... .\n\
             ... ..\n\
             ..   .") {
            Err(err) => {
                panic!("Error parsing grid string: {}", err);
            },
            Ok(map) => {
                assert_eq!(map.dims.width, 6);
                assert_eq!(map.dims.height, 3);

                assert_eq!(map[Location{x:0,y:0}], Tile{ terrain: Terrain::Land, loc: Location{x:0,y:0}, city: None, unit: None });
                assert_eq!(map[Location{x:1,y:0}], Tile{ terrain: Terrain::Land, loc: Location{x:1,y:0}, city: None, unit: None });
                assert_eq!(map[Location{x:2,y:0}], Tile{ terrain: Terrain::Land, loc: Location{x:2,y:0}, city: None, unit: None });
                assert_eq!(map[Location{x:3,y:0}], Tile{ terrain: Terrain::Land, loc: Location{x:3,y:0}, city: None, unit: None });
                assert_eq!(map[Location{x:4,y:0}], Tile{ terrain: Terrain::Water, loc: Location{x:4,y:0}, city: None, unit: None });
                assert_eq!(map[Location{x:5,y:0}], Tile{ terrain: Terrain::Land, loc: Location{x:5,y:0}, city: None, unit: None });

                assert_eq!(map[Location{x:0,y:1}], Tile{ terrain: Terrain::Land, loc: Location{x:0,y:1}, city: None, unit: None });
                assert_eq!(map[Location{x:1,y:1}], Tile{ terrain: Terrain::Land, loc: Location{x:1,y:1}, city: None, unit: None });
                assert_eq!(map[Location{x:2,y:1}], Tile{ terrain: Terrain::Land, loc: Location{x:2,y:1}, city: None, unit: None });
                assert_eq!(map[Location{x:3,y:1}], Tile{ terrain: Terrain::Water, loc: Location{x:3,y:1}, city: None, unit: None });
                assert_eq!(map[Location{x:4,y:1}], Tile{ terrain: Terrain::Land, loc: Location{x:4,y:1}, city: None, unit: None });
                assert_eq!(map[Location{x:5,y:1}], Tile{ terrain: Terrain::Land, loc: Location{x:5,y:1}, city: None, unit: None });

                assert_eq!(map[Location{x:0,y:2}], Tile{ terrain: Terrain::Land, loc: Location{x:0,y:2}, city: None, unit: None });
                assert_eq!(map[Location{x:1,y:2}], Tile{ terrain: Terrain::Land, loc: Location{x:1,y:2}, city: None, unit: None });
                assert_eq!(map[Location{x:2,y:2}], Tile{ terrain: Terrain::Water, loc: Location{x:2,y:2}, city: None, unit: None });
                assert_eq!(map[Location{x:3,y:2}], Tile{ terrain: Terrain::Water, loc: Location{x:3,y:2}, city: None, unit: None });
                assert_eq!(map[Location{x:4,y:2}], Tile{ terrain: Terrain::Water, loc: Location{x:4,y:2}, city: None, unit: None });
                assert_eq!(map[Location{x:5,y:2}], Tile{ terrain: Terrain::Land, loc: Location{x:5,y:2}, city: None, unit: None });
            }
        }
    }

    #[test]
    fn test_str_to_obs_map() {
        LocationGrid::<Obs>::try_from(
            "\
            *..\n\
            ???\n\
            ...").unwrap();


        if let Ok(_map) = LocationGrid::<Tile>::try_from("") {
            panic!("Empty string should be an error");
        }

        match LocationGrid::<Tile>::try_from("   \n   ") {
            Err(err) => {
                panic!("Error parsing grid string: {}", err);
            },
            Ok(map) => {
                assert_eq!(map.dims, Dims{width: 3, height: 2});
            }
        }

        match LocationGrid::<Obs>::try_from(
            ".... .\n\
             ..? ..\n\
             .. ? .") {
            Err(err) => {
                panic!("Error parsing grid string: {}", err);
            },
            Ok(map) => {
                assert_eq!(map.dims.width, 6);
                assert_eq!(map.dims.height, 3);

                assert_eq!(map[Location{x:0,y:0}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:0,y:0}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:1,y:0}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:1,y:0}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:2,y:0}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:2,y:0}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:3,y:0}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:3,y:0}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:4,y:0}], Obs::Observed{tile: Tile{ terrain: Terrain::Water, loc: Location{x:4,y:0}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:5,y:0}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:5,y:0}, city: None, unit: None }, turn: 0, current: false});

                assert_eq!(map[Location{x:0,y:1}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:0,y:1}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:1,y:1}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:1,y:1}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:2,y:1}], Obs::Unobserved);
                assert_eq!(map[Location{x:3,y:1}], Obs::Observed{tile: Tile{ terrain: Terrain::Water, loc: Location{x:3,y:1}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:4,y:1}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:4,y:1}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:5,y:1}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:5,y:1}, city: None, unit: None }, turn: 0, current: false});

                assert_eq!(map[Location{x:0,y:2}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:0,y:2}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:1,y:2}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:1,y:2}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:2,y:2}], Obs::Observed{tile: Tile{ terrain: Terrain::Water, loc: Location{x:2,y:2}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:3,y:2}], Obs::Unobserved);
                assert_eq!(map[Location{x:4,y:2}], Obs::Observed{tile: Tile{ terrain: Terrain::Water, loc: Location{x:4,y:2}, city: None, unit: None }, turn: 0, current: false});
                assert_eq!(map[Location{x:5,y:2}], Obs::Observed{tile: Tile{ terrain: Terrain::Land, loc: Location{x:5,y:2}, city: None, unit: None }, turn: 0, current: false});
            }
        }
    }

    #[test]
    fn test_iter() {
        let terrains: [Terrain; 9] = [
            Terrain::Land, Terrain::Land, Terrain::Land,
            Terrain::Land, Terrain::Water, Terrain::Land,
            Terrain::Land, Terrain::Land, Terrain::Land
        ];
        let locs: [Location; 9] = [
            Location{x:0,y:0}, Location{x:0,y:1}, Location{x:0,y:2},
            Location{x:1,y:0}, Location{x:1,y:1}, Location{x:1,y:2},
            Location{x:2,y:0}, Location{x:2,y:1}, Location{x:2,y:2}
        ];

        let grid: LocationGrid<Tile> = LocationGrid::try_from("...\n. .\n...").unwrap();

        let mut count = 0;
        for (i, tile) in grid.iter().enumerate() {
            println!("{:?}", tile);
            count += 1;
            assert_eq!(terrains[i], tile.terrain);
            assert_eq!(locs[i], tile.loc);
            assert_eq!(None, tile.unit);
            assert_eq!(None, tile.city);
        }
        assert_eq!(9, count);
    }
}
