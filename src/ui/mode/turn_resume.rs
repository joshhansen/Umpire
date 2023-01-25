use crate::{game::player::PlayerTurnControl, ui::UI};

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct TurnResumeMode {}
impl IMode for TurnResumeMode {
    fn run<U: UI>(
        &self,
        game: &mut PlayerTurnControl,
        _ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        if game.production_set_requests().next().is_some() {
            *mode = Mode::SetProductions;
            return ModeStatus::Continue;
        }

        if game.unit_orders_requests().next().is_some() {
            *mode = Mode::GetOrders;
            return ModeStatus::Continue;
        }

        if game.turn_is_done() {
            *mode = Mode::TurnOver;
        }

        ModeStatus::Continue
    }
}
