use std::io::Write;

use async_trait::async_trait;

use common::game::obs::Obs;

use crate::Draw;

#[async_trait]
impl Draw for Obs {
    async fn draw_no_flush(
        &mut self,
        game: &common::game::player::PlayerTurnControl,
        stdout: &mut std::io::Stdout,
        palette: &crate::color::Palette,
    ) -> std::io::Result<()> {
        match self {
            Obs::Observed { tile, .. } => tile.draw(game, stdout, palette).await,
            Obs::Unobserved => write!(stdout, "?"),
        }
    }
}