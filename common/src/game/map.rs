//! Abstract map representation
//!
//! Data structures and algorithms for representing and working with the game map.

pub mod dijkstra;
pub mod gen;
pub(in crate::game) mod grid;
pub mod terrain;
pub(in crate::game) mod tile;

pub use self::grid::{LocationGrid, LocationGridI, SparseLocationGrid};
pub use self::terrain::Terrain;
pub use self::tile::Tile;

use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Formatter, Result as FmtResult},
};

use thiserror::Error;

use crate::{
    game::{
        alignment::{AlignedMaybe, Alignment},
        city::{City, CityID},
        unit::{Unit, UnitID, UnitType},
        GameError, PlayerNum,
    },
    util::{Dimensioned, Dims, Location},
};

use self::dijkstra::Source;
use super::unit::orders::Orders;
use super::UmpireResult;

#[derive(Debug, Error)]
pub enum NewUnitError {
    #[error("Attempted to create a unit at {loc} outside the bounds {dims}")]
    OutOfBounds { loc: Location, dims: Dims },

    #[error("Attempted to create a unit at {loc} but the unit {prior_unit:?} was already present; the city is producing {unit_type_under_production}")]
    UnitAlreadyPresent {
        loc: Location,
        prior_unit: Unit,
        unit_type_under_production: UnitType,
    },
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
    /// A grid of map tiles. All cities and units are owned by the tiles that contain them.
    tiles: LocationGrid<Tile>,

    /// where are all the units located?
    ///
    /// This duplicates the values of `unit_loc_by_id` but those contain duplicates. we keep this to keep things
    /// deduplicated at all times so we can iterate once and only once over each unit's location.
    unit_locs: HashSet<Location>,

    /// What is the location of the tile that directly contains a particular unit (if any)?
    ///
    /// Carried units are not found here but rather in `unit_carrier_by_id`. The carrier unit's location can
    /// then be looked up to find the current location of a carried unit.
    unit_loc_by_id: HashMap<UnitID, Location>,

    /// Which unit carries a particular unit (if any)?
    ///
    /// Maps from carried -> carrier
    unit_carrier_by_id: HashMap<UnitID, UnitID>,

    /// What is the location of a city with the given ID?
    city_loc_by_id: HashMap<CityID, Location>,

    /// The next UnitID, to be used upon the next unit's creation.
    next_unit_id: UnitID,

    /// The next CityID, to be used upon the next city's creation.
    next_city_id: CityID,

    /// The number of cities controlled by each alignment
    alignment_city_counts: HashMap<Alignment, usize>,

    /// The number of each type of unit controlled by each alignment
    alignment_unit_type_counts: HashMap<Alignment, HashMap<UnitType, usize>>,
}

impl MapData {
    pub fn new<F>(dims: Dims, terrain_initializer: F) -> Self
    where
        F: Fn(Location) -> Terrain,
    {
        Self::new_from_grid(LocationGrid::new(dims, |loc| {
            Tile::new(terrain_initializer(loc), loc)
        }))
    }

    pub fn new_from_grid(tiles: LocationGrid<Tile>) -> Self {
        let next_city_id: CityID = tiles
            .iter()
            .filter_map(|tile| tile.city.as_ref())
            .map(|city| city.id)
            .max()
            .map(|id| id.next())
            .unwrap_or_else(CityID::default);

        let next_unit_id: UnitID = tiles
            .iter()
            .filter_map(|tile| tile.unit.as_ref())
            .map(|unit| unit.id)
            .max()
            .map(|id| id.next())
            .unwrap_or_else(UnitID::default);

        let mut map_data = Self {
            tiles,
            unit_locs: HashSet::new(),
            unit_loc_by_id: HashMap::new(),
            unit_carrier_by_id: HashMap::new(),
            city_loc_by_id: HashMap::new(),
            next_city_id,
            next_unit_id,
            alignment_city_counts: HashMap::new(),
            alignment_unit_type_counts: HashMap::new(),
        };

        map_data.index();

        map_data
    }

    /// Index all tiles
    ///
    /// Assumes that none of them have been previously indexed
    fn index(&mut self) {
        for loc in self.tiles.iter_locs() {
            self.index_tile(loc);
        }
    }

    fn index_tile(&mut self, loc: Location) {
        let tile = LocationGridI::get(&self.tiles, loc).cloned().unwrap(); //CLONE
        if let Some(city) = tile.city.as_ref() {
            self.index_city(city);
        }
        if let Some(unit) = tile.unit.as_ref() {
            self.index_toplevel_unit(unit);
            for carried_unit in unit.carried_units() {
                self.index_carried_unit(carried_unit, unit);
            }
        }
    }

    /// Add a carried unit to the relevant indices
    fn index_carried_unit(&mut self, carried: &Unit, carrier: &Unit) {
        self.index_carried_unit_piecemeal(
            carried.id,
            carried.alignment,
            carried.type_,
            carrier.id,
            carrier.loc,
        )
    }

    /// Add a carried unit to the relevant indices, specified piecemeal
    fn index_carried_unit_piecemeal(
        &mut self,
        carried_id: UnitID,
        carried_alignment: Alignment,
        carried_type: UnitType,
        carrier_id: UnitID,
        carrier_loc: Location,
    ) {
        // NOTE: Carried units will already be represented in `unit_locs` by the location of their carrier unit
        //       No need to update `unit_locs` here for that reason.

        let overwritten_loc: Option<Location> = self.unit_loc_by_id.insert(carried_id, carrier_loc);
        let overwritten_carrier_id: Option<UnitID> =
            self.unit_carrier_by_id.insert(carried_id, carrier_id);

        debug_assert!(overwritten_loc.is_none());
        debug_assert!(overwritten_carrier_id.is_none());

        *self
            .alignment_unit_type_counts
            .entry(carried_alignment)
            .or_default()
            .entry(carried_type)
            .or_insert(0) += 1;
    }

    /// Remove a carried unit from relevant indices
    fn unindex_carried_unit(&mut self, unit: &Unit) {
        // NOTE: Carried units will already be represented in `unit_locs` by the location of their carrier unit
        //       No need to update `unit_locs` here for that reason.

        let removed_loc: Option<Location> = self.unit_loc_by_id.remove(&unit.id);
        let removed_carrier_id: Option<UnitID> = self.unit_carrier_by_id.remove(&unit.id);

        debug_assert!(removed_loc.is_some());
        debug_assert!(removed_carrier_id.is_some());

        *self
            .alignment_unit_type_counts
            .entry(unit.alignment)
            .or_default()
            .entry(unit.type_)
            .or_insert(0) -= 1;
    }

    /// Add a top-level unit (and all carried units) to the relevant indices
    fn index_toplevel_unit(&mut self, unit: &Unit) {
        let added = self.unit_locs.insert(unit.loc);
        debug_assert!(added);

        let overwritten_loc: Option<Location> = self.unit_loc_by_id.insert(unit.id, unit.loc);
        debug_assert_eq!(
            overwritten_loc,
            None,
            "Tried to index a unit {:?} but an entry already exists for its ID in unit_loc_by_id; points to tile {:?}",
            unit,
            overwritten_loc.map(|loc| self.tile(loc).unwrap())
        );

        *self
            .alignment_unit_type_counts
            .entry(unit.alignment)
            .or_default()
            .entry(unit.type_)
            .or_insert(0) += 1;

        for carried_unit in unit.carried_units() {
            self.index_carried_unit(carried_unit, unit);
        }
    }

    /// Remove a top-level unit (and all carried units) from the relevant indices
    fn unindex_toplevel_unit(&mut self, unit: &Unit) {
        let was_present = self.unit_locs.remove(&unit.loc);
        debug_assert!(was_present);

        let removed_loc: Option<Location> = self.unit_loc_by_id.remove(&unit.id);
        debug_assert_eq!(removed_loc.unwrap(), unit.loc);

        *self
            .alignment_unit_type_counts
            .entry(unit.alignment)
            .or_default()
            .entry(unit.type_)
            .or_insert(0) -= 1;

        for carried_unit in unit.carried_units() {
            self.unindex_carried_unit(carried_unit);
        }
    }

    /// Add a city to the relevant indices
    fn index_city(&mut self, city: &City) {
        let insertion_result = self.city_loc_by_id.insert(city.id, city.loc);
        debug_assert!(insertion_result.is_none());

        *self
            .alignment_city_counts
            .entry(city.alignment)
            .or_insert(0) += 1;
    }

    /// Remove a city from the relevant indices
    fn unindex_city(&mut self, city: &City) {
        let removed_loc = self.city_loc_by_id.remove(&city.id);
        debug_assert_eq!(removed_loc.unwrap(), city.loc);

        *self
            .alignment_city_counts
            .entry(city.alignment)
            .or_insert(0) -= 1;
    }

    fn in_bounds(&self, loc: Location) -> bool {
        self.dims().contain(loc)
    }

    pub fn terrain(&self, loc: Location) -> Option<&Terrain> {
        LocationGridI::get(&self.tiles, loc).map(|tile| &tile.terrain)
    }

    /// Set the terrain of a tile at the given location, returning the prior setting.
    pub fn set_terrain(&mut self, loc: Location, terrain: Terrain) -> Result<Terrain, GameError> {
        let tile = self
            .tiles
            .get_mut(loc)
            .ok_or(GameError::NoTileAtLocation { loc })?;
        let old = tile.terrain;
        tile.terrain = terrain;
        Ok(old)
    }

    /// Create a new unit properly indexed and managed
    ///
    /// Returns the ID of the new unit.
    pub fn new_unit<S: Into<String>>(
        &mut self,
        loc: Location,
        type_: UnitType,
        alignment: Alignment,
        name: S,
    ) -> Result<UnitID, NewUnitError> {
        if !self.in_bounds(loc) {
            return Err(NewUnitError::OutOfBounds {
                loc,
                dims: self.dims(),
            });
        }

        if let Some(ref prior_unit) = LocationGridI::get(&self.tiles, loc).unwrap().unit {
            return Err(NewUnitError::UnitAlreadyPresent {
                loc,
                prior_unit: prior_unit.clone(),
                unit_type_under_production: type_,
            });
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
        if let Some(tile) = LocationGridI::get(&self.tiles, loc) {
            tile.unit.as_ref()
        } else {
            None
        }
    }

    /// Get the top-level unit at the given location, if any exists; mutably
    fn toplevel_unit_by_loc_mut(&mut self, loc: Location) -> Option<&mut Unit> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            tile.unit.as_mut()
        } else {
            None
        }
    }

    /// Get the top-level unit or carried unit at `loc` which has ID `id`, if any
    pub fn unit_by_loc_and_id(&self, loc: Location, id: UnitID) -> Option<&Unit> {
        if let Some(toplevel_unit) = self.toplevel_unit_by_loc(loc) {
            if toplevel_unit.id == id {
                return Some(toplevel_unit);
            }

            toplevel_unit
                .carried_units()
                .find(|carried_unit| carried_unit.id == id)
        } else {
            None
        }
    }

    /// Get the top-level unit or carried unit at `loc` which has ID `id`, if any; mutably
    fn unit_by_loc_and_id_mut(&mut self, loc: Location, id: UnitID) -> Option<&mut Unit> {
        if let Some(toplevel_unit) = self.toplevel_unit_by_loc_mut(loc) {
            if toplevel_unit.id == id {
                return Some(toplevel_unit);
            }

            toplevel_unit
                .carried_units_mut()
                .find(|carried_unit| carried_unit.id == id)
        } else {
            None
        }
    }

    pub fn pop_unit_by_loc_and_id(&mut self, loc: Location, id: UnitID) -> Option<Unit> {
        self.pop_toplevel_unit_by_loc_and_id(loc, id)
            .or_else(|| self.pop_carried_unit_by_loc_and_id(loc, id))
    }

    pub fn pop_unit_by_id(&mut self, id: UnitID) -> Option<Unit> {
        self.pop_toplevel_unit_by_id(id)
            .or_else(|| self.pop_carried_unit_by_id(id))
    }

    pub fn pop_player_unit_by_id(&mut self, player: PlayerNum, id: UnitID) -> Option<Unit> {
        if self.player_unit_by_id(player, id).is_some() {
            self.pop_unit_by_id(id)
        } else {
            None
        }
    }

    /// Get a mutable reference to the top-level unit at the given location, if any exists
    fn unit_by_loc_mut(&mut self, loc: Location) -> Option<&mut Unit> {
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
            self.pop_toplevel_unit_by_loc_and_id(loc, id)
        } else {
            None
        }
    }

    /// Remove the top-evel unit at location `loc` if it has ID `id`
    fn pop_toplevel_unit_by_loc_and_id(&mut self, loc: Location, id: UnitID) -> Option<Unit> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            let matches_id = if let Some(unit) = tile.unit.as_ref() {
                unit.id == id
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

    fn pop_carried_unit_by_loc_and_id(
        &mut self,
        carried_unit_loc: Location,
        carried_unit_id: UnitID,
    ) -> Option<Unit> {
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

    /// Relocate a unit from anywhere on the map to the destination
    ///
    /// This makes no consideration of whether that makes sense. For actual game logic see Game::move_unit_by_id
    ///
    /// The contained value is an Option representing any unit that was present at the destination
    pub fn relocate_unit_by_id(
        &mut self,
        id: UnitID,
        dest: Location,
    ) -> Result<Option<Unit>, GameError> {
        let unit = self
            .pop_unit_by_id(id)
            .ok_or(GameError::NoSuchUnit { id })?;

        Ok(self.set_unit(dest, unit))
    }

    /// Occupy the city at the given location using the unit with the given ID.
    ///
    /// This will update the city's alignment to match the occupier unit.
    ///
    /// Errors if no city exists at the location, or the location is out of bounds, or a unit is already present in the
    /// city---garrisoned units should be defeated prior to occupying.
    ///
    /// FIXME Move into Game---probably makes more sense there
    pub fn occupy_city(
        &mut self,
        occupier_unit_id: UnitID,
        city_loc: Location,
    ) -> Result<(), GameError> {
        let alignment = self
            .unit_by_id(occupier_unit_id)
            .map(|unit| unit.alignment)
            .ok_or(GameError::NoSuchUnit {
                id: occupier_unit_id,
            })?;

        {
            let tile = self
                .tile_mut(city_loc)
                .ok_or(GameError::NoTileAtLocation { loc: city_loc })?;

            if tile.unit.is_some() {
                return Err(GameError::CannotOccupyGarrisonedCity {
                    occupier_unit_id,
                    city_id: tile.city.as_ref().unwrap().id,
                    garrisoned_unit_id: tile.unit.as_ref().unwrap().id,
                });
            }

            if let Some(city) = tile.city.as_mut() {
                city.alignment = alignment;
            } else {
                return Err(GameError::NoCityAtLocation { loc: city_loc });
            }
        }

        // self.mark_unit_movement_complete(occupier_unit_id)?;

        self.relocate_unit_by_id(occupier_unit_id, city_loc)
            .map(|_| ()) //discard the "replaced" unit since we know there isn't one there
    }

    /// Get the unit with ID `id`, if any
    ///
    /// This covers all units, whether top-level or carried.
    pub fn unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.unit_loc_by_id
            .get(&id)
            .map(|loc| self.unit_by_loc_and_id(*loc, id).unwrap())
    }

    /// Get the unit with ID `id`, if any, mutably
    ///
    /// This covers all units, whether top-level or carried.
    fn unit_by_id_mut(&mut self, id: UnitID) -> Option<&mut Unit> {
        self.unit_by_loc_and_id_mut(self.unit_loc_by_id[&id], id)
    }

    pub fn player_unit_by_id(&self, player: PlayerNum, id: UnitID) -> Option<&Unit> {
        self.unit_by_id(id)
            .filter(|unit| unit.belongs_to_player(player) && unit.id == id)
    }

    pub fn player_unit_by_id_mut(&mut self, player: PlayerNum, id: UnitID) -> Option<&mut Unit> {
        self.unit_by_id_mut(id)
            .filter(|unit| unit.belongs_to_player(player) && unit.id == id)
    }

    pub fn mark_unit_movement_complete(&mut self, id: UnitID) -> Result<(), GameError> {
        let unit = self
            .unit_by_id_mut(id)
            .ok_or(GameError::NoSuchUnit { id })?;

        unit.movement_complete();
        Ok(())
    }

    pub fn record_unit_movement(&mut self, id: UnitID, moves: u16) -> UmpireResult<u16> {
        let unit = self
            .unit_by_id_mut(id)
            .ok_or(GameError::NoSuchUnit { id })?;

        unit.record_movement(moves)
    }

    pub fn unit_loc(&self, id: UnitID) -> Option<Location> {
        self.unit_by_id(id).map(|unit| unit.loc)
    }

    pub fn toplevel_unit_id_by_loc(&self, loc: Location) -> Option<UnitID> {
        self.toplevel_unit_by_loc(loc).map(|unit| unit.id)
    }

    /// Check for any errors we would encounter were we to try having the carrier carry the given unit
    fn carry_status(&self, carrier_unit_id: UnitID, carried_unit: &Unit) -> Result<(), GameError> {
        self.unit_by_id(carrier_unit_id)
            .ok_or(GameError::NoSuchUnit {
                id: carrier_unit_id,
            })?
            .carry_status(carried_unit)
    }

    /// Check for any errors we would encounter were we to try having the carrier carry the specified unit
    fn carry_status_by_id(
        &self,
        carrier_unit_id: UnitID,
        carried_unit_id: UnitID,
    ) -> Result<(), GameError> {
        let unit = self
            .unit_by_id(carried_unit_id)
            .ok_or(GameError::NoSuchUnit {
                id: carried_unit_id,
            })?;

        debug_assert_eq!(carried_unit_id, unit.id);

        self.carry_status(carrier_unit_id, unit)?;

        Ok(())
    }

    /// Make top-level carrier unit with ID `carrier_unit_id` carry `carried_unit`.
    ///
    /// Returns the number of units now carried
    fn _carry_unit_no_checks(&mut self, carrier_unit_id: UnitID, carried_unit: Unit) -> usize {
        debug_assert_eq!(
            self.carry_status(carrier_unit_id, &carried_unit),
            Ok(()),
            "Error carrying unit {:?} on unit {:?}",
            carried_unit,
            self.unit_by_id(carrier_unit_id),
        );

        let carried_unit_id = carried_unit.id;

        // let carrier_unit = self.unit_by_id(carrier_unit_id).unwrap();

        // self.index_carried_unit(&carried_unit, carrier_unit);

        let carried_alignment = carried_unit.alignment;
        let carried_type = carried_unit.type_;

        let (carry_result, carrier_loc) = {
            let carrier_unit = self.unit_by_id_mut(carrier_unit_id).unwrap();

            let carry_result = carrier_unit.carry(carried_unit);
            (carry_result, carrier_unit.loc)
        };

        self.index_carried_unit_piecemeal(
            carried_unit_id,
            carried_alignment,
            carried_type,
            carrier_unit_id,
            carrier_loc,
        );

        carry_result.unwrap()
    }

    /// Make top-level carrier unit with ID `carrier_unit_id` carry `carried_unit`.
    pub fn carry_unit(
        &mut self,
        carrier_unit_id: UnitID,
        carried_unit: Unit,
    ) -> Result<usize, GameError> {
        self.carry_status(carrier_unit_id, &carried_unit)?;

        Ok(self._carry_unit_no_checks(carrier_unit_id, carried_unit))
    }

    /// Move a unit on the map to the carrying space of another unit on the map
    pub fn carry_unit_by_id(
        &mut self,
        carrier_unit_id: UnitID,
        carried_unit_id: UnitID,
    ) -> Result<usize, GameError> {
        // Check that everything's groovy before we go popping any units out of place
        self.carry_status_by_id(carrier_unit_id, carried_unit_id)?;

        // Now pop away
        let unit = self
            .pop_unit_by_id(carried_unit_id)
            .ok_or(GameError::NoSuchUnit {
                id: carried_unit_id,
            })?;

        // And do the carry without re-checking since we know it will succeed
        Ok(self._carry_unit_no_checks(carrier_unit_id, unit))
    }

    pub fn set_unit_orders(
        &mut self,
        unit_id: UnitID,
        orders: Orders,
    ) -> Result<Option<Orders>, GameError> {
        let unit = self
            .unit_by_id_mut(unit_id)
            .ok_or(GameError::NoSuchUnit { id: unit_id })?;
        Ok(unit.set_orders(orders))
    }

    pub fn set_player_unit_orders(
        &mut self,
        player: PlayerNum,
        unit_id: UnitID,
        orders: Orders,
    ) -> Result<Option<Orders>, GameError> {
        let unit = self
            .player_unit_by_id_mut(player, unit_id)
            .ok_or(GameError::NoSuchUnit { id: unit_id })?;
        Ok(unit.set_orders(orders))
    }

    pub fn clear_player_unit_orders(
        &mut self,
        player: PlayerNum,
        unit_id: UnitID,
    ) -> Result<Option<Orders>, GameError> {
        let unit = self
            .player_unit_by_id_mut(player, unit_id)
            .ok_or(GameError::NoSuchUnit { id: unit_id })?;
        Ok(unit.clear_orders())
    }

    pub fn activate_player_unit(
        &mut self,
        player: PlayerNum,
        unit_id: UnitID,
    ) -> Result<(), GameError> {
        let unit = self
            .player_unit_by_id_mut(player, unit_id)
            .ok_or(GameError::NoSuchUnit { id: unit_id })?;

        unit.activate();

        Ok(())
    }

    pub fn new_city<S: Into<String>>(
        &mut self,
        loc: Location,
        alignment: Alignment,
        name: S,
    ) -> Result<&City, String> {
        if !self.in_bounds(loc) {
            return Err(format!(
                "Attempted to create city at location {} which is out of bounds {}",
                loc,
                self.dims()
            ));
        }

        if let Some(ref prior_city) = LocationGridI::get(&self.tiles, loc).unwrap().city {
            return Err(format!(
                "Attempted to create city at location {}, but city {} is already there",
                loc, prior_city
            ));
        }

        let city_id = self.next_city_id;
        self.next_city_id = self.next_city_id.next();

        let city = City::new(city_id, alignment, loc, name);

        self.index_city(&city);

        self.tiles[loc].city = Some(city);

        Ok(self.tiles[loc].city.as_ref().unwrap())
    }

    pub fn city_by_loc(&self, loc: Location) -> Option<&City> {
        if let Some(tile) = LocationGridI::get(&self.tiles, loc) {
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

    pub fn player_city_by_loc(&self, player: PlayerNum, loc: Location) -> Option<&City> {
        self.city_by_loc(loc)
            .filter(|city| city.belongs_to_player(player))
    }

    pub fn pop_city_by_loc(&mut self, loc: Location) -> Option<City> {
        if let Some(tile) = self.tiles.get_mut(loc) {
            let old_city = tile.city.take();
            if let Some(old_city) = old_city.as_ref() {
                self.unindex_city(old_city);
            }
            old_city
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
        LocationGridI::get(&self.tiles, loc)
    }

    fn tile_mut(&mut self, loc: Location) -> Option<&mut Tile> {
        self.tiles.get_mut(loc)
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

    pub(crate) fn cities(&self) -> impl Iterator<Item = &City> {
        self.city_loc_by_id
            .values()
            .map(move |loc| self.city_by_loc(*loc).unwrap())
    }

    //FIXME Use the `city_locs` index instead of scanning every tile in search of cities
    fn cities_mut(&mut self) -> impl Iterator<Item = &mut City> {
        self.tiles.iter_mut().filter_map(|tile| tile.city.as_mut())
    }

    pub(crate) fn toplevel_units(&self) -> impl Iterator<Item = &Unit> {
        self.tiles.iter().filter_map(|tile| tile.unit.as_ref())
    }

    fn toplevel_units_mut(&mut self) -> impl Iterator<Item = &mut Unit> {
        self.tiles.iter_mut().filter_map(|tile| tile.unit.as_mut())
    }

    pub(crate) fn units(&self) -> impl Iterator<Item = &Unit> {
        self.unit_locs
            .iter()
            .map(move |unit_loc| self.tile(*unit_loc).unwrap())
            .flat_map(|tile| tile.all_units())
    }

    pub fn player_toplevel_units(&self, player: PlayerNum) -> impl Iterator<Item = &Unit> {
        self.toplevel_units()
            .filter(move |unit| unit.belongs_to_player(player))
    }

    pub fn player_toplevel_units_mut(
        &mut self,
        player: PlayerNum,
    ) -> impl Iterator<Item = &mut Unit> {
        self.toplevel_units_mut()
            .filter(move |unit| unit.belongs_to_player(player))
    }

    /// An iterator over all units (toplevel and carried) belonging to the given player
    pub fn player_units(&self, player: PlayerNum) -> impl Iterator<Item = &Unit> {
        self.units()
            .filter(move |unit| unit.belongs_to_player(player))
    }

    fn player_units_mut<F: FnMut(&mut Unit)>(&mut self, player: PlayerNum, mut callback: F) {
        for unit in self.player_toplevel_units_mut(player) {
            callback(unit);

            for carried_unit in unit.carried_units_mut() {
                callback(carried_unit);
            }
        }
    }

    // How many of each type of unit does the given player control?
    pub fn player_unit_type_counts(
        &self,
        player: PlayerNum,
    ) -> Result<&HashMap<UnitType, usize>, GameError> {
        self.alignment_unit_type_counts
            .get(&Alignment::Belligerent { player })
            .ok_or(GameError::NoSuchPlayer { player })
    }

    pub fn refresh_player_unit_moves_remaining(&mut self, player: PlayerNum) {
        self.player_units_mut(player, |unit| unit.refresh_moves_remaining());
    }

    /// All cities belonging to the player `player`
    pub fn player_cities(&self, player: PlayerNum) -> impl Iterator<Item = &City> {
        self.cities()
            .filter(move |city| city.belongs_to_player(player))
    }

    fn player_cities_mut(&mut self, player: PlayerNum) -> impl Iterator<Item = &mut City> {
        self.cities_mut()
            .filter(move |city| city.belongs_to_player(player))
    }

    fn player_city_by_loc_mut(&mut self, player: PlayerNum, loc: Location) -> Option<&mut City> {
        self.city_by_loc_mut(loc)
            .filter(|city| city.belongs_to_player(player))
    }

    fn player_city_by_id_mut(&mut self, player: PlayerNum, city_id: CityID) -> Option<&mut City> {
        self.city_by_id_mut(city_id)
            .filter(|city| city.belongs_to_player(player))
    }

    pub fn player_cities_with_production_target(
        &self,
        player: PlayerNum,
    ) -> impl Iterator<Item = &City> {
        self.player_cities(player)
            .filter(|city| city.production().is_some())
    }

    fn player_cities_with_production_target_mut(
        &mut self,
        player: PlayerNum,
    ) -> impl Iterator<Item = &mut City> {
        self.player_cities_mut(player)
            .filter(|city| city.production().is_some())
    }

    pub fn increment_player_city_production_targets(&mut self, player: PlayerNum) {
        let max_unit_cost: u16 = UnitType::values().iter().map(|ut| ut.cost()).max().unwrap();
        for city in self.player_cities_with_production_target_mut(player) {
            // We cap the production progress since, in weird circumstances such as a city having a unit blocking its
            // production for a very long time, the production progress adds can overflow
            if city.production_progress < max_unit_cost {
                city.production_progress += 1;
            }
        }
    }

    pub fn player_cities_lacking_production_target(
        &self,
        player: PlayerNum,
    ) -> impl Iterator<Item = &City> {
        self.player_cities(player)
            .filter(|city| city.production().is_none() && !city.ignore_cleared_production())
    }

    /// How many cities does the given player control?
    pub fn player_city_count(&self, player: PlayerNum) -> Result<usize, GameError> {
        self.alignment_city_counts
            .get(&Alignment::Belligerent { player })
            .cloned()
            .ok_or(GameError::NoSuchPlayer { player })
    }

    /// The number of non-neutral players having at least one city or unit
    pub fn players(&self) -> usize {
        let mut alignments: HashSet<Alignment> = HashSet::new();

        alignments.extend(self.alignment_city_counts.keys());
        alignments.extend(self.alignment_unit_type_counts.keys());

        alignments.remove(&Alignment::Neutral);

        alignments.len()
    }

    pub fn iter_locs(&self) -> impl Iterator<Item = Location> {
        self.tiles.iter_locs()
    }

    pub fn clear_city_production_progress_by_loc(&mut self, loc: Location) -> UmpireResult<()> {
        self.city_by_loc_mut(loc)
            .map(|city| city.production_progress = 0)
            .ok_or(GameError::NoCityAtLocation { loc })
    }

    pub fn clear_city_production_progress_by_id(&mut self, id: CityID) -> UmpireResult<()> {
        self.city_by_id_mut(id)
            .map(|city| city.production_progress = 0)
            .ok_or(GameError::NoSuchCity { id })
    }

    pub fn clear_city_production_by_loc(
        &mut self,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> Result<Option<UnitType>, GameError> {
        let city = self
            .city_by_loc_mut(loc)
            .ok_or(GameError::NoCityAtLocation { loc })?;

        Ok(city.clear_production(ignore_cleared_production))
    }

    pub fn set_city_alignment_by_loc(
        &mut self,
        loc: Location,
        alignment: Alignment,
    ) -> UmpireResult<()> {
        self.city_by_loc_mut(loc)
            .map(|city| city.alignment = alignment)
            .ok_or(GameError::NoCityAtLocation { loc })
    }

    pub fn set_city_production_by_loc(
        &mut self,
        loc: Location,
        production: UnitType,
    ) -> Result<Option<UnitType>, GameError> {
        let city = self
            .city_by_loc_mut(loc)
            .ok_or(GameError::NoCityAtLocation { loc })?;
        Ok(city.set_production(production))
    }

    pub fn set_player_city_production_by_loc(
        &mut self,
        player: PlayerNum,
        loc: Location,
        production: UnitType,
    ) -> Result<Option<UnitType>, GameError> {
        self.player_city_by_loc_mut(player, loc)
            .map(|city| city.set_production(production))
            .ok_or(GameError::NoCityAtLocation { loc })
    }

    pub fn set_player_city_production_by_id(
        &mut self,
        player: PlayerNum,
        city_id: CityID,
        production: UnitType,
    ) -> Result<Option<UnitType>, GameError> {
        self.player_city_by_id_mut(player, city_id)
            .map(|city| city.set_production(production))
            .ok_or(GameError::NoSuchCity { id: city_id })
    }
}

impl Dimensioned for MapData {
    fn dims(&self) -> Dims {
        self.tiles.dims()
    }
}

impl Source<Tile> for MapData {
    fn get(&self, loc: Location) -> &Tile {
        self.tile(loc).unwrap()
    }
}

impl Debug for MapData {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        for y in 0..self.dims().height {
            for x in 0..self.dims().width {
                write!(f, "{:?}", self.get(Location { x, y }))?;
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
    fn try_from(s: &'static str) -> Result<Self, String> {
        let grid = LocationGrid::try_from(s)?;
        Ok(Self::new_from_grid(grid))
    }
}

impl TryFrom<String> for MapData {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        let grid = LocationGrid::try_from(s)?;
        Ok(Self::new_from_grid(grid))
    }
}

#[cfg(test)]
mod test {
    use rand::{distributions::Distribution, thread_rng};

    use super::gen::generate_map;
    use super::MapData;
    use crate::{
        game::{
            map::{terrain::Terrain, CityID, LocationGridI},
            unit::{TransportMode, UnitID, UnitType},
            Alignment, GameError,
        },
        name::IntNamer,
        util::{Dimensioned, Dims, Location},
    };

    #[test]
    pub fn test_identifiers() {
        let mut map = MapData::new(Dims::new(5, 5), |_loc| Terrain::Land);
        let unit_id = map
            .new_unit(
                Location::new(2, 3),
                UnitType::Destroyer,
                Alignment::Belligerent { player: 0 },
                "Edsgar",
            )
            .unwrap();

        let city_id = map
            .new_city(
                Location::new(1, 1),
                Alignment::Belligerent { player: 0 },
                "Steubenville",
            )
            .unwrap()
            .id;

        assert_eq!(city_id, CityID::default());
        assert_eq!(unit_id, UnitID::default());

        let map = MapData::try_from("iiiiii000000").unwrap();
        assert_eq!(map.next_city_id, CityID::new(6));
        assert_eq!(map.next_unit_id, UnitID::new(6));
    }

    #[test]
    pub fn test_map_data() {
        let mut map = MapData::new(Dims::new(10, 10), |_| Terrain::Land);

        let unit_id = map
            .new_unit(
                Location::new(0, 0),
                UnitType::Infantry,
                Alignment::Belligerent { player: 0 },
                "Unit 0",
            )
            .unwrap();

        assert!(map.player_units(0).any(|unit| unit.id == unit_id));

        map.relocate_unit_by_id(unit_id, Location::new(5, 5))
            .unwrap();

        assert!(map.player_units(0).any(|unit| unit.id == unit_id));

        let _popped_unit = map.pop_unit_by_id(unit_id).unwrap();
        assert!(!map.player_units(0).any(|unit| unit.id == unit_id));
    }

    #[test]
    pub fn test_new_and_pop() {
        let mut rand = thread_rng();

        for _ in 0..100 {
            let mut city_namer = IntNamer::new("city");
            let mut map = generate_map(&mut city_namer, Dims::new(180, 90), 1);

            for i in 0..100 {
                let loc = map.dims().sample(&mut rand);

                let name = format!("Unit {}", i);

                // New

                let unit_id = map
                    .new_unit(
                        loc,
                        UnitType::Infantry,
                        Alignment::Belligerent { player: 0 },
                        name.clone(),
                    )
                    .unwrap();

                assert!(map.player_units(0).any(|unit| unit.id == unit_id));

                assert_eq!(map.unit_loc(unit_id), Some(loc));

                let unit = map.unit_by_id(unit_id).unwrap().clone();

                assert_eq!(map.tile(loc).unwrap().unit.as_ref(), Some(&unit));

                // Pop

                let popped_unit = map.pop_unit_by_id(unit_id).unwrap();

                assert_eq!(unit, popped_unit);

                assert!(!map.player_units(0).any(|unit| unit.id == unit_id));

                assert_eq!(map.tile(loc).unwrap().unit, None);
            }
        }
    }

    // #[test]
    // pub fn test_carry() {
    //     let mut rand = thread_rng();

    //     for _ in 0..100 {
    //         let mut city_namer = IntNamer::new("city");
    //         let mut map = generate_map(&mut city_namer, Dims::new(180, 90), 1);

    //         for i in 0.. 100 {
    //             let loc = map.dims().sample(&mut rand);

    //             let name = format!("Unit {}", i);

    //             // New

    //             let unit_id = map.new_unit(loc, UnitType::Infantry,
    //                                     Alignment::Belligerent{player:0}, name.clone()).unwrap();

    //             assert!(map.player_units(0).any(|unit| unit.id == unit_id));

    //             assert_eq!(map.unit_loc(unit_id), Some(loc));

    //             let unit = map.unit_by_id(unit_id).unwrap().clone();

    //             assert_eq!(map.tile(loc).unwrap().unit.as_ref(), Some(&unit));

    //             // Pop

    //             let popped_unit = map.pop_unit_by_id(unit_id).unwrap();

    //             assert_eq!(unit, popped_unit);

    //             assert!(!map.player_units(0).any(|unit| unit.id == unit_id));

    //             assert_eq!(map.tile(loc).unwrap().unit, None);
    //         }
    //     }
    // }

    #[test]
    pub fn test_relocate() {
        let mut map = MapData::new(Dims::new(10, 10), |_| Terrain::Land);

        let unit_id = map
            .new_unit(
                Location::new(0, 0),
                UnitType::Transport,
                Alignment::Belligerent { player: 0 },
                "Unit 0",
            )
            .unwrap();

        let unit_id2 = map
            .new_unit(
                Location::new(0, 1),
                UnitType::Infantry,
                Alignment::Belligerent { player: 0 },
                "Passenger",
            )
            .unwrap();

        map.carry_unit_by_id(unit_id, unit_id2).unwrap();

        assert!(map.player_units(0).any(|unit| unit.id == unit_id));

        let mut rand = thread_rng();

        for _ in 0..1000 {
            let dest = map.dims().sample(&mut rand);
            map.relocate_unit_by_id(unit_id, dest).unwrap();

            assert!(map.player_units(0).any(|unit| unit.id == unit_id));
            assert!(map.player_units(0).any(|unit| unit.id == unit_id2));

            assert_eq!(map.unit_loc(unit_id), Some(dest));
            assert_eq!(map.unit_loc(unit_id2), Some(dest));
        }
    }

    #[test]
    pub fn test_set_terrain() {
        let mut map = MapData::new(Dims::new(10, 10), |_| Terrain::Land);
        for loc in map.dims().iter_locs() {
            let result = map.set_terrain(loc, Terrain::Water);
            assert_eq!(result.unwrap(), Terrain::Land);
        }

        for loc in [
            Location::new(0, 10),
            Location::new(10, 0),
            Location::new(10, 10),
            Location::new(593, 9000),
        ] {
            assert_eq!(
                map.set_terrain(loc, Terrain::Water),
                Err(GameError::NoTileAtLocation { loc })
            );
        }
    }

    #[test]
    fn test_unit_by_id() {
        let mut map = MapData::new(Dims::new(10, 10), |_| Terrain::Land);

        {
            let id = UnitID::default();
            assert_eq!(map.unit_by_id(id), None);
        }

        let id = map
            .new_unit(
                Location::new(5, 6),
                UnitType::Submarine,
                Alignment::Neutral,
                "Swiss Army Man",
            )
            .unwrap();

        assert_eq!(map.unit_by_id(id).unwrap().id, id);
    }

    #[test]
    fn test_carry_unit_by_id() {
        let mut map = MapData::try_from("ffffffktaaaaa").unwrap();

        let ids: Vec<UnitID> = map
            .tiles
            .iter_locs()
            .map(|loc| {
                LocationGridI::get(&map.tiles, loc)
                    .unwrap()
                    .unit
                    .as_ref()
                    .unwrap()
                    .id
            })
            .collect();

        let fighter_ids = &ids[..=5];
        let carrier_id = ids[6];
        let transport_id = ids[7];
        let armor_ids = &ids[8..];

        // First try carrying an armor on a fighter (not gonna work)
        assert_eq!(
            map.carry_unit_by_id(fighter_ids[0], armor_ids[0]),
            Err(GameError::UnitHasNoCarryingSpace {
                carrier_id: fighter_ids[0]
            })
        );

        // Then try the reverse, carrying a fighter on an armor (should fail)
        assert_eq!(
            map.carry_unit_by_id(armor_ids[0], fighter_ids[0]),
            Err(GameError::UnitHasNoCarryingSpace {
                carrier_id: armor_ids[0]
            })
        );

        // Now for the first five fighters, carry them on the carrier
        for i in 0..UnitType::Carrier.carrying_capacity() {
            assert_eq!(map.carry_unit_by_id(carrier_id, fighter_ids[i]), Ok(i + 1));

            // Try carrying on the transport too
            assert_eq!(
                map.carry_unit_by_id(transport_id, fighter_ids[i]),
                Err(GameError::WrongTransportMode {
                    carried_id: fighter_ids[i],
                    carried_transport_mode: TransportMode::Air,
                    carrier_transport_mode: TransportMode::Land,
                })
            );

            // Make sure everything's still in place after the failed move
            assert!(map.unit_by_id(fighter_ids[i]).is_some());

            assert!(map.unit_by_id(transport_id).is_some());
        }

        // Now carry the sixth, which won't fit
        assert_eq!(
            map.carry_unit_by_id(carrier_id, fighter_ids[5]),
            Err(GameError::InsufficientCarryingSpace {
                carried_id: fighter_ids[5]
            })
        );

        // Carry the first five armor on the transport
        for i in 0..UnitType::Transport.carrying_capacity() {
            assert_eq!(map.carry_unit_by_id(transport_id, armor_ids[i]), Ok(i + 1),);

            assert_eq!(
                map.carry_unit_by_id(carrier_id, armor_ids[i]),
                Err(GameError::WrongTransportMode {
                    carried_id: armor_ids[i],
                    carried_transport_mode: TransportMode::Land,
                    carrier_transport_mode: TransportMode::Air,
                })
            );

            // Make sure everything's still in place after the failed move
            assert!(map.unit_by_id(armor_ids[i]).is_some());

            assert!(map.unit_by_id(carrier_id).is_some());
        }

        // The straw that breaks the transport's back: the sixth armor
        assert_eq!(
            map.carry_unit_by_id(transport_id, armor_ids[4]),
            Err(GameError::InsufficientCarryingSpace {
                carried_id: armor_ids[4]
            })
        );
    }

    #[test]
    fn test_player_unit_by_id_mut() {
        let mut map = MapData::try_from("iI").unwrap();
        let id1 = map.toplevel_unit_by_loc(Location::new(0, 0)).unwrap().id;
        let id2 = map.toplevel_unit_by_loc(Location::new(1, 0)).unwrap().id;

        assert_eq!(map.player_unit_by_id_mut(0, id1).unwrap().id, id1);
        assert_eq!(map.player_unit_by_id_mut(1, id1), None);
        assert_eq!(map.player_unit_by_id_mut(0, id2), None);
        assert_eq!(map.player_unit_by_id_mut(1, id2).unwrap().id, id2);
    }

    /// Make sure we pop the right unit when popping a carried unit by ID
    #[test]
    fn test_pop_carried_unit_by_id() {
        let mut map = MapData::try_from("it").unwrap();
        let infantry_id = map.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
        let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();

        // Carry the unit
        map.carry_unit_by_id(transport_id, infantry_id).unwrap();

        // Now pop it
        let unit = map.pop_unit_by_id(infantry_id).unwrap();
        assert_eq!(unit.id, infantry_id, "Popped the wrong unit");
    }

    /// Make sure pop_toplevel_unit_by_id only pops toplevel units with the right ID, not carrier units which occupy the
    /// same location but don't have the specified ID
    #[test]
    fn test_pop_toplevel_unit_by_id() {
        let mut map = MapData::try_from("it").unwrap();
        let infantry_id = map.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
        let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();

        // Carry the unit
        map.carry_unit_by_id(transport_id, infantry_id).unwrap();

        // Now pop it
        assert_eq!(map.pop_toplevel_unit_by_id(infantry_id), None);
    }
}
