use crate::{
    game::player::PlayerTurnControl,
    ui::UI,
};

use super::{
    IMode,
    Mode,
    ModeStatus,
};

pub(in crate::ui) struct GetOrdersMode {}
impl IMode for GetOrdersMode {
    fn run<U:UI>(&self, game: &mut PlayerTurnControl, _ui: &mut U, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
        if let Some(unit_id) = game.unit_orders_requests().next() {
            *mode = Mode::GetUnitOrders{unit_id, first_move:true};
        } else {
            *mode = Mode::TurnResume;
        }
        
        ModeStatus::Continue
    }
}