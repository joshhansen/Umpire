use std::fmt;

use crate::{
    game::{
        Alignment,
        Aligned,
        combat::CombatCapable,
        obs::Observer,
        unit::{
            Located,
            UnitType,
        },
    },
    util::Location,
};

#[derive(Clone,Copy,Debug,Eq,Hash,PartialEq)]
pub struct CityID {
    id: u64
}
impl CityID {
    pub fn new(id: u64) -> Self {
        Self{ id }
    }
    pub fn next(self) -> Self {
        Self{ id: self.id + 1 }
    }
}

pub const CITY_MAX_HP: u16 = 1;

#[derive(Clone,Hash,PartialEq,Eq)]
pub struct City {
    pub id: CityID,
    pub alignment: Alignment,
    pub loc: Location,//NOTE City location is also reflected in the Game::grid matrix, so this could be stale
    hp: u16,
    production: Option<UnitType>,
    pub production_progress: u16,
    name: String,

    /// When set to true, even a unit_under_production of None will not bring this city's production menu up
    ignore_cleared_production: bool,
}
impl City {
    pub fn new<S:Into<String>>(id: CityID, alignment: Alignment, loc: Location, name: S) -> City {
        City {
            id,
            loc,
            alignment,
            hp: CITY_MAX_HP,
            production: None,
            production_progress: 0,
            name: name.into(),
            ignore_cleared_production: false,
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn set_production(&mut self, production: UnitType) {
        self.production = Some(production);
    }

    pub fn clear_production_and_ignore(&mut self) {
        self.production = None;
        self.ignore_cleared_production = true;
    }

    pub fn clear_production_without_ignoring(&mut self) {
        self.production = None;
        self.ignore_cleared_production = false;
    }

    // pub fn set_production(&mut self, production: Option<UnitType>) {
    //     self.ignore_cleared_production = production.is_none();
    //     self.unit_under_production = production;
    // }

    pub fn production(&self) -> Option<UnitType> {
        self.production
    }

    pub fn ignore_cleared_production(&self) -> bool {
        self.ignore_cleared_production
    }
}

impl CombatCapable for City {
    fn hp(&self) -> u16 { self.hp }
    fn max_hp(&self) -> u16 { CITY_MAX_HP }
}

impl fmt::Display for City {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = write!(f, "City \"{}\"", self.name);
        if let Some(ref produced_unit) = self.production {
            result = result.and(write!(f, ", producing {} ({}/{})", produced_unit, self.production_progress, produced_unit.cost()));
        }
        result
    }
}

impl fmt::Debug for City {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Located for City {
    fn loc(&self) -> Location { self.loc }
}

impl Aligned for City {
    fn alignment(&self) -> Alignment {
        self.alignment
    }
}

impl Observer for City {
    fn sight_distance(&self) -> u16 {
        3
    }
}