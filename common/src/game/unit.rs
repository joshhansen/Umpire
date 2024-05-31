//! Abstract representation of units and cities and their interactions.

pub mod orders;

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    colors::{Colorized, Colors},
    game::{
        alignment::{Aligned, Alignment},
        combat::CombatCapable,
        map::{Terrain, Tile},
        obs::Observer,
        GameError,
    },
    name::Named,
    util::{Located, Location},
};

use self::orders::Orders;

use super::{ai::fX, move_::MoveError, UmpireResult};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct UnitID {
    id: u64,
}
impl UnitID {
    pub fn new(id: u64) -> Self {
        Self { id }
    }
    pub fn next(self) -> Self {
        UnitID { id: self.id + 1 }
    }
}

impl Default for UnitID {
    fn default() -> Self {
        UnitID::new(0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum TransportMode {
    Land,
    Sea,
    Air,
}
impl TransportMode {
    /// Determine whether a unit with this transport mode can operate on terrain of the given type
    pub fn can_traverse(&self, terrain: Terrain) -> bool {
        match self {
            TransportMode::Land => terrain == Terrain::Land,
            TransportMode::Sea => terrain == Terrain::Water,
            TransportMode::Air => terrain == Terrain::Land || terrain == Terrain::Water,
        }
    }

    pub fn default_terrain(&self) -> Terrain {
        match self {
            TransportMode::Land => Terrain::Land,
            TransportMode::Sea => Terrain::Water,
            TransportMode::Air => Terrain::Land,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct CarryingSpace {
    owner: Alignment,
    accepted_transport_mode: TransportMode,
    capacity: usize,
    space: Vec<Unit>,
}
impl CarryingSpace {
    fn new(owner: Alignment, accepted_transport_mode: TransportMode, capacity: usize) -> Self {
        Self {
            owner,
            accepted_transport_mode,
            capacity,
            space: Vec::with_capacity(capacity),
        }
    }

    /// Check for any problems that would prohibit us from carrying the unit
    fn carry_status(&self, unit: &Unit) -> Result<(), GameError> {
        if self.owner != unit.alignment {
            return Err(GameError::OnlyAlliesCarry {
                carried_id: unit.id,
                carrier_alignment: self.owner,
                carried_alignment: unit.alignment,
            });
        }

        if unit.type_.transport_mode() != self.accepted_transport_mode {
            return Err(GameError::WrongTransportMode {
                carried_id: unit.id,
                carrier_transport_mode: self.accepted_transport_mode,
                carried_transport_mode: unit.type_.transport_mode(),
            });
        }

        debug_assert!(self.space.len() <= self.capacity);

        if self.space.len() == self.capacity {
            return Err(GameError::InsufficientCarryingSpace {
                carried_id: unit.id,
            });
        }

        Ok(())
    }

    fn can_carry_unit(&self, unit: &Unit) -> bool {
        self.carry_status(unit).is_ok()
    }

    /// Carry the given unit
    ///
    /// Returns the number of carried units (including this new one) on success. A number of errors issue if there is
    /// a mismatch of unit alignment with carrier alignment, if the accepted transport mode doesn't match, and if the
    /// carrying space is already full.
    fn carry(&mut self, unit: Unit) -> Result<usize, GameError> {
        self.carry_status(&unit)?;

        self.space.push(unit);
        Ok(self.space.len())
    }

    fn release_by_id(&mut self, id: UnitID) -> Option<Unit> {
        self.space
            .iter()
            .position(|carried_unit| carried_unit.id == id)
            .map(|carried_unit_idx| self.space.remove(carried_unit_idx))
    }

    fn units_held(&self) -> usize {
        self.space.len()
    }

    fn carried_units(&self) -> impl Iterator<Item = &Unit> {
        self.space.iter()
    }

    fn carried_units_mut(&mut self) -> impl Iterator<Item = &mut Unit> {
        self.space.iter_mut()
    }
}

pub const POSSIBLE_UNIT_TYPES: usize = 10;

/// How many unit types there are, counting city as a unit type
pub const POSSIBLE_UNIT_TYPES_WRIT_LARGE: usize = POSSIBLE_UNIT_TYPES + 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum UnitType {
    Infantry,
    Armor,
    Fighter,
    Bomber,
    Transport,
    Destroyer,
    Submarine,
    Cruiser,
    Battleship,
    Carrier,
}

impl UnitType {
    pub const fn values() -> [UnitType; POSSIBLE_UNIT_TYPES] {
        [
            UnitType::Infantry,
            UnitType::Armor,
            UnitType::Fighter,
            UnitType::Bomber,
            UnitType::Transport,
            UnitType::Destroyer,
            UnitType::Submarine,
            UnitType::Cruiser,
            UnitType::Battleship,
            UnitType::Carrier,
        ]
    }

    pub fn max_hp(self) -> u16 {
        match self {
            UnitType::Infantry | UnitType::Fighter => 1,
            UnitType::Armor | UnitType::Bomber | UnitType::Destroyer | UnitType::Submarine => 2,
            UnitType::Transport => 3,
            UnitType::Cruiser => 4,
            UnitType::Battleship => 8,
            UnitType::Carrier => 6,
        }
    }

    /// The number of turns a city must dedicate its production to the unit type to produce a single unit of that type
    pub fn cost(self) -> u16 {
        match self {
            UnitType::Infantry => 6,
            UnitType::Armor => 11, // Cheaper per HP than infantry - trade first-mover advantage for long-term efficiency
            UnitType::Fighter => 12,
            UnitType::Bomber => 18, // Longer range AND tougher than fighters
            UnitType::Destroyer | UnitType::Submarine => 24,
            UnitType::Transport => 30,
            UnitType::Cruiser => 36,
            UnitType::Carrier => 48,
            UnitType::Battleship => 60,
        }
    }

    pub fn key(self) -> char {
        match self {
            UnitType::Infantry => 'i',
            UnitType::Armor => 'a',
            UnitType::Fighter => 'f',
            UnitType::Bomber => 'b',
            UnitType::Transport => 't',
            UnitType::Destroyer => 'd',
            UnitType::Submarine => 's',
            UnitType::Cruiser => 'c',
            UnitType::Battleship => 'p',
            UnitType::Carrier => 'k',
        }
    }

    pub fn sight_distance(self) -> u16 {
        match self {
            UnitType::Infantry | UnitType::Armor | UnitType::Transport => 2,
            UnitType::Destroyer | UnitType::Submarine | UnitType::Cruiser => 3,
            UnitType::Fighter | UnitType::Bomber | UnitType::Battleship | UnitType::Carrier => 4,
        }
    }

    //TODO Replace with impl From<char>
    pub fn try_from_key(c: char) -> Result<UnitType, ()> {
        for unit_type in &UnitType::values() {
            if unit_type.key() == c {
                return Ok(*unit_type);
            }
        }
        Err(())
    }

    /// Determine whether a unit of this type could potentially move to a particular tile
    /// (maybe requiring combat to do so).
    ///
    /// If a city is present, this will always be true. Otherwise, it will be determined by the match between
    /// the unit's capabilities and the terrain (e.g. planes over water, but not tanks over water).
    pub fn can_move_on_tile(self, tile: &Tile) -> bool {
        tile.city.is_some() || self.transport_mode().can_traverse(tile.terrain)
    }

    /// Determine whether a unit of this type could actually occupy a particular tile (maybe requiring combat to do so).
    pub fn can_occupy_tile(self, tile: &Tile) -> bool {
        if tile.city.is_some() {
            self.can_occupy_cities()
        } else {
            self.transport_mode().can_traverse(tile.terrain)
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            UnitType::Infantry => "Infantry",
            UnitType::Armor => "Armor",
            UnitType::Fighter => "Fighter",
            UnitType::Bomber => "Bomber",
            UnitType::Transport => "Transport",
            UnitType::Destroyer => "Destroyer",
            UnitType::Submarine => "Submarine",
            UnitType::Cruiser => "Cruiser",
            UnitType::Battleship => "Battleship",
            UnitType::Carrier => "Carrier",
        }
    }

    pub fn transport_mode(self) -> TransportMode {
        match self {
            UnitType::Infantry | UnitType::Armor => TransportMode::Land,
            UnitType::Fighter | UnitType::Bomber => TransportMode::Air,
            UnitType::Transport
            | UnitType::Destroyer
            | UnitType::Submarine
            | UnitType::Cruiser
            | UnitType::Battleship
            | UnitType::Carrier => TransportMode::Sea,
        }
    }

    pub fn carrying_capacity(self) -> usize {
        match self {
            UnitType::Carrier => 5,
            UnitType::Transport => 4,
            _ => 0,
        }
    }

    /// Can this type of unit occupy cities?
    pub fn can_occupy_cities(self) -> bool {
        self.transport_mode() == TransportMode::Land
    }

    pub fn movement_per_turn(&self) -> u16 {
        match self {
            UnitType::Infantry | UnitType::Battleship | UnitType::Carrier => 1,
            UnitType::Armor | UnitType::Transport | UnitType::Submarine | UnitType::Cruiser => 2,
            UnitType::Bomber | UnitType::Destroyer => 3,
            UnitType::Fighter => 5,
        }
    }

    /// The starting fuel configuration for units of this type
    pub fn fuel(&self) -> Fuel {
        match self {
            UnitType::Fighter => Fuel::limited(20),
            UnitType::Bomber => Fuel::limited(30),
            _ => Fuel::Unlimited,
        }
    }

    pub fn can_traverse(&self, terrain: Terrain) -> bool {
        self.transport_mode().can_traverse(terrain)
    }

    pub fn default_terrain(&self) -> Terrain {
        self.transport_mode().default_terrain()
    }

    fn new_carrying_space_for(self, alignment: Alignment) -> Option<CarryingSpace> {
        match self {
            UnitType::Transport => Some(CarryingSpace::new(
                alignment,
                TransportMode::Land,
                self.carrying_capacity(),
            )),
            UnitType::Carrier => Some(CarryingSpace::new(
                alignment,
                TransportMode::Air,
                self.carrying_capacity(),
            )),
            _ => None,
        }
    }

    /// One-hot feature encoding of this unit type
    pub fn features(self) -> [fX; POSSIBLE_UNIT_TYPES] {
        let mut feats = [0.0; POSSIBLE_UNIT_TYPES];
        let idx = UnitType::values()
            .into_iter()
            .position(|ut| self == ut)
            .unwrap();
        feats[idx] = 1.0;
        feats
    }

    /// One-hot feature encoding of this unit type
    ///
    /// Includes assertion of non-city-ness
    pub fn features_writ_large(self) -> [fX; POSSIBLE_UNIT_TYPES_WRIT_LARGE] {
        let mut feats = [0.0; POSSIBLE_UNIT_TYPES_WRIT_LARGE];
        let idx = UnitType::values()
            .into_iter()
            .position(|ut| self == ut)
            .unwrap();
        feats[idx] = 1.0;
        feats
    }

    /// Zeros of the same length as the output of `features`
    pub fn none_features() -> [fX; POSSIBLE_UNIT_TYPES] {
        [0.0; POSSIBLE_UNIT_TYPES]
    }

    pub fn none_features_writ_large(is_city: bool) -> [fX; POSSIBLE_UNIT_TYPES_WRIT_LARGE] {
        if is_city {
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]
        } else {
            [0.0; POSSIBLE_UNIT_TYPES_WRIT_LARGE]
        }
    }
}

impl fmt::Display for UnitType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl PartialOrd for UnitType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.cost().partial_cmp(&other.cost())
    }
}

impl Ord for UnitType {
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp = self.cost().cmp(&other.cost());
        if cmp == Ordering::Equal {
            self.key().cmp(&other.key())
        } else {
            cmp
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Fuel {
    Unlimited,
    Limited { max: u16, remaining: u16 },
}
impl Fuel {
    pub fn limited(max_fuel: u16) -> Self {
        Self::Limited {
            max: max_fuel,
            remaining: max_fuel,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Unit {
    pub id: UnitID,
    pub loc: Location,
    pub type_: UnitType,
    pub alignment: Alignment,
    hp: u16,
    max_hp: u16,
    pub moves_remaining: u16,
    name: String,
    pub orders: Option<Orders>,
    carrying_space: Option<CarryingSpace>,
    pub fuel: Fuel,
}

impl Unit {
    pub fn new<S: Into<String>>(
        id: UnitID,
        loc: Location,
        type_: UnitType,
        alignment: Alignment,
        name: S,
    ) -> Self {
        let max_hp = type_.max_hp();
        Unit {
            id,
            loc,
            type_,
            alignment,
            hp: max_hp,
            max_hp,
            moves_remaining: type_.movement_per_turn(),
            name: name.into(),
            orders: None,
            carrying_space: type_.new_carrying_space_for(alignment),
            fuel: type_.fuel(),
        }
    }

    pub fn movement_per_turn(&self) -> u16 {
        self.type_.movement_per_turn()
    }

    /// Indicate whether this unit can move (if only theoretically) onto a given tile.
    ///
    /// This is determined as follows:
    ///
    /// If the tile contains a unit:
    ///    if the unit is unfriendly, then defer to terrain features / city presence
    ///    if the unit is friendly:
    ///        if the unit has appropriate carrying space for this unit, then we can move on the tile
    ///        otherwise, we cannot move on the tile
    /// otherwise, defer to terrain features / city presence
    ///
    /// NOTE: This method (and related) duplicate funcionality of the move methods themselves, but more efficiently.
    ///       Take care with the repetition to ensure consistency
    pub fn can_move_on_tile(&self, tile: &Tile) -> bool {
        if let Some(ref unit) = tile.unit {
            if !unit.is_friendly_to(self) {
                self.type_.can_move_on_tile(tile)
            } else {
                unit.can_carry_unit(self)
            }
        } else if let Some(ref city) = tile.city {
            if city.is_friendly_to(self) {
                self.type_.can_move_on_tile(tile)
            } else {
                self.can_occupy_cities()
            }
        } else {
            self.type_.can_move_on_tile(tile)
        }
    }

    /// Could this unit attack the given tile if it were adjacent?
    ///
    /// This basically amounts to whether there is an enemy city or unit on the tile
    pub fn can_attack_tile(&self, tile: &Tile) -> bool {
        tile.unit
            .as_ref()
            .map(|_| true)
            .or_else(|| tile.city.as_ref().map(|_| true))
            .unwrap_or(false)
    }

    pub fn moves_remaining(&self) -> u16 {
        self.moves_remaining
    }

    pub(in crate::game) fn movement_complete(&mut self) {
        self.moves_remaining = 0;
    }

    pub(in crate::game) fn record_movement(&mut self, moves: u16) -> UmpireResult<u16> {
        if let Fuel::Limited { max: _, remaining } = self.fuel {
            if remaining < moves {
                return Err(GameError::MoveError(MoveError::InsufficientFuel));
            }
        }

        if self.moves_remaining < moves {
            return Err(GameError::MoveError(MoveError::RemainingMovesExceeded {
                intended_distance: moves,
                moves_remaining: self.moves_remaining,
            }));
        }

        self.moves_remaining -= moves;

        if let Fuel::Limited { remaining, .. } = &mut self.fuel {
            *remaining -= moves;
        }

        Ok(self.moves_remaining)
    }

    pub(in crate::game) fn refresh_moves_remaining(&mut self) {
        self.moves_remaining = self.movement_per_turn();
    }

    pub(in crate::game) fn can_carry_unit(&self, unit: &Unit) -> bool {
        if let Some(ref carrying_space) = self.carrying_space {
            carrying_space.can_carry_unit(unit)
        } else {
            false
        }
    }

    /// Check for any error conditions we would encounter if we did try to carry the given unit
    pub(in crate::game) fn carry_status(&self, unit: &Unit) -> Result<(), GameError> {
        if let Some(ref carrying_space) = self.carrying_space {
            carrying_space.carry_status(unit)
        } else {
            Err(GameError::UnitHasNoCarryingSpace {
                carrier_id: self.id,
            })
        }
    }

    /// Carry a unit in this unit's carrying space.
    ///
    /// For example, make a Transport carry an Armor.
    ///
    /// This method call should only be called by MapData's carry_unit method.
    pub(in crate::game) fn carry(&mut self, mut unit: Unit) -> Result<usize, GameError> {
        self.carry_status(&unit)?;

        // We can set this before we've actually don the carry because we checked for any possible errors above
        unit.loc = self.loc;

        self.carrying_space.as_mut().unwrap().carry(unit)
    }

    pub(in crate::game) fn release_by_id(&mut self, carried_unit_id: UnitID) -> Option<Unit> {
        if let Some(ref mut carrying_space) = self.carrying_space {
            carrying_space.release_by_id(carried_unit_id)
        } else {
            None
        }
    }

    pub fn carried_units(&self) -> impl Iterator<Item = &Unit> {
        self.carrying_space
            .iter()
            .flat_map(|carrying_space| carrying_space.carried_units())
    }

    pub(in crate::game) fn carried_units_mut(&mut self) -> impl Iterator<Item = &mut Unit> {
        self.carrying_space
            .iter_mut()
            .flat_map(|carrying_space| carrying_space.carried_units_mut())
    }

    /// Is the unit a carrier?
    pub fn carrier(&self) -> bool {
        self.carrying_space.is_some()
    }

    pub fn short_desc(&self) -> String {
        format!("{} \"{}\"", self.type_, self.name)
    }

    pub fn medium_desc(&self) -> String {
        format!("{} [{}/{}]", self.short_desc(), self.hp, self.max_hp)
    }

    /// Can this unit occupy cities?
    pub fn can_occupy_cities(&self) -> bool {
        self.type_.can_occupy_cities()
    }

    pub fn has_orders(&self) -> bool {
        self.orders.is_some()
    }

    pub fn set_orders(&mut self, orders: Orders) -> Option<Orders> {
        self.orders.replace(orders)
    }

    pub fn clear_orders(&mut self) -> Option<Orders> {
        self.orders.take()
    }

    pub fn activate(&mut self) {
        self.orders = None;
        for carried_unit in self.carried_units_mut() {
            carried_unit.orders = None;
        }
    }

    pub fn transport_mode(&self) -> TransportMode {
        self.type_.transport_mode()
    }

    pub fn max_hp(&self) -> u16 {
        self.max_hp
    }

    /// If the unit's fuel is limited, refill it. Otherwise, do nothing.
    ///
    /// Returns the quantity of fuel refilled.
    pub fn refuel(&mut self) -> u16 {
        if let Fuel::Limited { max, remaining } = &mut self.fuel {
            let refueled = *max - *remaining;
            *remaining = *max;
            refueled
        } else {
            0
        }
    }
}

impl Aligned for Unit {
    fn alignment(&self) -> Alignment {
        self.alignment
    }
}

impl CombatCapable for Unit {
    fn hp(&self) -> u16 {
        self.hp
    }
    fn max_hp(&self) -> u16 {
        self.max_hp
    }
}

impl Colorized for Unit {
    fn color(&self) -> Option<Colors> {
        self.alignment.color()
    }
}

impl Located for Unit {
    fn loc(&self) -> Location {
        self.loc
    }
}

impl Named for Unit {
    fn name(&self) -> &String {
        &self.name
    }
}

impl Observer for Unit {
    fn sight_distance(&self) -> u16 {
        self.type_.sight_distance()
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = write!(f, "{} {}", self.alignment, self.medium_desc());
        if let Some(ref carrying_space) = self.carrying_space {
            result = result.and(write!(f, " carrying {} units", carrying_space.units_held()));
        }
        result
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;

    use crate::{
        game::{
            city::{City, CityID},
            map::{dijkstra::Source, LocationGrid, Terrain, Tile},
            obs::{Obs, ObsTracker},
            unit::{Alignment, Observer, Unit, UnitID, UnitType},
        },
        util::{Dims, Location, Wrap2d},
    };

    #[test]
    fn test_observations() {
        let map_s = "\
x   o    x
x  ooo   x
x ooioo  x
x  ooo   x
x   o    x";

        match LocationGrid::<Tile>::try_from(map_s) {
            Err(err) => {
                panic!("Error parsing map: {}", err);
            }
            Ok(map) => {
                assert_eq!(
                    map.dims(),
                    Dims {
                        width: 10,
                        height: 5
                    }
                );

                let width = map_s.lines().map(|line| line.len()).max();
                let height = map_s.lines().count();

                assert_eq!(width, Some(10));
                assert_eq!(height, 5);

                let infantry_loc = Location { x: 4, y: 2 };

                let infantry = Unit::new(
                    UnitID::new(0),
                    infantry_loc,
                    UnitType::Infantry,
                    Alignment::Belligerent { player: 0 },
                    "Lynn Stone",
                );

                // let mut obs_tracker: ObsTracker = ObsTracker::new_fog_of_war(map.dims());
                let mut obs_tracker = ObsTracker::new(map.dims());

                for loc in map.iter_locs() {
                    assert_eq!(*obs_tracker.get(loc), Obs::Unobserved);
                }

                let turn = 0;
                let action_count = 0;

                infantry.observe(&map, turn, action_count, Wrap2d::BOTH, &mut obs_tracker);

                let observed_locs_arr = [
                    Location { x: 4, y: 0 },
                    Location { x: 3, y: 1 },
                    Location { x: 4, y: 1 },
                    Location { x: 5, y: 1 },
                    Location { x: 2, y: 2 },
                    Location { x: 3, y: 2 },
                    Location { x: 4, y: 2 },
                    Location { x: 5, y: 2 },
                    Location { x: 6, y: 2 },
                    Location { x: 3, y: 3 },
                    Location { x: 4, y: 3 },
                    Location { x: 5, y: 3 },
                    Location { x: 4, y: 4 },
                ];
                let observed_locs: BTreeSet<&Location> =
                    BTreeSet::from_iter(observed_locs_arr.iter());

                for loc in map.iter_locs() {
                    assert_eq!(
                        *obs_tracker.get(loc),
                        if observed_locs.contains(&loc) {
                            Obs::Observed {
                                tile: map[loc].clone(),
                                turn,
                                action_count,
                                current: true,
                            }
                        } else {
                            Obs::Unobserved
                        }
                    );
                }

                /*
                x   oo   x
                x  oooo  x
                x ooiioo x
                x  oooo  x
                x   oo   x"
                */
                let mut infantry = infantry;
                infantry.loc = Location { x: 5, y: 2 };

                infantry.observe(&map, turn, action_count, Wrap2d::BOTH, &mut obs_tracker);

                let observed_locs_arr_2 = [
                    Location { x: 5, y: 0 },
                    Location { x: 6, y: 1 },
                    Location { x: 7, y: 2 },
                    Location { x: 6, y: 3 },
                    Location { x: 5, y: 4 },
                ];
                let observed_locs_2: BTreeSet<&Location> =
                    BTreeSet::from_iter(observed_locs_arr_2.iter());

                for loc in map.iter_locs() {
                    assert_eq!(
                        *obs_tracker.get(loc),
                        if observed_locs.contains(&loc) || observed_locs_2.contains(&loc) {
                            Obs::Observed {
                                tile: map[loc].clone(),
                                turn,
                                action_count,
                                current: true,
                            }
                        } else {
                            Obs::Unobserved
                        }
                    );
                }

                obs_tracker.archive();

                for loc in map.iter_locs() {
                    assert_eq!(
                        *obs_tracker.get(loc),
                        if observed_locs.contains(&loc) || observed_locs_2.contains(&loc) {
                            Obs::Observed {
                                tile: map[loc].clone(),
                                turn,
                                action_count,
                                current: false,
                            }
                        } else {
                            Obs::Unobserved
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn test_mobility() {
        let loc = Location { x: 5, y: 5 };
        let loc2 = Location::new(5, 6);

        let infantry = Unit::new(
            UnitID::new(0),
            loc,
            UnitType::Infantry,
            Alignment::Belligerent { player: 0 },
            "Isabel Nash",
        );
        let transport = Unit::new(
            UnitID::new(0),
            loc,
            UnitType::Transport,
            Alignment::Belligerent { player: 0 },
            "Blah blah",
        );
        let friendly_unit = Unit::new(
            UnitID::new(1),
            loc,
            UnitType::Armor,
            Alignment::Belligerent { player: 0 },
            "Lynn Stone",
        );
        let enemy_unit = Unit::new(
            UnitID::new(2),
            loc,
            UnitType::Armor,
            Alignment::Belligerent { player: 1 },
            "James Lindsey",
        );

        let tile1 = Tile::new(Terrain::Land, loc);
        assert!(infantry.can_move_on_tile(&tile1));

        let tile2 = Tile::new(Terrain::Water, loc);
        assert!(!infantry.can_move_on_tile(&tile2));

        let mut tile3 = Tile::new(Terrain::Land, loc);
        tile3.unit = Some(friendly_unit);
        assert!(!infantry.can_move_on_tile(&tile3));

        let mut tile4 = Tile::new(Terrain::Land, loc);
        tile4.unit = Some(enemy_unit.clone());
        assert!(infantry.can_move_on_tile(&tile4));

        {
            let mut tile = Tile::new(Terrain::Land, loc2);
            tile.city = Some(City::new(
                CityID::new(0),
                Alignment::Belligerent { player: 1 },
                loc2,
                "Urbania",
            ));
            assert!(infantry.can_move_on_tile(&tile));

            assert!(!transport.can_move_on_tile(&tile));

            tile.unit = Some(enemy_unit.clone());

            assert!(transport.can_move_on_tile(&tile));
        }

        {
            let mut tile = Tile::new(Terrain::Water, loc2);
            let transport2 = Unit::new(
                UnitID::new(0),
                loc2,
                UnitType::Transport,
                Alignment::Belligerent { player: 0 },
                "Blah blah",
            );
            tile.unit = Some(transport2);

            assert!(!transport.can_move_on_tile(&tile));
        }

        {
            let mut tile = Tile::new(Terrain::Water, loc2);
            tile.city = Some(City::new(
                CityID::new(0),
                Alignment::Belligerent { player: 0 },
                loc2,
                "Urbania",
            ));
            let transport2 = Unit::new(
                UnitID::new(0),
                loc2,
                UnitType::Transport,
                Alignment::Belligerent { player: 0 },
                "Blah blah",
            );
            tile.unit = Some(transport2);

            assert!(!transport.can_move_on_tile(&tile));
        }
    }

    #[test]
    pub fn test_can_carry_unit() {
        let l1 = Location::new(0, 0);
        let l2 = Location::new(1, 0);
        let t1 = Unit::new(
            UnitID::new(0),
            l1,
            UnitType::Transport,
            Alignment::Belligerent { player: 0 },
            "0",
        );
        let t2 = Unit::new(
            UnitID::new(0),
            l2,
            UnitType::Transport,
            Alignment::Belligerent { player: 0 },
            "1",
        );

        assert!(!t1.can_carry_unit(&t2));
    }
}
