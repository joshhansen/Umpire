extern crate termion;

use std::convert::TryFrom;
use std::fmt;
use std::iter::FromIterator;
use std::ops::{Index,IndexMut};

use termion::color::AnsiValue;

use unit::{Alignment,City,Unit,Aligned,Sym};
use util::{Dims,Location,Wrapping};


#[derive(Clone,PartialEq)]
pub enum Terrain {
    WATER,
    LAND,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

impl Terrain {
    pub fn color(&self) -> AnsiValue {
        match *self {
            Terrain::WATER => AnsiValue(12),
            Terrain::LAND => AnsiValue(10),
            // Terrain::CITY => AnsiValue(245)
        }
    }
}

impl fmt::Debug for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Terrain::WATER => "Water",
            Terrain::LAND => "Land"
        })
    }
}

#[derive(Clone,Debug)]
pub struct Tile {
    pub terrain: Terrain,
    pub unit: Option<Unit>,
    pub city: Option<City>,
    pub loc: Location
}

impl Tile {
    fn new(terrain: Terrain, loc: Location) -> Tile {
        Tile{ terrain: terrain, unit: None, city: None, loc: loc }
    }

    pub fn sym(&self) -> char {
        match self.unit {
            None => match self.city {
                None => ' ',
                Some(ref city) => city.sym()
            },
            Some(ref unit) => unit.sym()
        }
    }

    pub fn alignment(&self) -> Option<Alignment> {
        match self.unit {
            None => match self.city {
                None => None,
                Some(ref city) => Some(city.alignment())
            },
            Some(ref unit) => Some(unit.alignment)
        }
    }

    pub fn fg_color(&self) -> Option<AnsiValue> {
        match self.unit {
            None => match self.city {
                None => None,
                Some(ref city) => Some(city.alignment().color())
            },
            Some(ref last_unit) => Some(last_unit.alignment.color())
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

pub struct LocationGrid<T> {
    grid: Vec<Vec<T>>,
    dims: Dims
}

impl<T> LocationGrid<T> {
    fn new_from_vec(dims: &Dims, grid: Vec<Vec<T>>) -> Self {
        LocationGrid{ grid: grid, dims: *dims }
    }

    fn new<I>(dims: &Dims, initializer: I) -> Self
        where I : Fn(&Location) -> T {
        let mut grid: Vec<Vec<T>> = Vec::new();

        let mut loc = Location{x:0, y:0};

        for x in 0..dims.width {
            loc.x = x;

            let mut col: Vec<T> = Vec::new();
            for y in 0..dims.height {
                loc.y = y;

                col.push(initializer(&loc));
            }

            grid.push(col);
        }

        LocationGrid{ grid: grid, dims: *dims }
    }

    fn get_wrapped(&self, loc: &Location, wrapping: &Wrapping) -> Option<T> {
        None//FIXME
    }

    pub fn get<'a>(&'a self, loc: &Location) -> Option<&'a T> {
        if let Some(col) = self.grid.get(loc.x as usize) {
            col.get(loc.y as usize)
        } else {
            None
        }
    }

    pub fn get_mut<'a>(&'a mut self, loc: &Location) -> Option<&'a mut T> {
        if let Some(col) = self.grid.get_mut(loc.x as usize) {
            col.get_mut(loc.y as usize)
        } else {
            None
        }
    }
}

impl LocationGrid<Tile> {
    pub fn iter(&self) -> LocationGridIterator {
        LocationGridIterator{loc: Location{x: 0, y: 0}, loc_grid: &self}
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

// impl IntoIterator for LocationGrid<Tile> {
//     type Item = (Location,Tile);
//     type IntoIter = LocationGridIterator;
//
//     fn into_iter(self) -> Self::IntoIter {
//         LocationGridIterator{x: 0, y: 0}
//     }
// }

pub struct LocationGridIterator<'a> {
    loc: Location,
    loc_grid: &'a LocationGrid<Tile>
}

impl <'b> Iterator for LocationGridIterator<'b> {
    type Item = (Location,&'b Tile);
    fn next(&mut self) -> Option<(Location,&'b Tile)> {
        /*
            If the location is invalid, return None
            Get the value from the current location
            Step location forward
            return the value
        */
        if let Some(tile) = self.loc_grid.get(&self.loc) {

            let result = Some((self.loc, tile));

            self.loc.y += 1;
            if self.loc.y >= self.loc_grid.dims.height {
                self.loc.y = 0;
                self.loc.x += 1;
            }

            result
        } else {
            None
        }
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
/// would yield a location grid with tiles populated such that each non-whitespace character
/// corresponds to land and each whitespace character corresponds to ocean
///
/// Error if there are no lines or if the lines aren't of equal length
impl TryFrom<&'static str> for LocationGrid<Tile> {
    type Err = &'static str;
    fn try_from(str: &'static str) -> Result<LocationGrid<Tile>,&'static str> {
        let lines = Vec::from_iter( str.lines().map(|line| Vec::from_iter( line.chars() )) );
        if lines.is_empty() {
            return Err("String contained no lines");
        }

        let width = lines[0].len();

        if lines.len() == 1 && width == 0 {
            return Err("No map was provided (the string was empty)");
        }

        for line in &lines {
            if line.len() != width {
                return Err("Lines aren't all the same width");
            }
        }

        Ok(
            LocationGrid::new(
                &Dims{width: width as u16, height: lines.len() as u16 },
                |loc| Tile::new(
                    if lines[loc.y as usize][loc.x as usize]==' ' {
                        Terrain::WATER
                    } else {
                        Terrain::LAND
                    },
                    *loc
                )

            )
        )

        // Ok(LocationGrid::new(
        //     &Dims{width: 5, height: 5},
        //     |loc| Tile::new(Terrain::WATER, *loc)
        // ))
    }
}

#[test]
fn test_str_to_map() {
    if let Ok(_map) = LocationGrid::try_from("") {
        assert!(false, "Empty string should be an error");
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

            assert_eq!(map[Location{x:0,y:0}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:1,y:0}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:2,y:0}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:3,y:0}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:4,y:0}].terrain, Terrain::WATER);
            assert_eq!(map[Location{x:5,y:0}].terrain, Terrain::LAND);

            assert_eq!(map[Location{x:0,y:1}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:1,y:1}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:2,y:1}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:3,y:1}].terrain, Terrain::WATER);
            assert_eq!(map[Location{x:4,y:1}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:5,y:1}].terrain, Terrain::LAND);

            assert_eq!(map[Location{x:0,y:2}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:1,y:2}].terrain, Terrain::LAND);
            assert_eq!(map[Location{x:2,y:2}].terrain, Terrain::WATER);
            assert_eq!(map[Location{x:3,y:2}].terrain, Terrain::WATER);
            assert_eq!(map[Location{x:4,y:2}].terrain, Terrain::WATER);
            assert_eq!(map[Location{x:5,y:2}].terrain, Terrain::LAND);
        }
    }
}

pub type Tiles = LocationGrid<Tile>;

pub mod gen;
mod test;

#[test]
fn test_iter() {
    let grid = LocationGrid::try_from("abc\ndef\nhij").unwrap();
    for (loc, tile) in grid.iter() {
        println!("{} {:?}", loc, tile);
    }
}
