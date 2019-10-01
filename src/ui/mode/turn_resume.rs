use crate::{
    game::Game,
    ui::TermUI,
};

use super::{
    IMode,
    Mode,
};

pub(in crate::ui) struct TurnResumeMode{}
impl IMode for TurnResumeMode {
    fn run(&self, game: &mut Game, _ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        // Process production set requests
        if game.production_set_requests().next().is_some() {
            *mode = Mode::SetProductions;
            return true;
        }

        if game.units_with_pending_orders().next().is_some() {
            *mode = Mode::CarryOutOrders;
            return true;
        }

        if game.unit_orders_requests().next().is_some() {
            *mode = Mode::GetOrders;
            return true;
        }

        if game.turn_is_done() {
            *mode = Mode::TurnOver;
        }

        true
    }
}