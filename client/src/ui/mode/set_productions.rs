use common::game::player::PlayerTurnControl;

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct SetProductionsMode {}
impl IMode for SetProductionsMode {
    fn run<U: UI>(
        &self,
        game: &mut PlayerTurnControl,
        ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        if game.production_set_requests().next().is_none() {
            ui.log_message("Productions set.".to_string());
            *mode = Mode::TurnResume;
            return ModeStatus::Continue;
        }

        let city_loc = game.production_set_requests().next().unwrap();

        *mode = Mode::SetProduction { city_loc };
        ModeStatus::Continue
    }
}
