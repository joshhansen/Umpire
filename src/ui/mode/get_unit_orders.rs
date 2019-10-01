use std::convert::TryFrom;

use crossterm::{
    KeyEvent,
};

use crate::{
    conf::{self, key_desc},
    game::{
        Game,
        unit::UnitID,
    },
    log::LogTarget,
    ui::{
        audio::Sounds,
        buf::RectBuffer,
        MoveAnimator,
        TermUI,
    },
    util::{Direction,Rect},
};

use super::{
    IMode,
    IVisibleMode,
    KeyStatus,
    Mode,
    StateDisposition,
    cols,
};

pub(in crate::ui) struct GetUnitOrdersMode{
    pub rect: Rect,
    pub unit_id: UnitID,
    pub first_move: bool
}
impl IVisibleMode for GetUnitOrdersMode {
    fn rect(&self) -> Rect {
        self.rect
    }

    fn buf_mut(ui: &mut TermUI) -> &mut RectBuffer {
        ui.sidebar_buf_mut()
    }
}
impl GetUnitOrdersMode {
    fn write_buf(&self, game: &Game, ui: &mut TermUI) {
        let unit = game.unit_by_id(self.unit_id).unwrap();

        let buf = ui.sidebar_buf_mut();
        buf.set_row(0, format!("Get Orders for {}", unit));
        buf.set_row(2, format!("Move: ↖ ↗          {} {}", conf::KEY_UP_LEFT, conf::KEY_UP_RIGHT));
        buf.set_row(3, format!("       ← ↓ ↑ →      {} {} {} {}", conf::KEY_LEFT, conf::KEY_DOWN, conf::KEY_UP, conf::KEY_RIGHT));
        buf.set_row(4, format!("      ↙ ↘          {} {}", conf::KEY_DOWN_LEFT, conf::KEY_DOWN_RIGHT));
        buf.set_row(6, cols("Examine:", conf::KEY_EXAMINE));
        buf.set_row(8, cols("Explore:", conf::KEY_EXPLORE));
        buf.set_row(10, cols("Skip:", key_desc(conf::KEY_SKIP)));
        buf.set_row(12, cols("Sentry:", conf::KEY_SENTRY));
        buf.set_row(14, cols("Quit:", conf::KEY_QUIT));
    }
}
impl IMode for GetUnitOrdersMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        let (unit_loc,unit_type) = {
            let unit = game.unit_by_id(self.unit_id).unwrap();
            ui.log_message(format!("Requesting orders for unit {} at {}", unit, unit.loc));
            (unit.loc,unit.type_)
        };

        if self.first_move {
            ui.play_sound(Sounds::Unit(unit_type));
            ui.map_scroller.scrollable.center_viewport(unit_loc);
        }

        self.write_buf(game, ui);
        ui.draw(game);

        let viewport_loc = ui.map_scroller.scrollable.map_to_viewport_coords(unit_loc, ui.viewport_rect().dims()).unwrap();
        ui.map_scroller.scrollable.draw_tile_and_flush(game, &mut ui.stdout, viewport_loc, false, true, None);

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {

                    if let KeyEvent::Char(c) = key {
                        if let Ok(dir) = Direction::try_from(c) {
                            if let Some(dest) = unit_loc.shift_wrapped(dir, game.map_dims(), game.wrapping()) {
                                match game.move_unit_by_id(self.unit_id, dest) {
                                    Ok(move_result) => {
                                        ui.animate_move(game, &move_result);

                                        if game.unit_orders_requests().any(|unit_id| unit_id==self.unit_id) {
                                            *mode = Mode::GetUnitOrders{unit_id:self.unit_id, first_move:false};
                                        } else {
                                            *mode = Mode::GetOrders;
                                        }
                                        
                                        Self::clear_buf(ui);
                                        return true;
                                    },
                                    Err(msg) => {
                                        ui.log_message(format!("Error: {}", msg));
                                    }
                                }
                            }
                        } else if c == conf::KEY_SKIP {
                            game.order_unit_skip(self.unit_id).unwrap();
                            // game.give_orders(self.unit_id, Some(Orders::Skip), ui, false).unwrap();
                            *mode = Mode::GetOrders;
                            Self::clear_buf(ui);
                            return true;
                        } else if c == conf::KEY_SENTRY {
                            ui.log_message("Going sentry");
                            // game.give_orders(self.unit_id, Some(Orders::Sentry), ui, false).unwrap();
                            game.order_unit_sentry(self.unit_id).unwrap();
                            *mode = Mode::GetOrders;
                            Self::clear_buf(ui);
                            return true;
                        } else if c == conf::KEY_EXPLORE {
                            let outcome = game.order_unit_explore(self.unit_id).unwrap();
                            if let Some(move_result) = outcome.move_result() {
                                ui.animate_move(game, &move_result);
                            }
                            *mode = Mode::GetOrders;
                            return true;
                        }
                    }
                },
                KeyStatus::Handled(state_disposition) => {
                    match state_disposition {
                        StateDisposition::Quit => return false,
                        StateDisposition::Next => return true,
                        StateDisposition::Stay => {}
                    }
                }
            }
        }
    }
}
