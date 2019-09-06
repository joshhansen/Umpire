//! Abstract map representation
//!
//! Data structures and algorithms for representing and working with the game map.

use std::fmt;

use termion::color::{AnsiValue,Color};

use crate::color::{ColorPair,Palette,PairColorized};
use game::{Aligned,AlignedMaybe,Alignment};
use unit::{City,Sym,Unit};
use util::Location;


#[derive(Clone,PartialEq)]
pub enum Terrain {
    Water,
    Land,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

// impl Terrain {
//     pub fn color(&self, palette: &Palette) -> AnsiValue {
//         match *self {
//             Terrain::Water => AnsiValue(12),
//             Terrain::Land => AnsiValue(10),
//             // Terrain::CITY => AnsiValue(245)
//         }
//     }
// }

impl <C:Color+Copy> PairColorized<C> for Terrain {
    fn color_pair(&self, palette: &Palette<C>) -> Option<ColorPair<C>> {
        Some(match *self {
            Terrain::Water => palette.ocean,
            Terrain::Land => palette.land
        })
    }
}

impl fmt::Display for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Terrain::Water => "Water",
            Terrain::Land => "Land"
        })
    }
}

impl fmt::Debug for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Clone,Debug,PartialEq)]
pub struct Tile {
    pub terrain: Terrain,
    pub unit: Option<Unit>,
    pub city: Option<City>,
    pub loc: Location
}

impl Tile {
    pub fn new(terrain: Terrain, loc: Location) -> Tile {
        Tile{ terrain, unit: None, city: None, loc }
    }

    pub fn sym(&self) -> &'static str {
        if let Some(ref unit) = self.unit {
            unit.sym()
        } else if let Some(ref city) = self.city {
            city.sym()
        } else {
            " "
        }
    }

    // pub fn fg_color(&self, palette: &Palette<dyn Color>) -> Option<AnsiValue> {
    //     match self.unit {
    //         Some(ref last_unit) => Some(last_unit.alignment.color()),
    //         None => match self.city {
    //             Some(ref city) => Some(city.alignment().color()),
    //             None => None
    //         }
    //     }
    // }

    // pub fn bg_color(&self, palette: &Palette<dyn Color>) -> AnsiValue {
    //     self.terrain.color()
    // }

    pub fn pop_unit(&mut self) -> Option<Unit> {
        let unit = self.unit.clone();
        self.unit = None;
        unit
    }

    pub fn set_unit(&mut self, unit: Unit) {
        self.unit = Some(unit);
    }
}

impl <C:Color+Copy> PairColorized<C> for Tile {
    /// A tile's color pair is the color of the foreground, i.e. units, cities, etc.
    fn color_pair(&self, palette: &Palette<C>) -> Option<ColorPair<C>> {

        if let Some(ref last_unit) = self.unit {
            last_unit.alignment.color_pair(palette)
        } else if let Some(ref city) = self.city {
            city.alignment().color_pair(palette)
        } else {
            None
        }
    }
}

impl AlignedMaybe for Tile {
    fn alignment_maybe(&self) -> Option<Alignment> {
        if let Some(ref city) = self.city {
            Some(city.alignment())
        } else if let Some(ref unit) = self.unit {
            Some(unit.alignment())
        } else {
            None
        }
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref city) = self.city {
            if let Some(ref unit) = self.unit {
                write!(f, "{} with {} garrisoned; {}", city, unit, self.terrain)
            } else {
                write!(f, "{}; {}", city, self.terrain)
            }
        } else if let Some(ref unit) = self.unit {
            write!(f, "{} on {}", unit, self.terrain)
        } else {
            write!(f, "{}", self.terrain)
        }
    }
}





pub mod dijkstra;
pub mod gen;
mod grid;
pub mod newmap;

pub use self::grid::LocationGrid;

#[cfg(test)]
mod test {
    use game::Alignment;
    use map::{Terrain,Tile};
    use map::newmap::UnitID;
    use unit::{Unit,UnitType};
    use util::Location;


    #[test]
    fn test_tile() {
        let loc = Location{x: 10, y: 10};
        let terrain = Terrain::Land;

        let tile = Tile::new(terrain, loc);

        assert_eq!(tile.unit, None);

        let mut tile = tile;

        let unit = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Neutral, "Mordai Nowhere");
        let unit2 = unit.clone();
        tile.set_unit(unit);
        assert_eq!(tile.unit, Some(unit2));
    }

}
