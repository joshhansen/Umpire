extern crate termion;

use unit::Unit;

use termion::color::{Fg, AnsiValue};

#[derive(PartialEq)]
pub enum TerrainType {
    WATER,
    LAND,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

pub struct Terrain {
    pub type_: TerrainType,
    pub x: u16,
    pub y: u16
}

impl Terrain {
    pub fn water(x: u16, y: u16) -> Terrain {
        Terrain{ type_: TerrainType::WATER, x: x, y: y }
    }

    pub fn land(x: u16, y: u16) -> Terrain {
        Terrain{ type_: TerrainType::LAND, x: x, y: y }
    }

    pub fn color(&self) -> AnsiValue {
        match self.type_ {
            TerrainType::WATER => AnsiValue(12),
            TerrainType::LAND => AnsiValue(10),
            // TerrainType::CITY => AnsiValue(245)
        }
    }
}

pub struct Tile {
    pub terrain: Terrain,
    pub units: Vec<Unit>
}

impl Tile {
    fn new(terrain: Terrain) -> Tile {
        Tile{ terrain: terrain, units: Vec::new() }
    }
}


pub mod draw;
pub mod gen;
