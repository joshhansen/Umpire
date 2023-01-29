use std::io::{Result as IoResult, Write};

use crossterm::{
    style::{Color, ResetColor, SetForegroundColor},
    QueueableCommand,
};

use common::game::{
    map::{Terrain, Tile},
    Aligned, AlignedMaybe,
};

use crate::ui::Draw;

impl Draw for Tile {
    fn draw_no_flush(
        &mut self,
        game: &common::game::PlayerTurnControl,
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
