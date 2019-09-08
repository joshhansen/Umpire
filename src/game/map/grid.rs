use std::convert::TryFrom;
use std::fmt;
use std::iter::FromIterator;
use std::ops::{Index,IndexMut};

use crate::{
    game::{
        Alignment,
        PlayerNum,
        map::{
            Terrain,
            Tile,
            dijkstra::Source,
            newmap::CityID,
        },
        obs::Obs,
        unit::City,
    },
    util::{Dims,Location},
};

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
        if let Some(col) = self.grid.get(loc.x as usize) {
            col.get(loc.y as usize)
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, loc: Location) -> Option<&mut T> {
        if let Some(col) = self.grid.get_mut(loc.x as usize) {
            col.get_mut(loc.y as usize)
        } else {
            None
        }
    }

    pub fn dims(&self) -> Dims {
        self.dims
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item=&'a T> {
        self.grid.iter().flat_map(|item: &Vec<T>| item.iter())
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item=&'a mut T> {
        self.grid.iter_mut().flat_map(|item: &mut Vec<T>| item.iter_mut())
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

impl Source<Tile> for LocationGrid<Tile> {
    fn get(&self, loc: Location) -> &Tile {
        self.get(loc).unwrap()
    }
    fn dims(&self) -> Dims {
        self.dims
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
    fn dims(&self) -> Dims {
        self.dims
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

        for j in 0..self.dims.height {
            let j_ = self.dims.height - j - 1;

            for i in 0..self.dims.width {
                if i > 0 {
                    result = result.and(write!(f, "\t"));
                }
                result = result.and(self[Location{x:j_, y:i}].fmt(f));
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
///  xx  xx\
///  x1  0x"
/// )`
/// would yield a location grid with tiles populated thus:
/// * numerals represent land terrain with a city belonging to the player of that number
///   i.e. character "3" becomes a city belonging to player 3 located on land.
/// * other non-whitespace characters correspond to land
/// * whitespace characters correspond to water
///
/// Error if there are no lines or if the lines aren't of equal length
impl TryFrom<&'static str> for LocationGrid<Tile> {
    type Error = String;
    fn try_from(str: &'static str) -> Result<LocationGrid<Tile>,String> {
        let lines = Vec::from_iter( str.lines().map(|line| Vec::from_iter( line.chars() )) );
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

        Ok(
            LocationGrid::new(
                Dims{width: width as u16, height: height as u16 },
                |loc| {
                    let c = lines[loc.y as usize][loc.x as usize];
                    let mut tile = Tile::new(
                        if c==' ' {
                            Terrain::Water
                        } else {
                            Terrain::Land
                        },
                        loc
                    );
                    let id: u64 = u64::from(loc.x * height + loc.y);
                    if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
                        tile.city = Some(City::new(
                            CityID::new(id),
                            Alignment::Belligerent{player: player_num},
                            loc,
                            format!("City_{}_{}", loc.x, loc.y)
                        ));
                    }
                    tile
                }
            )
        )
    }
}

/// Convert a multiline string into a grid of observations
/// A convenience method
/// For example:
/// `LocationGrid::try_from(
/// "?x x x\
///  ?x  xx\
///  x1  0x"
/// )`
/// would yield a location grid with tiles populated thus:
/// * question marks represent unobserved tiles
/// * non-question-mark characters represent tiles observed in turn 0:
///   - numerals represent land terrain with a city belonging to the player of that number i.e.
///     character "3" becomes a city belonging to player 3 located on land.
///   - other non-whitespace characters correspond to land
///   - whitespace characters correspond to water
///
/// Error if there are no lines or if the lines aren't of equal length
impl TryFrom<&'static str> for LocationGrid<Obs> {
    type Error = String;
    fn try_from(str: &'static str) -> Result<LocationGrid<Obs>,String> {
        let lines = Vec::from_iter( str.lines().map(|line| Vec::from_iter( line.chars() )) );
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

        Ok(
            LocationGrid::new(
                Dims{width: width as u16, height },
                |loc| {
                    let c = lines[loc.y as usize][loc.x as usize];
                    if c == '?' {
                        Obs::Unobserved
                        // None
                    } else {
                        let mut tile = Tile::new(
                            if c==' ' {
                                Terrain::Water
                            } else {
                                Terrain::Land
                            },
                            loc
                        );

                        let id: u64 = u64::from(loc.x * height + loc.y);
                        if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
                            tile.city = Some(City::new(
                                CityID::new(id),
                                Alignment::Belligerent{player: player_num},
                                loc,
                                format!("City_{}_{}", loc.x, loc.y)
                            ));
                        }

                        // Obs::Observed{tile, turn: 0}
                        Obs::Observed{ tile, turn: 0, current: false }
                    }
                }
            )
        )
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
            assert!(false, "Empty string should be an error");
        }

        match LocationGrid::<Tile>::try_from("   \n   ") {
            Err(_) => {
                assert!(false, "String should have parsed");
            },
            Ok(map) => {
                assert_eq!(map.dims, Dims{width: 3, height: 2});
            }
        }

        match LocationGrid::<Tile>::try_from(
            "blah h\n\
             zzz zz\n\
             zz   z") {
            Err(_) => {
                assert!(false, "Any other string should be ok");
            },
            Ok(map) => {
                assert_eq!(map.dims.width, 6);
                assert_eq!(map.dims.height, 3);

                assert_eq!(map[Location{x:0,y:0}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:1,y:0}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:2,y:0}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:3,y:0}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:4,y:0}].terrain, Terrain::Water);
                assert_eq!(map[Location{x:5,y:0}].terrain, Terrain::Land);

                assert_eq!(map[Location{x:0,y:1}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:1,y:1}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:2,y:1}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:3,y:1}].terrain, Terrain::Water);
                assert_eq!(map[Location{x:4,y:1}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:5,y:1}].terrain, Terrain::Land);

                assert_eq!(map[Location{x:0,y:2}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:1,y:2}].terrain, Terrain::Land);
                assert_eq!(map[Location{x:2,y:2}].terrain, Terrain::Water);
                assert_eq!(map[Location{x:3,y:2}].terrain, Terrain::Water);
                assert_eq!(map[Location{x:4,y:2}].terrain, Terrain::Water);
                assert_eq!(map[Location{x:5,y:2}].terrain, Terrain::Land);
            }
        }
    }

    #[test]
    fn test_str_to_obs_map() {
        let map: LocationGrid<Obs> = LocationGrid::try_from(
            "\
            *xx\n\
            ???\n\
            xxx").unwrap();


        if let Ok(_map) = LocationGrid::<Tile>::try_from("") {
            assert!(false, "Empty string should be an error");
        }

        match LocationGrid::<Tile>::try_from("   \n   ") {
            Err(_) => {
                assert!(false, "String should have parsed");
            },
            Ok(map) => {
                assert_eq!(map.dims, Dims{width: 3, height: 2});
            }
        }

        match LocationGrid::<Obs>::try_from(
            "blah h\n\
             zz? zz\n\
             zz ? z") {
            Err(_) => {
                assert!(false, "Any other string should be ok");
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

        let grid: LocationGrid<Tile> = LocationGrid::try_from("abc\nd f\nhij").unwrap();

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
