use crate::{
    game::player::PlayerTurnControl,
    log::{LogTarget,Message,MessageSource},
    ui::{
        Draw,
        TermUI,
    },
};

use super::{
    IMode,
    Mode,
    ModeStatus,
};

pub(in crate::ui) struct TurnStartMode {}
impl IMode for TurnStartMode {
    fn run(&self, game: &mut PlayerTurnControl, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
        ui.current_player.draw(game, &mut ui.stdout, &ui.palette);

        // A newline for spacing
        ui.log_message("");

        ui.log_message(Message {
            text: format!("Turn {}, player {} go!", game.turn(), game.current_player()),
            mark: Some('_'),
            fg_color: None,
            bg_color: None,
            source: Some(MessageSource::Mode)
        });

        *mode = Mode::TurnResume;

        ModeStatus::Continue
    }
}