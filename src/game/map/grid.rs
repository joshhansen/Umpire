use std::{
    collections::HashMap,
    convert::TryFrom,
    fmt,
    iter::FromIterator,
    ops::{
        Index,
        IndexMut,
    },
};


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

pub trait LocationGridI<T> : Dimensioned + Index<Location,Output=T> {
    fn get(&self, loc: Location) -> Option<&T>;

    fn get_mut(&mut self, loc: Location) -> Option<&mut T>;

    fn replace(&mut self, loc: Location, value: T) -> Option<T>;
}

// NOTE This is a dense representation and really doesn't handle large maps well, e.g. 10000x10000
#[derive(Clone)]
pub struct LocationGrid<T> {
    /// The values stored in column-major order
    /// 
    /// Look up locations thus:
    ///      grid[col * dims.height + row]
    /// i.e. grid[x * dims.height + y]
    grid: Vec<T>,
    dims: Dims,
}

impl<T> LocationGrid<T> {
    /// Make a new location grid from values provided in column-major order
    pub fn new_from_vec(dims: Dims, grid: Vec<T>) -> Self {
        Self{ grid, dims }
    }

    pub fn new<I>(dims: Dims, initializer: I) -> Self
        where I : Fn(Location) -> T {

        let mut grid = Vec::with_capacity(dims.area() as usize);
        for loc in dims.iter_locs_column_major() {
            grid.push(initializer(loc));
        }

        debug_assert_eq!(grid.len(), dims.area() as usize);
    
        Self{ grid, dims }
    }

    pub fn iter(&self) -> impl Iterator<Item=&T> {
        self.grid.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item=&mut T> {
        self.grid.iter_mut()
    }

    pub fn iter_locs(&self) -> impl Iterator<Item=Location> {
        self.dims.iter_locs()
    }
}

impl <T> LocationGridI<T> for LocationGrid<T> {
    fn get(&self, loc: Location) -> Option<&T> {
        if !self.dims.contain(loc) {
            return None;
        }

        self.grid.get((loc.x * self.dims.height + loc.y) as usize)
    }

    fn get_mut(&mut self, loc: Location) -> Option<&mut T> {
        if !self.dims.contain(loc) {
            return None;
        }

        self.grid.get_mut((loc.x * self.dims.height + loc.y) as usize)
    }

    fn replace(&mut self, loc: Location, value: T) -> Option<T> {
        debug_assert!(self.dims.contain(loc));

        self.grid.get_mut((loc.x * self.dims.height + loc.y) as usize)
                 .map(|v| std::mem::replace(v, value))
    }
}

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
        if let Some(obs) = LocationGridI::get(self, loc) {
            obs
        } else {
            &Obs::Unobserved
        }
    }
}

impl<T> Index<Location> for LocationGrid<T> {
    type Output = T;
    fn index(&self, loc: Location) -> &T {
        &self.grid[(loc.x * self.dims.height + loc.y) as usize]
    }
}

impl<T> IndexMut<Location> for LocationGrid<T> {
    fn index_mut(&mut self, loc: Location) -> &mut T {
        &mut self.grid[(loc.x * self.dims.height + loc.y) as usize]
    }
}

/// NOTE: this impl is identical to the Debug impl on SparseLocationGrid
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
    fn try_from(s: String) -> Result<Self,String> {
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
    fn try_from(s: String) -> Result<Self,String> {
        let lines = Vec::from_iter( s.lines().map(|line| Vec::from_iter( line.chars() )) );

        let tile_grid: LocationGrid<Tile> = LocationGrid::try_from(s).unwrap();
        let dims = tile_grid.dims;

        let obs_vecs: Vec<Obs> = tile_grid.grid.into_iter()
                                               .map(|tile: Tile| {
            let loc = tile.loc;
            let c = lines[loc.y as usize][loc.x as usize];
            if c == '?' {
                Obs::Unobserved
            } else {
                Obs::Observed{tile, turn: 0, current: false}
            }
        }).collect();

        Ok(LocationGrid::new_from_vec(dims, obs_vecs))
    }
}

impl TryFrom<&'static str> for LocationGrid<Obs> {
    type Error = String;
    fn try_from(s: &'static str) -> Result<Self,String> {
        Self::try_from(String::from(s))
    }
}

#[derive(Clone)]
pub struct SparseLocationGrid<T> {
    grid: HashMap<Location,T>,
    dims: Dims
}

impl <T> SparseLocationGrid<T> {
    pub fn new(dims: Dims) -> Self {
        Self {
            grid: HashMap::new(),
            dims,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=&T> {
        self.grid.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item=&mut T> {
        self.grid.values_mut()
    }
}

impl <T> Dimensioned for SparseLocationGrid<T> {
    fn dims(&self) -> Dims {
        self.dims
    }
}

impl <T> Index<Location> for SparseLocationGrid<T> {
    type Output = T;

    fn index(&self, loc: Location) -> &Self::Output {
        self.get(loc).unwrap()
    }
}

impl<T> IndexMut<Location> for SparseLocationGrid<T> {
    fn index_mut(&mut self, loc: Location) -> &mut T {
        self.get_mut(loc).unwrap()
    }
}

impl <T> LocationGridI<T> for SparseLocationGrid<T> {
    fn get(&self, loc: Location) -> Option<&T> {
        self.grid.get(&loc)
    }

    fn get_mut(&mut self, loc: Location) -> Option<&mut T> {
        self.grid.get_mut(&loc)
    }

    fn replace(&mut self, loc: Location, value: T) -> Option<T> {
        self.grid.insert(loc, value)
    }
}

/// NOTE: this impl is identical to the Debug impl on LocationGrid
impl <T:fmt::Debug> fmt::Debug for SparseLocationGrid<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = write!(f, "");

        for y in 0..self.dims.height {
            for x in 0..self.dims.width {
                let value = self.get(Location::new(x,y));
                if let Some(value) = value {
                    result = result.and(value.fmt(f));
                } else {
                    result = result.and(write!(f, " "));
                }
                // result = result.and(self[Location::new(x,y)].fmt(f));
            }
            result = result.and(writeln!(f));
        }

        result
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

    use super::{
        LocationGrid,
        LocationGridI,
    };

    #[test]
    fn test_grid() {
        let mut grid = LocationGrid::new(
            Dims::new(10, 20),
            |_| 0
        );

        assert!(grid.iter().all(|x| *x==0));

        assert_eq!(grid.replace(Location::new(5, 6), 100), Some(0));

        assert_eq!(grid.iter().filter(|x| **x==100).count(), 1);

        assert_eq!(grid.get(Location::new(50, 1000)), None);

        assert_eq!(grid.get_mut(Location::new(10, 20)), None);

    }

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

    #[test]
    fn test_layout() {
        let v = vec![
            Location::new(0, 0),
            Location::new(0, 1),
            Location::new(0, 2),
            Location::new(0, 3),
            Location::new(0, 4),
            Location::new(1, 0),
            Location::new(1, 1),
            Location::new(1, 2),
            Location::new(1, 3),
            Location::new(1, 4),
            Location::new(2, 0),
            Location::new(2, 1),
            Location::new(2, 2),
            Location::new(2, 3),
            Location::new(2, 4),
        ];

        let grid = LocationGrid::new_from_vec(Dims::new(3, 5), v);

        for loc in grid.dims.iter_locs() {
            assert_eq!(loc, grid[loc]);
        }
    }
}
