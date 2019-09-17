//! Symbols used by the text UI

use crate::{
    game::{
        map::{Terrain,Tile},
        unit::{
            City,
            Unit,
            UnitType,
        },
    },
};

pub trait Sym {
    fn sym(&self, unicode: bool) -> &'static str;
}

#[derive(Copy,Clone)]
pub enum Symbols {
    Land,
    Ocean,
    City,
}

impl Symbols {
    pub fn get(&self, unicode: bool) -> &'static str {
        match *self {
            Symbols::Land => "·",
            Symbols::Ocean => "~",
            Symbols::City => "#",
        }
    }
}

impl Sym for City {
    fn sym(&self, unicode: bool) -> &'static str {
        Symbols::City.get(unicode)
    }
}

impl Sym for Terrain {
    fn sym(&self, unicode: bool) -> &'static str {
        match *self {
            Terrain::Land => Symbols::Land.get(unicode),
            Terrain::Water => Symbols::Ocean.get(unicode),
        }
    }
}

impl Sym for Tile {
    fn sym(&self, unicode: bool) -> &'static str {
        if let Some(ref unit) = self.unit {
            unit.sym(unicode)
        } else if let Some(ref city) = self.city {
            city.sym(unicode)
        } else {
            self.terrain.sym(unicode)
        }
    }
}

impl Sym for Unit {
    fn sym(&self, unicode: bool) -> &'static str {
        self.type_.sym(unicode)
    }
}

impl Sym for UnitType {
    fn sym(&self, unicode: bool) -> &'static str {
        match self {
            UnitType::Infantry => "i",
            UnitType::Armor => "A",
            UnitType::Fighter => if unicode{ "✈" } else {"f"},
            UnitType::Bomber => "b",
            UnitType::Transport => "t",
            UnitType::Destroyer => "d",
            UnitType::Submarine => "─",
            UnitType::Cruiser => "c",
            UnitType::Battleship => "B",
            UnitType::Carrier => "C"
        }
    }
}