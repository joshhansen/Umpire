use crate::{
    game::Game,
    ui::TermUI,
};

use super::{
    IMode,
    Mode,
};

pub struct GetOrdersMode {}
impl IMode for GetOrdersMode {
    fn run(&self, game: &mut Game, _ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        if let Some(unit_id) = game.unit_orders_requests().next() {
            *mode = Mode::GetUnitOrders{unit_id, first_move:true};
        } else {
            *mode = Mode::TurnResume;
        }
        
        true
    }
}