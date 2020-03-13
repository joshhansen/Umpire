use crate::{
    game::player::PlayerTurnControl,
    log::LogTarget,
    ui::TermUI,
};

use super::{
    IMode,
    Mode,
    ModeStatus,
};

pub(in crate::ui) struct SetProductionsMode{}
impl IMode for SetProductionsMode {
    fn run(&self, game: &mut PlayerTurnControl, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
        if game.production_set_requests().next().is_none() {
            ui.log_message("Productions set.".to_string());
            *mode = Mode::TurnResume;
            return ModeStatus::Continue;
        }

        let city_loc = game.production_set_requests().next().unwrap();

        *mode = Mode::SetProduction{city_loc};
        ModeStatus::Continue
    }
}