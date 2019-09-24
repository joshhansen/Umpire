use crate::{
    game::Game,
    log::{LogTarget,Message,MessageSource},
    ui::{
        Draw,
        TermUI,
    },
};

use super::{
    IMode,
    Mode,
};

pub struct TurnStartMode {}
impl IMode for TurnStartMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        ui.current_player.draw(game, &mut ui.stdout, &ui.palette);

        ui.log_message(Message {
            text: format!("Turn {}, player {} go!", game.turn(), game.current_player()),
            mark: Some('_'),
            fg_color: None,
            bg_color: None,
            source: Some(MessageSource::Mode)
        });

        *mode = Mode::TurnResume;

        true
    }
}