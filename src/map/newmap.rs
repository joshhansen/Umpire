use std::collections::HashMap;
use std::convert::TryFrom;
use std::iter::FromIterator;

use map::{Terrain,Tile};
use map::dijkstra::Source;
use map::grid::LocationGrid;
use util::{Dims,Location};
use unit::{Alignment,City,PlayerNum,Unit,UnitType};

#[derive(Clone,Copy,Debug,Eq,Hash,PartialEq)]
pub struct CityID {
    id: u64
}
impl CityID {
    pub fn new(id: u64) -> Self {
        Self{ id }
    }
    fn next(&self) -> Self {
        Self{ id: self.id + 1 }
    }
}

#[derive(Clone,Copy,Debug,Eq,Hash,PartialEq)]
pub struct UnitID {
    id: u64
}
impl UnitID {
    pub fn new(id: u64) -> Self {
        Self{ id }
    }
    fn next(&self) -> Self {
        UnitID{ id: self.id + 1 }
    }
}

// pub struct MapData {
//     dims: Dims,
//     terrain: LocationGrid<Terrain>,
//     units_by_id: HashMap<UnitID,Unit>,
//     cities_by_id: HashMap<CityID,City>,
//     unit_ids_by_loc: HashMap<Location,UnitID>,
//     city_ids_by_loc: HashMap<Location,CityID>,

//     // player_observations: HashMap<PlayerNum,Box<ObsTracker>>,

//     next_unit_id: UnitID,
//     next_city_id: CityID
// }

// impl MapData {

//     pub fn new(dims: Dims) -> Self {
//         Self {
//             dims: dims,
//             terrain: LocationGrid::new(dims, |_loc| Terrain::Water),
//             units_by_id: HashMap::new(),
//             cities_by_id: HashMap::new(),
//             unit_ids_by_loc: HashMap::new(),
//             city_ids_by_loc: HashMap::new(),
//             next_unit_id: UnitID{id: 0},
//             next_city_id: CityID{id: 0}
//         }
//     }

//     pub fn dims(&self) -> Dims {
//         self.dims
//     }

//     fn in_bounds(&self, loc: Location) -> bool {
//         self.dims.in_bounds(loc)
//     }

//     pub fn terrain(&self, loc: Location) -> Option<&Terrain> {
//         self.terrain.get(loc)
//     }

//     pub fn set_terrain(&mut self, loc: Location, terrain: Terrain) {
//         self.terrain[loc] = terrain;
//     }


//     pub fn new_unit<S:Into<String>>(&mut self, loc: Location, type_: UnitType, alignment: Alignment, name: S) -> (UnitID,&Unit) {
//         let unit_id = self.next_unit_id;
//         self.next_unit_id = self.next_unit_id.next();

//         let unit: Unit = Unit::new(unit_id, loc, type_, alignment, name);

//         let insertion_result1 = self.units_by_id.insert(unit_id, unit);
//         debug_assert!(insertion_result1.is_none());

//         let insertion_result2 = self.unit_ids_by_loc.insert(loc, unit_id);
//         debug_assert!(insertion_result2.is_none());

//         (unit_id, self.units_by_id.get(&unit_id).unwrap())
//     }

//     pub fn unit_by_loc(&self, loc: Location) -> Option<&Unit> {
//         if let Some(unit_id) = self.unit_ids_by_loc.get(&loc) {
//             self.units_by_id.get(unit_id)
//         } else {
//             None
//         }
//     }

//     pub fn mut_unit_by_loc(&mut self, loc: Location) -> Option<&mut Unit> {
//         if let Some(unit_id) = self.unit_ids_by_loc.get(&loc) {
//             self.units_by_id.get_mut(unit_id)
//         } else {
//             None
//         }
//     }

//     pub fn pop_unit_by_loc(&mut self, loc: Location) -> Option<Unit> {
//         if let Some(unit_id) = self.unit_ids_by_loc.remove(&loc) {
//             self.units_by_id.remove(&unit_id)
//         } else {
//             None
//         }
//     }

//     pub fn unit_by_id(&self, id: UnitID) -> Option<&Unit> {
//         self.units_by_id.get(&id)
//     }

//     pub fn mut_unit_by_id(&mut self, id: UnitID) -> Option<&mut Unit> {
//         self.units_by_id.get_mut(&id)
//     }

//     // pub fn remove_unit_by_loc(&mut self, loc: Location) -> Option<Unit> {
//     //     if let Some(ref unit) = self.units_by_loc.remove(&loc) {
//     //         if let Some(ref unit2) = self.units_by_id.remove(&unit.id) {

//     //         }

//     //     } else {
//     //         None
//     //     }
//     // }

//     pub fn new_city<S:Into<String>>(&mut self, loc: Location, alignment: Alignment, name: S) -> (CityID,&City) {
//         let city_id = self.next_city_id;
//         self.next_city_id = self.next_city_id.next();

//         let city = City::new(city_id, alignment, loc, name);

//         let insertion_result1 = self.cities_by_id.insert(city_id, city);
//         debug_assert!(insertion_result1.is_none());

//         let insertion_result2 = self.city_ids_by_loc.insert(loc, city_id);
//         debug_assert!(insertion_result2.is_none());

//         (city_id, self.cities_by_id.get(&city_id).unwrap())
//     }

//     pub fn city_by_loc(&self, loc: Location) -> Option<&City> {
//         if let Some(city_id) = self.city_ids_by_loc.get(&loc) {
//             self.cities_by_id.get(&city_id)
//         } else {
//             None
//         }
//     }

//     pub fn mut_city_by_loc(&mut self, loc: Location) -> Option<&mut City> {
//         if let Some(city_id) = self.city_ids_by_loc.get(&loc) {
//             self.cities_by_id.get_mut(&city_id)
//         } else {
//             None
//         }
//     }

//     pub fn city_by_id(&self, id: CityID) -> Option<&City> {
//         self.cities_by_id.get(&id)
//     }

//     pub fn mut_city_by_id(&mut self, id: CityID) -> Option<&mut City> {
//         self.cities_by_id.get_mut(&id)
//     }

//     // pub fn move_unit_from_loc(&mut self, src: Location, dest: Location) -> Result<(), String> {
//     //     if let Some(unit) = self.unit_from_loc(src) {



//     //         Ok(())
//     //     } else {
//     //         Err(format!("Attempted to move unit from {} when none exists", src))
//     //     }
//     // }

//     // pub fn move_unit_from_id(&self, id: UnitID, dest: Location) {

//     // }
// }


// /// Convert a multiline string into a map
// /// A convenience method
// /// For example:
// /// `Map::try_from(
// /// "xx x x\
// ///  xx  xx\
// ///  x1  0x"
// /// )`
// /// would yield a map populated thus:
// /// * numerals represent land terrain with a city belonging to the player of that number
// ///   i.e. character "3" becomes a city belonging to player 3 located on land.
// /// * other non-whitespace characters correspond to land
// /// * whitespace characters correspond to water
// ///
// /// Error if there are no lines or if the lines aren't of equal length
// impl TryFrom<&'static str> for MapData {
//     type Error = String;
//     fn try_from(str: &'static str) -> Result<MapData,String> {
//         let lines = Vec::from_iter( str.lines().map(|line| Vec::from_iter( line.chars() )) );
//         if lines.is_empty() {
//             return Err(String::from("String contained no lines"));
//         }

//         let width = lines[0].len();
//         if lines.len() == 1 && width == 0 {
//             return Err(String::from("No map was provided (the string was empty)"));
//         }

//         for line in &lines {
//             if line.len() != width {
//                 return Err(format!("Lines aren't all the same width. Expected {}, found {}", width, line.len()));
//             }
//         }

//         let dims = Dims{width: width as u16, height: lines.len() as u16 };
//         let mut map = MapData::new(dims);
//         let mut loc = Location{x: 0, y: 0};
//         for x in 0..dims.width {
//             loc.x = x;
//             for y in 0..dims.height {
//                 loc.y = y;

//                 let c = lines[loc.y as usize][loc.x as usize];
//                 map.set_terrain(loc, if c==' ' {
//                     Terrain::Water
//                 } else {
//                     Terrain::Land
//                 });

//                 if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
//                     map.new_city(loc, Alignment::Belligerent{player: player_num}, format!("City_{}_{}", loc.x, loc.y));
//                 }
//             }
//         }

//         Ok(map)
//     }
// }

pub struct MapData {
    dims: Dims,
    tiles: LocationGrid<Tile>,
    unit_loc_by_id: HashMap<UnitID,Location>,
    city_loc_by_id: HashMap<CityID,Location>,

    // player_observations: HashMap<PlayerNum,Box<ObsTracker>>,

    next_unit_id: UnitID,
    next_city_id: CityID
}

impl MapData {

    pub fn new(dims: Dims) -> Self {
        Self {
            dims,
            tiles: LocationGrid::new(dims, |loc| Tile::new(Terrain::Water, loc)),
            unit_loc_by_id: HashMap::new(),
            city_loc_by_id: HashMap::new(),
            next_unit_id: UnitID{id: 0},
            next_city_id: CityID{id: 0}
        }
    }

    pub fn dims(&self) -> Dims {
        self.dims
    }

    fn in_bounds(&self, loc: Location) -> bool {
        self.dims.in_bounds(loc)
    }

    pub fn terrain(&self, loc: Location) -> Option<&Terrain> {
        self.tiles.get(loc).map(|tile| &tile.terrain)
    }

    pub fn set_terrain(&mut self, loc: Location, terrain: Terrain) {
        self.tiles.get_mut(loc).unwrap().terrain = terrain;
    }

    pub fn new_unit<S:Into<String>>(&mut self, loc: Location, type_: UnitType, alignment: Alignment, name: S) -> Result<&Unit,String> {
        if !self.in_bounds(loc) {
            return Err(format!("Attempted to create unit at location {} which is out of bounds {}", loc, self.dims));
        }

        if let Some(ref prior_unit) = self.tiles.get(loc).unwrap().unit {
            return Err(format!("Attempted to create unit at location {}, but unit {} is already there", loc, prior_unit));
        }

        let unit_id = self.next_unit_id;
        self.next_unit_id = self.next_unit_id.next();

        let unit: Unit = Unit::new(unit_id, loc, type_, alignment, name);

        let insertion_result = self.unit_loc_by_id.insert(unit_id, loc);
        debug_assert!(insertion_result.is_none());

        self.tiles[loc].unit = Some(unit);

        Ok(self.tiles[loc].unit.as_ref().unwrap())
    }

    pub fn unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        if let Some(tile) = self.tiles.get(loc) {
            tile.unit.as_ref()
        } else {
            None
        }
    }

    pub fn mut_unit_by_loc(&mut self, loc: Location) -> Option<&mut Unit> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            tile.unit.as_mut()
        } else {
            None
        }
    }

    pub fn pop_unit_by_loc(&mut self, loc: Location) -> Option<Unit> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            tile.unit.take()
        } else {
            None
        }
    }

    pub fn set_unit(&mut self, loc: Location, mut unit: Unit) -> Option<Unit> {
        unit.loc = loc;
        self.unit_loc_by_id.insert(unit.id, loc);

        let old_unit = self.pop_unit_by_loc(loc);
        self.tiles.get_mut(loc).unwrap().unit = Some(unit);
        old_unit
    }

    pub fn unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.unit_by_loc( * self.unit_loc_by_id.get(&id).unwrap() )
    }

    pub fn mut_unit_by_id(&mut self, id: UnitID) -> Option<&mut Unit> {
        let loc = *self.unit_loc_by_id.get(&id).unwrap();
        self.mut_unit_by_loc( loc )//FIXME NLL -- this should be a one-liner
    }

    pub fn unit_loc(&self, id: UnitID) -> Option<Location> {
        self.unit_by_id(id).map(|unit| unit.loc)
    }

    pub fn unit_id(&self, loc: Location) -> Option<UnitID> {
        self.unit_by_loc(loc).map(|unit| unit.id)
    }

    pub fn new_city<S:Into<String>>(&mut self, loc: Location, alignment: Alignment, name: S) -> Result<&City,String> {
        if !self.in_bounds(loc) {
            return Err(format!("Attempted to create city at location {} which is out of bounds {}", loc, self.dims));
        }

        if let Some(ref prior_city) = self.tiles.get(loc).unwrap().city {
            return Err(format!("Attempted to create city at location {}, but city {} is already there", loc, prior_city));
        }

        let city_id = self.next_city_id;
        self.next_city_id = self.next_city_id.next();

        let city = City::new(city_id, alignment, loc, name);

        let insertion_result = self.city_loc_by_id.insert(city_id, loc);
        debug_assert!(insertion_result.is_none());

        self.tiles[loc].city = Some(city);

        Ok(self.tiles[loc].city.as_ref().unwrap())
    }

    pub fn city_by_loc(&self, loc: Location) -> Option<&City> {
        if let Some(tile) = self.tiles.get(loc) {
            tile.city.as_ref()
        } else {
            None
        }
    }

    pub fn mut_city_by_loc(&mut self, loc: Location) -> Option<&mut City> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            tile.city.as_mut()
        } else {
            None
        }
    }

    pub fn pop_city_by_loc(&mut self, loc: Location) -> Option<City> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            tile.city.take()
        } else {
            None
        }
    }

    pub fn city_by_id(&self, id: CityID) -> Option<&City> {
        self.city_by_loc( * self.city_loc_by_id.get(&id).unwrap() )
    }

    pub fn mut_city_by_id(&mut self, id: CityID) -> Option<&mut City> {
        let loc = * self.city_loc_by_id.get(&id).unwrap();
        self.mut_city_by_loc( loc )//FIXME NLL -- this should be a one-liner
    }

    pub fn tile(&self, loc: Location) -> Option<&Tile> {
        self.tiles.get(loc)
    }

    // pub fn tile_mut(&mut self, loc: Location) -> Option<&mut Tile> {
    //     self.tiles.get_mut(loc)
    // }



    // pub fn move_unit_from_loc(&mut self, src: Location, dest: Location) -> Result<(), String> {
    //     if let Some(unit) = self.unit_from_loc(src) {



    //         Ok(())
    //     } else {
    //         Err(format!("Attempted to move unit from {} when none exists", src))
    //     }
    // }

    // pub fn move_unit_from_id(&self, id: UnitID, dest: Location) {

    // }

    pub fn destroy_unit_by_loc(&mut self, loc: Location) {
        let unit = self.pop_unit_by_loc(loc).unwrap();
        let removed = self.unit_loc_by_id.remove(&unit.id);
        debug_assert!(removed.is_some());
    }

    pub fn destroy_unit_by_id(&mut self, unit_id: UnitID) {
        let loc = self.unit_loc_by_id.remove(&unit_id).unwrap();
        let unit = self.pop_unit_by_loc(loc);
        debug_assert!(unit.is_some());
    }
}

impl Source<Tile> for MapData {
    fn get(&self, loc: Location) -> Option<&Tile> {
        self.tile(loc)
    }
    fn dims(&self) -> Dims {
        self.dims
    }
}

/// Convert a multiline string into a map
/// A convenience method
/// For example:
/// `Map::try_from(
/// "xx x x\
///  xx  xx\
///  x1  0x"
/// )`
/// would yield a map populated thus:
/// * numerals represent land terrain with a city belonging to the player of that number
///   i.e. character "3" becomes a city belonging to player 3 located on land.
/// * other non-whitespace characters correspond to land
/// * whitespace characters correspond to water
///
/// Error if there are no lines or if the lines aren't of equal length
impl TryFrom<&'static str> for MapData {
    type Error = String;
    fn try_from(str: &'static str) -> Result<Self,String> {
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

        let dims = Dims{width: width as u16, height: lines.len() as u16 };
        let mut map = Self::new(dims);
        let mut loc = Location{x: 0, y: 0};
        for x in 0..dims.width {
            loc.x = x;
            for y in 0..dims.height {
                loc.y = y;

                let c = lines[loc.y as usize][loc.x as usize];
                map.set_terrain(loc, if c==' ' {
                    Terrain::Water
                } else {
                    Terrain::Land
                });

                if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
                    map.new_city(loc, Alignment::Belligerent{player: player_num}, format!("City_{}_{}", loc.x, loc.y)).unwrap();
                }
            }
        }

        Ok(map)
    }
}
