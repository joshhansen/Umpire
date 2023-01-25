use std::{
    fmt,
    io::{Result as IoResult, Write},
};

// Use crossterm to colorize the debug output
use crossterm::{
    style::{Color, ResetColor, SetForegroundColor},
    QueueableCommand,
};

use crate::{
    color::{Colorized, Colors},
    game::{city::City, unit::Unit, Aligned, AlignedMaybe, Alignment},
    ui::Draw,
    util::Location,
};

use super::Terrain;

//FIXME Cleaner Debug impl
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub terrain: Terrain,
    pub unit: Option<Unit>,
    pub city: Option<City>,
    pub loc: Location,
}

impl Tile {
    pub fn new(terrain: Terrain, loc: Location) -> Tile {
        Tile {
            terrain,
            unit: None,
            city: None,
            loc,
        }
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

impl Draw for Tile {
    fn draw_no_flush(
        &mut self,
        game: &crate::game::PlayerTurnControl,
        stdout: &mut std::io::Stdout,
        palette: &crate::color::Palette,
    ) -> IoResult<()> {
        // If there's a unit, show the unit
        if let Some(ref unit) = self.unit {
            // Capitalize if it belongs to player 1
            if unit.belongs_to_player(1) {
                stdout.queue(SetForegroundColor(Color::Red)).unwrap();
            } else {
                stdout.queue(SetForegroundColor(Color::White)).unwrap();
            };

            let result = write!(stdout, "{}", unit.type_.key());
            stdout.queue(ResetColor).unwrap();
            return result;
        }

        // If there's a city, show the city
        if let Some(ref city) = self.city {
            if city.is_neutral() {
                stdout.queue(SetForegroundColor(Color::DarkGrey)).unwrap();
            } else if city.belongs_to_player(1) {
                stdout.queue(SetForegroundColor(Color::Red)).unwrap();
            } else {
                stdout.queue(SetForegroundColor(Color::White)).unwrap();
            }
            let result = write!(stdout, "#");
            stdout.queue(ResetColor).unwrap();
            return result;
        }

        // Otherwise, show the terrain
        let result = match self.terrain {
            Terrain::Land => {
                stdout.queue(SetForegroundColor(Color::Green)).unwrap();
                write!(stdout, "·")
            }
            Terrain::Water => {
                stdout.queue(SetForegroundColor(Color::Blue)).unwrap();
                write!(stdout, "~")
            }
        };

        stdout.queue(ResetColor).unwrap();

        result
    }
}

#[cfg(test)]
mod test {
    use crate::{
        game::{
            map::{Terrain, Tile},
            unit::{Unit, UnitID, UnitType},
            Alignment,
        },
        util::Location,
    };

    #[test]
    fn test_tile() {
        let loc = Location { x: 10, y: 10 };
        let terrain = Terrain::Land;

        let tile = Tile::new(terrain, loc);

        assert_eq!(tile.unit, None);

        let mut tile = tile;

        let unit = Unit::new(
            UnitID::new(0),
            loc,
            UnitType::Infantry,
            Alignment::Neutral,
            "Mordai Nowhere",
        );
        let unit2 = unit.clone();
        tile.set_unit(unit);
        assert_eq!(tile.unit, Some(unit2));
    }
}
