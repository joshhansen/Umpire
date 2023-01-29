use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    game::{combat::CombatCapable, obs::Observer, unit::UnitType, Aligned, Alignment},
    util::{Located, Location},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CityID {
    id: u64,
}
impl CityID {
    pub fn new(id: u64) -> Self {
        Self { id }
    }
    pub fn next(self) -> Self {
        Self { id: self.id + 1 }
    }
}
impl Default for CityID {
    fn default() -> Self {
        CityID::new(0)
    }
}

pub const CITY_MAX_HP: u16 = 1;

#[derive(Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct City {
    pub id: CityID,
    pub alignment: Alignment,
    pub loc: Location, //NOTE City location is also reflected in the Game::grid matrix, so this could be stale
    hp: u16,
    production: Option<UnitType>,
    pub production_progress: u16,
    name: String,

    /// When set to true, even a unit_under_production of None will not bring this city's production menu up
    ignore_cleared_production: bool,
}
impl City {
    pub fn new<S: Into<String>>(id: CityID, alignment: Alignment, loc: Location, name: S) -> City {
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

    pub fn short_desc(&self) -> String {
        format!("City {}", self.name)
    }

    /// Set the city's production and return its previous status
    pub fn set_production(&mut self, production: UnitType) -> Option<UnitType> {
        self.production.replace(production)
    }

    /// Clear the city's production but ignore it when looking for un-set productions in the future.
    ///
    /// The user must manually activate it in Examine Mode after this.
    pub fn clear_production_and_ignore(&mut self) {
        self.production = None;
        self.ignore_cleared_production = true;
    }

    /// Clear the city's production, and include it in the future when looking for un-set productions.
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
    fn hp(&self) -> u16 {
        self.hp
    }
    fn max_hp(&self) -> u16 {
        CITY_MAX_HP
    }
}

impl fmt::Display for City {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = write!(f, "{} {}", self.alignment, self.short_desc());
        if let Some(ref produced_unit) = self.production {
            result = result.and(write!(
                f,
                ", producing {} ({}/{})",
                produced_unit,
                self.production_progress,
                produced_unit.cost()
            ));
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
    fn loc(&self) -> Location {
        self.loc
    }
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
