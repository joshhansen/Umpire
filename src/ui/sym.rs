//! Symbols used by the text UI

use crate::{
    game::{
        map::{Terrain,Tile},
        city::City,
        unit::{
            Unit,
            UnitType,
        },
    },
};

pub(in crate::ui) trait Sym {
    fn sym(&self, unicode: bool) -> &'static str;
}

#[derive(Copy,Clone)]
pub(in crate::ui) enum Symbols {
    Land,
    Ocean,
    City,
}

impl Symbols {
    pub fn get(self, _unicode: bool) -> &'static str {
        match self {
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

//NOTE `Map::draw_tile_no_flush implements a similar symbol selection algorithm that allows for city and unit overrides.
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