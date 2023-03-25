use std::io::Write;

use common::game::obs::Obs;

use crate::Draw;

impl Draw for Obs {
    fn draw_no_flush(
        &mut self,
        game: &common::game::player::PlayerTurnControl,
        stdout: &mut std::io::Stdout,
        palette: &crate::color::Palette,
    ) -> std::io::Result<()> {
        match self {
            Obs::Observed { tile, .. } => tile.draw(game, stdout, palette),
            Obs::Unobserved => write!(stdout, "?"),
        }
    }
}
