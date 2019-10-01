use crate::{
    game::Game,
    ui::TermUI,
};

use super::{
    IMode,
    Mode,
};

pub(in crate::ui) struct CarryOutOrdersMode {}
impl IMode for CarryOutOrdersMode {
    fn run(&self, game: &mut Game, _ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        if let Some(unit_id) = game.units_with_pending_orders().next() {
            let unit = game.unit_by_id(unit_id).unwrap();
            if unit.moves_remaining() > 0 {
                *mode = Mode::CarryOutUnitOrders{unit_id};
            }
        } else {
            *mode = Mode::TurnResume;
        }

        true
    }
}