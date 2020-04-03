use crate::{
    game::player::PlayerTurnControl,
    log::{Message,MessageSource},
    ui::UI,
};

use super::{
    IMode,
    Mode,
    ModeStatus,
};

pub(in crate::ui) struct TurnStartMode {}
impl IMode for TurnStartMode {
    fn run<U:UI>(&self, game: &mut PlayerTurnControl, ui: &mut U, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
        ui.draw_current_player(game);

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