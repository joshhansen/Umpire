//! Abstract map representation
//!
//! Data structures and algorithms for representing and working with the game map.

pub mod dijkstra;
pub mod gen;
mod grid;
pub mod terrain;
pub(in crate::game) mod tile;

pub use self::terrain::Terrain;
pub use self::tile::Tile;
pub use self::grid::LocationGrid;

use std::{
    collections::HashMap,
    convert::TryFrom,
    fmt::{
        Debug,
        Formatter,
        Result as FmtResult,
    },
    iter::{
        FromIterator,
        once,
    },
};

use crate::{
    game::{
        AlignedMaybe,
        Alignment,
        GameError,
        PlayerNum,
        city::{CityID,City},
        unit::{UnitID,Unit,UnitType},
    },
    util::{Dims,Dimensioned,Location},
};

use self::dijkstra::Source;



#[derive(Debug)]
pub enum NewUnitError {
    OutOfBounds {
        loc: Location,
        dims: Dims
    },
    UnitAlreadyPresent {
        loc: Location,
        prior_unit: Unit,
        unit_type_under_production: UnitType,
    }
}

/// An abstract, indexed representation of the map data.
///
/// The main role of this structure is to tracker the IDs, locations, and carried status of all
/// units and cities, in addition to holding the tile data itself.
/// 
/// A given unit is either carried or not carried. Carried units are represented by a mapping of their UnitID's
/// to the carrier UnitID's. Non-carried units are represented in the mapping from their UnitID's to the location
/// of the tile containing them.
///
/// All cities are represented in the CityID->Location mapping.
///
/// Reasonable constraints on tiles, cities, and units are enforced. For example, if a carrier unit is destroyed,
/// all of its carried units will also be destroyed.
#[derive(Clone)]
pub struct MapData {
    /// The dimensions of the map
    dims: Dims,

    /// A grid of map tiles. All cities and units are owned by the tiles that contain them.
    tiles: LocationGrid<Tile>,

    /// What is the location of the tile that directly contains a particular unit (if any)?
    ///
    /// Carried units are not found here but rather in `unit_carrier_by_id`. The carrier unit's location can
    /// then be looked up to find the current location of a carried unit.
    unit_loc_by_id: HashMap<UnitID,Location>,

    /// Which unit carries a particular unit (if any)?
    /// 
    /// Maps from carried -> carrier
    unit_carrier_by_id: HashMap<UnitID,UnitID>,

    /// What is the location of a city with the given ID?
    city_loc_by_id: HashMap<CityID,Location>,

    /// The next UnitID, to be used upon the next unit's creation.
    next_unit_id: UnitID,

    /// The next CityID, to be used upon the next city's creation.
    next_city_id: CityID,
}

impl MapData {
    pub fn new<F>(dims: Dims, terrain_initializer: F) -> Self
        where F: Fn(Location)->Terrain {

        Self {
            dims,
            tiles: LocationGrid::new(dims, |loc| Tile::new(terrain_initializer(loc), loc)),
            unit_loc_by_id: HashMap::new(),
            unit_carrier_by_id: HashMap::new(),
            city_loc_by_id: HashMap::new(),
            next_unit_id: UnitID::new(0),
            next_city_id: CityID::new(0),
        }
    }

    /// Add a carried unit to the relevant indices
    fn index_carried_unit(&mut self, carried: &Unit, carrier: &Unit) {
        self.index_carried_unit_piecemeal(carried.id, carrier.id, carrier.loc);
    }

    /// Add a carried unit to the relevant indices, specified piecemeal
    fn index_carried_unit_piecemeal(&mut self, carried_id: UnitID, carrier_id: UnitID, carrier_loc: Location) {
        let overwritten_loc: Option<Location> = self.unit_loc_by_id.insert(carried_id, carrier_loc);
        let overwritten_carrier_id: Option<UnitID> = self.unit_carrier_by_id.insert(carried_id, carrier_id);

        debug_assert!(overwritten_loc.is_none());
        debug_assert!(overwritten_carrier_id.is_none());
    }

    /// Remove a carried unit from relevant indices
    fn unindex_carried_unit(&mut self, unit: &Unit) {
        let removed_loc: Option<Location> = self.unit_loc_by_id.remove(&unit.id);
        let removed_carrier_id: Option<UnitID> = self.unit_carrier_by_id.remove(&unit.id);

        debug_assert!(removed_loc.is_some());
        debug_assert!(removed_carrier_id.is_some());
    }

    /// Add a top-level unit (and all carried units) to the relevant indices
    fn index_toplevel_unit(&mut self, unit: &Unit) {
        let overwritten_loc: Option<Location> = self.unit_loc_by_id.insert(unit.id, unit.loc);
        debug_assert!(overwritten_loc.is_none());

        for carried_unit in unit.carried_units() {
            self.index_carried_unit(&carried_unit, &unit);
        }
    }

    /// Remove a top-level unit (and all carried units) from the relevant indices
    fn unindex_toplevel_unit(&mut self, unit: &Unit) {
        let removed_loc: Option<Location> = self.unit_loc_by_id.remove(&unit.id);
        debug_assert!(removed_loc.is_some());

        for carried_unit in unit.carried_units() {
            self.unindex_carried_unit(&carried_unit);
        }
    }

    pub fn dims(&self) -> Dims {
        self.dims
    }

    fn in_bounds(&self, loc: Location) -> bool {
        self.dims.contain(loc)
    }

    pub fn terrain(&self, loc: Location) -> Option<&Terrain> {
        self.tiles.get(loc).map(|tile| &tile.terrain)
    }

    pub fn set_terrain(&mut self, loc: Location, terrain: Terrain) {
        self.tiles.get_mut(loc).unwrap().terrain = terrain;
    }

    /// Create a new unit properly indexed and managed
    /// 
    /// Returns the ID of the new unit.
    pub fn new_unit<S:Into<String>>(&mut self, loc: Location, type_: UnitType, alignment: Alignment, name: S) -> Result<UnitID,NewUnitError> {
        if !self.in_bounds(loc) {
            return Err(NewUnitError::OutOfBounds { loc, dims: self.dims });
        }

        if let Some(ref prior_unit) = self.tiles.get(loc).unwrap().unit {
            return Err(NewUnitError::UnitAlreadyPresent { loc, prior_unit: prior_unit.clone(), unit_type_under_production: type_ });
        }

        let unit_id = self.next_unit_id;
        self.next_unit_id = self.next_unit_id.next();

        let unit: Unit = Unit::new(unit_id, loc, type_, alignment, name);

        self.index_toplevel_unit(&unit);

        self.tiles[loc].unit = Some(unit);

        Ok(unit_id)
    }

    /// Get the top-level unit at the given location, if any exists
    pub fn toplevel_unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        if let Some(tile) = self.tiles.get(loc) {
            tile.unit.as_ref()
        } else {
            None
        }
    }

    /// Get the top-level unit at the given location, if any exists; mutably
    #[deprecated]
    pub fn toplevel_unit_by_loc_mut(&mut self, loc: Location) -> Option<&mut Unit> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            tile.unit.as_mut()
        } else {
            None
        }
    }

    /// Get the top-level unit or carried unit at `loc` which has ID `id`, if any
    pub fn unit_by_loc_and_id(&self, loc: Location, id: UnitID) -> Option<&Unit> {
        if let Some(toplevel_unit) = self.toplevel_unit_by_loc(loc) {
            if toplevel_unit.id==id {
                return Some(toplevel_unit);
            }

            toplevel_unit.carried_units().find(|carried_unit| carried_unit.id==id)
        } else {
            None
        }
    }

    /// Get the top-level unit or carried unit at `loc` which has ID `id`, if any; mutably
    #[deprecated]
    pub fn unit_by_loc_and_id_mut(&mut self, loc: Location, id: UnitID) -> Option<&mut Unit> {
        if let Some(toplevel_unit) = self.toplevel_unit_by_loc_mut(loc) {
            if toplevel_unit.id==id {
                return Some(toplevel_unit);
            }

            toplevel_unit.carried_units_mut().find(|carried_unit| carried_unit.id==id)
        } else {
            None
        }
    }

    pub fn pop_unit_by_loc_and_id(&mut self, loc: Location, id: UnitID) -> Option<Unit> {
        self.pop_toplevel_unit_by_loc_and_id(loc, id).or_else(|| self.pop_carried_unit_by_loc_and_id(loc, id))
        // let should_pop_toplevel: bool = if let Some(toplevel_unit) = self.toplevel_unit_by_loc(loc) {
        //     toplevel_unit.id==id
        // } else {
        //     return None;
        // };
        
        // if should_pop_toplevel {
        //     return self.pop_toplevel_unit_by_loc(loc);
        // }

        // if let Some(toplevel_unit) = self.unit_by_loc_mut(loc) {
        //     if let Some(carried_unit) = toplevel_unit.release_by_id(id) {
        //         self.unindex_carried_unit(&carried_unit);
        //         Some(carried_unit)
        //     } else {
        //         None
        //     }
        // } else {
        //     None
        // }
    }

    /// Get a mutable reference to the top-level unit at the given location, if any exists
    #[deprecated]
    pub fn unit_by_loc_mut(&mut self, loc: Location) -> Option<&mut Unit> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            tile.unit.as_mut()
        } else {
            None
        }
    }

    /// Remove the top-level unit from the given location (if any exists) and return it
    pub fn pop_toplevel_unit_by_loc(&mut self, loc: Location) -> Option<Unit> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            let popped_unit = tile.unit.take();
            if let Some(ref popped_unit) = popped_unit {
                self.unindex_toplevel_unit(popped_unit);
            }
            popped_unit
        } else {
            None
        }
    }


    /// Remove the top-level unit with ID `id` (if any exists) and return it
    pub fn pop_toplevel_unit_by_id(&mut self, id: UnitID) -> Option<Unit> {
        if let Some(loc) = self.unit_loc_by_id.get(&id).cloned() {
            self.pop_toplevel_unit_by_loc(loc)
        } else {
            None
        }
    }

    /// Remove the top-evel unit at location `loc` if it has ID `id`
    fn pop_toplevel_unit_by_loc_and_id(&mut self, loc: Location, id: UnitID) -> Option<Unit> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            let matches_id = if let Some(unit) = tile.unit.as_ref() {
                unit.id==id
            } else {
                false
            };

            if matches_id {
                let popped_unit = tile.unit.take().unwrap();
                self.unindex_toplevel_unit(&popped_unit);
                Some(popped_unit)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn pop_carried_unit_by_id(&mut self, carried_unit_id: UnitID) -> Option<Unit> {
        if let Some(carried_unit_loc) = self.unit_loc_by_id.get(&carried_unit_id).cloned() {
            self.pop_carried_unit_by_loc_and_id(carried_unit_loc, carried_unit_id)
        } else {
            None
        }
    }

    fn pop_carried_unit_by_loc_and_id(&mut self, carried_unit_loc: Location, carried_unit_id: UnitID) -> Option<Unit> {
        let carrier_unit = self.unit_by_loc_mut(carried_unit_loc).unwrap();
        if let Some(carried_unit) = carrier_unit.release_by_id(carried_unit_id) {
            self.unindex_carried_unit(&carried_unit);
            Some(carried_unit)
        } else {
            None
        }
    }
    

    /// Set the top-level unit at the given location to the one provided
    /// 
    /// Returns the previous unit, if any
    pub fn set_unit(&mut self, loc: Location, mut unit: Unit) -> Option<Unit> {
        unit.loc = loc;
        for carried_unit in unit.carried_units_mut() {
            carried_unit.loc = loc;
        }
        

        let old_unit = self.pop_toplevel_unit_by_loc(loc);

        self.index_toplevel_unit(&unit);

        self.tiles.get_mut(loc).unwrap().unit = Some(unit);
        old_unit
    }

    /// Get the unit with ID `id`, if any
    /// 
    /// This covers all units, whether top-level or carried.
    pub fn unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.unit_by_loc_and_id(self.unit_loc_by_id[&id], id)
    }

    #[deprecated]
    pub fn unit_by_id_mut(&mut self, id: UnitID) -> Option<&mut Unit> {
        self.unit_by_loc_and_id_mut(self.unit_loc_by_id[&id], id)
    }

    pub fn unit_loc(&self, id: UnitID) -> Option<Location> {
        self.unit_by_id(id).map(|unit| unit.loc)
    }

    pub fn toplevel_unit_id_by_loc(&self, loc: Location) -> Option<UnitID> {
        self.toplevel_unit_by_loc(loc).map(|unit| unit.id)
    }

    /// Make top-level carrier unit with ID `carrier_unit_id` carry `carried_unit`.
    pub fn carry_unit(&mut self, carrier_unit_id: UnitID, carried_unit: Unit) -> Result<usize,String> {
        let carried_unit_id = carried_unit.id;

        let (carry_result, carrier_unit_loc) = {
                let carrier_unit = self.unit_by_id_mut(carrier_unit_id)
                .ok_or_else(|| format!("Unit with ID {:?} cannot carry any units because it does not exist", carrier_unit_id))?;

                (carrier_unit.carry(carried_unit), carrier_unit.loc)
        };

        self.index_carried_unit_piecemeal(carried_unit_id, carrier_unit_id, carrier_unit_loc);

        carry_result
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

    fn city_by_loc_mut(&mut self, loc: Location) -> Option<&mut City> {
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
        self.city_by_loc(self.city_loc_by_id[&id])
    }

    fn city_by_id_mut(&mut self, id: CityID) -> Option<&mut City> {
        self.city_by_loc_mut(self.city_loc_by_id[&id])
    }

    pub fn tile(&self, loc: Location) -> Option<&Tile> {
        self.tiles.get(loc)
    }

    // /// Destroy the top-level unit at `loc`
    // pub fn destroy_unit_by_loc(&mut self, loc: Location) {
    //     let removed_unit = self.pop_unit_by_loc(loc).unwrap();
    //     self.unindex_toplevel_unit(&removed_unit);
    // }

    // pub fn destroy_unit_by_id(&mut self, unit_id: UnitID) {
    //     let removed_loc = self.unit_loc_by_id.remove(&unit_id).unwrap();
    //     let removed_unit = self.pop_unit_by_loc(removed_loc);
    //     debug_assert!(removed_unit.is_some());
    // }

    pub(crate) fn cities(&self) -> impl Iterator<Item=&City> {
        self.tiles.iter().filter_map(|tile| tile.city.as_ref())
    }

    fn cities_mut(&mut self) -> impl Iterator<Item=&mut City> {
        self.tiles.iter_mut().filter_map(|tile| tile.city.as_mut())
    }

    pub(crate) fn units(&self) -> impl Iterator<Item=&Unit> {
        self.tiles.iter().filter_map(|tile| tile.unit.as_ref())
    }

    fn units_mut(&mut self) -> impl Iterator<Item=&mut Unit> {
        self.tiles.iter_mut().filter_map(|tile| tile.unit.as_mut())
    }
    
    fn units_deep(&self) -> impl Iterator<Item=&Unit> {
        self.tiles.iter().filter_map(|tile| tile.unit.as_ref())
        .flat_map(|unit| once(unit).chain(unit.carried_units()))
    }

    pub fn player_units(&self, player: PlayerNum) -> impl Iterator<Item=&Unit> {
        self.units().filter(move |unit| unit.belongs_to_player(player))
    }

    #[deprecated]
    pub fn player_units_mut(&mut self, player: PlayerNum) -> impl Iterator<Item=&mut Unit> {
        self.units_mut().filter(move |unit| unit.belongs_to_player(player))
    }

    pub fn player_units_deep(&self, player: PlayerNum) -> impl Iterator<Item=&Unit> {
        self.units_deep().filter(move |unit| unit.belongs_to_player(player))
    }

    #[deprecated]
    pub fn player_units_deep_mutate<F:FnMut(&mut Unit)>(&mut self, player: PlayerNum, mut callback: F) {
        for unit in self.player_units_mut(player) {
            callback(unit);

            for carried_unit in unit.carried_units_mut() {
                callback(carried_unit);
            }
        }
    }

    /// All cities belonging to the player `player`
    pub fn player_cities(&self, player: PlayerNum) -> impl Iterator<Item=&City> {
        self.cities().filter(move |city| city.belongs_to_player(player))
    }

    #[deprecated]
    pub fn player_cities_mut(&mut self, player: PlayerNum) -> impl Iterator<Item=&mut City> {
        self.cities_mut().filter(move |city| city.belongs_to_player(player))
    }

    fn player_city_by_loc_mut(&mut self, player: PlayerNum, loc: Location) -> Option<&mut City> {
        self.city_by_loc_mut(loc).filter(|city| city.belongs_to_player(player))
    }

    pub fn player_cities_with_production_target(&self, player: PlayerNum) -> impl Iterator<Item=&City> {
        self.player_cities(player).filter(|city| city.production().is_some())
    }

    #[deprecated]
    pub fn player_cities_with_production_target_mut(&mut self, player: PlayerNum) -> impl Iterator<Item=&mut City> {
        self.player_cities_mut(player).filter(|city| city.production().is_some())
    }

    pub fn player_cities_lacking_production_target(&self, player: PlayerNum) -> impl Iterator<Item=&City> {
        self.player_cities(player).filter(|city| city.production().is_none() && !city.ignore_cleared_production())
    }

    pub fn iter_locs(&self) -> impl Iterator<Item=Location> {
        self.tiles.iter_locs()
    }

    pub fn clear_city_production_progress_by_loc(&mut self, loc: Location) -> Result<(),()> {
        self.city_by_loc_mut(loc).map(|city| city.production_progress = 0)
                                    .ok_or(())
    }

    pub fn clear_city_production_progress_by_id(&mut self, city_id: CityID) -> Result<(),()> {
        self.city_by_id_mut(city_id).map(|city| city.production_progress = 0)
                                    .ok_or(())
    }

    pub fn clear_city_production_and_ignore_by_loc(&mut self, loc: Location) -> Result<(),()> {
        self.city_by_loc_mut(loc).map(|city| city.clear_production_and_ignore()).ok_or(())
    }

    pub fn clear_city_production_without_ignoring_by_loc(&mut self, loc: Location) -> Result<(),()> {
        self.city_by_loc_mut(loc).map(|city| city.clear_production_without_ignoring()).ok_or(())
    }

    pub fn set_city_alignment_by_loc(&mut self, loc: Location, alignment: Alignment) -> Result<(),()> {
        self.city_by_loc_mut(loc).map(|city| city.alignment = alignment).ok_or(())
    }

    pub fn set_player_city_production_by_loc(&mut self, player: PlayerNum, loc: Location, production: UnitType) -> Result<(),GameError> {
        self.player_city_by_loc_mut(player, loc).map(|city| city.set_production(production))
                                                .ok_or(GameError::NoCityAtLocation{loc})
    }
}

impl Dimensioned for MapData {
    fn dims(&self) -> Dims {
        self.dims
    }
}

impl Source<Tile> for MapData {
    fn get(&self, loc: Location) -> &Tile {
        self.tile(loc).unwrap()
    }
}

impl Debug for MapData {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        for y in 0..self.dims.height {
            for x in 0..self.dims().width {
                write!(f, "{:?}", self.get(Location{x,y}))?;
            }

            writeln!(f)?;
        }

        Ok(())
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
/// * Letters the letter of the key for any unit yields that unit for player 0 on the appropriate terrain (land for air units).
/// * The capital version of the letter for a unit's key yields that unit for player 1 on the appropriate terrain (land for air units).
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
        let mut map = Self::new(dims, |loc| {
            let c = lines[loc.y as usize][loc.x as usize];

            if c==' ' {
                Terrain::Water
            } else {
                Terrain::Land
            }
        });

        for loc in map.iter_locs() {
            let c = lines[loc.y as usize][loc.x as usize];
            let c_lower = c.to_lowercase().next().unwrap();
            if let Ok(player_num) = format!("{}", c).parse::<PlayerNum>() {
                map.new_city(loc, Alignment::Belligerent{player: player_num}, format!("City_{}_{}", loc.x, loc.y)).unwrap();
            }
            if let Some(unit_type) = UnitType::from_key(c_lower) {
                let player_num = if c.is_lowercase() { 0 } else { 1 };
                map.new_unit(loc, unit_type, Alignment::Belligerent{player: player_num}, format!("Unit_{}_{}", loc.x, loc.y)).unwrap();
            }
        }

        Ok(map)
    }
}
