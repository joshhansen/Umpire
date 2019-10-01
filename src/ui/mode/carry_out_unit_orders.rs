use crate::{
    color::Colors,
    game::{
        Game,
        unit::UnitID,
    },
    log::{LogTarget,Message},
    ui::{
        buf::RectBuffer,
        MoveAnimator,
        TermUI,
    },
    util::Rect,
};

use super::{
    IMode,
    IVisibleMode,
    Mode,
};

pub struct CarryOutUnitOrdersMode {
    pub rect: Rect,
    pub unit_id: UnitID,
}

impl IVisibleMode for CarryOutUnitOrdersMode {
    fn rect(&self) -> Rect {
        self.rect
    }

    fn buf_mut(ui: &mut TermUI) -> &mut RectBuffer {
        ui.sidebar_buf_mut()
    }
}
impl CarryOutUnitOrdersMode {
    fn write_buf(&self, game: &Game, ui: &mut TermUI) {
        let unit = game.unit_by_id(self.unit_id).unwrap();

        let buf = ui.sidebar_buf_mut();
        buf.set_row(0, format!("Unit {} is {}", unit, unit.orders.unwrap().present_progressive_description()));
    }
}
impl IMode for CarryOutUnitOrdersMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        let unit = game.unit_by_id(self.unit_id).unwrap();

        ui.map_scroller.scrollable.center_viewport(unit.loc);

        self.write_buf(game, ui);

        ui.draw(game);

        let orders = unit.orders.as_ref().unwrap();

        match orders.carry_out(self.unit_id, game) {
            Ok(orders_outcome) => {
                if let Some(move_result) = orders_outcome.move_result() {
                    ui.animate_move(game, &move_result);
                }
                *mode = Mode::CarryOutOrders{};
            },
            Err(msg) => {
                // panic!(msg);
                ui.log_message(Message {
                    text: msg,
                    mark: Some('!'),
                    fg_color: Some(Colors::Text),
                    bg_color: Some(Colors::Notice),
                    source: None,
                });
            }
        }

        ui.sidebar_buf_mut().clear();

        true
    }
}