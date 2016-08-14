extern crate termion;

use termion::color::AnsiValue;

use unit::{Alignment,City,Unit,alignment_color,Aligned,Sym};

#[derive(Clone,PartialEq)]
pub enum TerrainType {
    WATER,
    LAND,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

#[derive(Clone)]
pub struct Terrain {
    pub type_: TerrainType,
}

impl Terrain {
    pub fn water() -> Terrain {
        Terrain{ type_: TerrainType::WATER }
    }

    pub fn land() -> Terrain {
        Terrain{ type_: TerrainType::LAND }
    }

    pub fn color(&self) -> AnsiValue {
        match self.type_ {
            TerrainType::WATER => AnsiValue(12),
            TerrainType::LAND => AnsiValue(10),
            // TerrainType::CITY => AnsiValue(245)
        }
    }
}

#[derive(Clone)]
pub struct Tile {
    pub terrain: Terrain,
    pub unit: Option<Unit>,
    pub city: Option<City>,
    pub x: u16,
    pub y: u16
}

impl Tile {
    fn new(terrain: Terrain, x:u16, y:u16) -> Tile {
        Tile{ terrain: terrain, unit: None, city: None, x: x, y: y }
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
                Some(ref city) => Some(alignment_color(city.alignment()))
            },
            Some(ref last_unit) => Some(alignment_color(last_unit.alignment))
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


pub mod draw;
pub mod gen;
