use std::fmt;

// Use crossterm to colorize the debug output
use crossterm::{queue, QueueableCommand, cursor};
use crossterm::style::{Print, SetForegroundColor, SetBackgroundColor, ResetColor, Color};

use crate::{
    color::{Colors,Colorized},
    game::{
        Aligned,
        AlignedMaybe,
        Alignment,
        city::City,
        unit::{Unit},
    },
    util::Location,
};

use super::Terrain;

#[derive(Clone,PartialEq)]
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

    pub fn pop_unit(&mut self) -> Option<Unit> {
        let unit = self.unit.clone();
        self.unit = None;
        unit
    }

    pub fn set_unit(&mut self, unit: Unit) {
        self.unit = Some(unit);
    }

    pub fn all_units(&self) -> Vec<&Unit> {
        if let Some(unit) = self.unit.as_ref() {
            let mut units = Vec::with_capacity(1 + unit.type_.carrying_capacity());

            units.push(unit);
            units.extend(unit.carried_units());

            units
        } else {
            Vec::new()
        }
    }
}

impl Colorized for Tile {
    fn color(&self) -> Option<Colors> {
        if let Some(ref last_unit) = self.unit {
            last_unit.alignment.color()
        } else if let Some(ref city) = self.city {
            city.alignment().color()
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

impl fmt::Debug for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        // If there's a unit, show the unit
        if let Some(ref unit) = self.unit {
            // Capitalize if it belongs to player 1
            if unit.belongs_to_player(1) {
                queue!(f, SetForegroundColor(Color::Red)).unwrap();
            } else {
                queue!(f, SetForegroundColor(Color::White)).unwrap();
            };

            let result = write!(f, "{}", unit.type_.key());
            queue!(f, ResetColor).unwrap();
            return result;
        }

        // If there's a city, show the city
        if let Some(ref city) = self.city {
            if city.belongs_to_player(1) {
                queue!(f, SetForegroundColor(Color::Red)).unwrap();
            } else {
                queue!(f, SetForegroundColor(Color::White)).unwrap();
            }
            let result = write!(f, "#");
            queue!(f, ResetColor).unwrap();
            return result;
        }

        // Otherwise, show the terrain
        let result = match self.terrain {
            Terrain::Land => {
                queue!(f, SetForegroundColor(Color::Green)).unwrap();
                write!(f, "·")
            },
            Terrain::Water => {
                queue!(f, SetForegroundColor(Color::Blue)).unwrap();
                write!(f, "~")
            },
        };

        queue!(f, ResetColor).unwrap();

        result
    }
}

#[cfg(test)]
mod test {
    use crate::{
        game::{
            Alignment,
            map::{
                Terrain,
                Tile,
            },
            unit::{UnitID,Unit,UnitType},
        },
        util::Location
    };

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
