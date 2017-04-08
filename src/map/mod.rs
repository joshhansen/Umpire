use std::convert::TryFrom;
use std::fmt;
use std::iter::{FlatMap,FromIterator};
use std::slice::Iter;
use std::ops::{Index,IndexMut};

use termion::color::AnsiValue;

use unit::{Aligned,Alignment,City,PlayerNum,Sym,Unit};
use util::{Dims,Location};


#[derive(Clone,PartialEq)]
pub enum Terrain {
    Water,
    Land,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

impl Terrain {
    pub fn color(&self) -> AnsiValue {
        match *self {
            Terrain::Water => AnsiValue(12),
            Terrain::Land => AnsiValue(10),
            // Terrain::CITY => AnsiValue(245)
        }
    }
}

impl fmt::Display for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Terrain::Water => "Water",
            Terrain::Land => "Land"
        })
    }
}

impl fmt::Debug for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Clone,Debug,PartialEq)]
pub struct Tile {
    pub terrain: Terrain,
    pub unit: Option<Unit>,
    pub city: Option<City>,
    pub loc: Location
}

impl Tile {
    pub fn new(terrain: Terrain, loc: Location) -> Tile {
        Tile{ terrain: terrain, unit: None, city: None, loc: loc }
    }

    pub fn sym(&self) -> &'static str {
        if let Some(ref unit) = self.unit {
            unit.sym()
        } else {
            if let Some(ref city) = self.city {
                city.sym()
            } else {
                " "
            }
        }
    }

    pub fn fg_color(&self) -> Option<AnsiValue> {
        match self.unit {
            Some(ref last_unit) => Some(last_unit.alignment.color()),
            None => match self.city {
                Some(ref city) => Some(city.alignment().color()),
                None => None
            }
        }
    }

    pub fn bg_color(&self) -> AnsiValue {
        self.terrain.color()
    }

    pub fn pop_unit(&mut self) -> Option<Unit> {
        let unit = self.unit.clone();
        self.unit = None;
        unit
    }

    pub fn set_unit(&mut self, unit: Unit) {
        self.unit = Some(unit);
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref city) = self.city {
            if let Some(ref unit) = self.unit {
                write!(f, "{} with {} garrisoned", city, unit)
            } else {
                write!(f, "{}", city)
            }
        } else {
            if let Some(ref unit) = self.unit {
                write!(f, "{} on {}", unit, self.terrain)
            } else {
                write!(f, "{}", self.terrain)
            }
        }
    }
}

pub struct LocationGrid<T> {
    grid: Vec<Vec<T>>,//grid[col i.e. x][row i.e. y]
    dims: Dims
}

impl<T> LocationGrid<T> {
    fn new_from_vec(dims: Dims, grid: Vec<Vec<T>>) -> Self {
        LocationGrid{ grid: grid, dims: dims }
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

        LocationGrid{ grid: grid, dims: dims }
    }

    pub fn get<'a>(&'a self, loc: &Location) -> Option<&'a T> {
        if let Some(col) = self.grid.get(loc.x as usize) {
            col.get(loc.y as usize)
        } else {
            None
        }
    }

    pub fn dims(&self) -> Dims {
        self.dims
    }
}

impl LocationGrid<Tile> {
    fn map1(item: &Vec<Tile>) -> Iter<Tile> {
        item.iter()
    }

    pub fn iter(&self) -> LocationGridIter {
        LocationGridIter {
            iter: self.grid.iter().flat_map(LocationGrid::map1)
        }
    }
}

pub struct LocationGridIter<'a> {
    iter: FlatMap<Iter<'a, Vec<Tile>>, Iter<'a, Tile>, fn(&Vec<Tile>) -> Iter<Tile> >
}
impl <'b> Iterator for LocationGridIter<'b> {
    type Item = &'b Tile;
    fn next(&mut self) -> Option<&'b Tile> {
        self.iter.next()
    }
}

impl<T> Index<Location> for LocationGrid<T> {
    type Output = T;
    fn index<'a>(&'a self, location: Location) -> &'a T {
        &self.grid[location.x as usize][location.y as usize]
    }
}

impl<T> IndexMut<Location> for LocationGrid<T> {
    fn index_mut<'a>(&'a mut self, location: Location) -> &'a mut T {
        let col:  &mut Vec<T> = self.grid.get_mut(location.x as usize).unwrap();
        col.get_mut(location.y as usize).unwrap()
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
            result = result.and(write!(f, "\n"));
        }

        result
    }
}

/// Convert a multiline string into a map
/// A convenience method
/// For example:
/// LocationGrid::try_from(
/// "xx x x\
///  xx  xx\
///  x    x"
/// )
/// would yield a location grid with tiles populated thus:
/// * numerals represent land terrain with a city belonging to the player of that number
///   i.e. character "3" becomes a city belonging to player 3 located on land.
/// * other non-whitespace characters correspond to land
/// * whitespace characters correspond to water
///
/// Error if there are no lines or if the lines aren't of equal length
impl TryFrom<&'static str> for LocationGrid<Tile> {
    type Err = String;
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

        Ok(
            LocationGrid::new(
                Dims{width: width as u16, height: lines.len() as u16 },
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
                    if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
                        tile.city = Some(City::new(
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

#[test]
fn test_str_to_map() {
    if let Ok(_map) = LocationGrid::try_from("") {
        assert!(false, "Empty string should be an error");
    }

    match LocationGrid::try_from("   \n   ") {
        Err(_) => {
            assert!(false, "String should have parsed");
        },
        Ok(map) => {
            assert_eq!(map.dims, Dims{width: 3, height: 2});
        }
    }

    match LocationGrid::try_from(
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

pub mod dijkstra;
pub mod gen;
#[cfg(test)]
mod test;

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

    let grid = LocationGrid::try_from("abc\nd f\nhij").unwrap();

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
