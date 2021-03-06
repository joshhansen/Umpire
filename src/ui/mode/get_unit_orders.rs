use std::{
    convert::TryFrom,
};

use crossterm::event::KeyCode;

use crate::{
    conf::{self, key_desc},
    game::{
        Proposed,
        player::{
            PlayerTurnControl,
        },
        unit::{
            UnitID, orders::OrdersResult,
        },
    },
    ui::{
        audio::Sounds,
        UI,
    },
    util::{Direction,Rect},
};

use super::{
    IMode,
    IVisibleMode,
    KeyStatus,
    Mode,
    ModeStatus,
    StateDisposition,
    cols,
};

pub(in crate::ui) struct GetUnitOrdersMode{
    pub rect: Rect,
    pub unit_id: UnitID,
    pub first_move: bool
}
impl IVisibleMode for GetUnitOrdersMode {
    fn clear_buf<U:UI>(ui: &mut U) {
        ui.clear_sidebar();
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    // fn buf_mut<U:UI>(ui: &mut U) -> &mut RectBuffer {
    //     ui.sidebar_buf_mut()
    // }
}
impl GetUnitOrdersMode {
    fn write_buf<U:UI>(&self, game: &PlayerTurnControl, ui: &mut U) {
        let unit = game.current_player_unit_by_id(self.unit_id).unwrap();

        ui.set_sidebar_row(0, format!("Get Orders for {}", unit));
        ui.set_sidebar_row(2, format!("Move: ↖ ↗          {} {}", conf::KEY_UP_LEFT, conf::KEY_UP_RIGHT));
        ui.set_sidebar_row(3, format!("       ← ↓ ↑ →      {} {} {} {}", conf::KEY_LEFT, conf::KEY_DOWN, conf::KEY_UP, conf::KEY_RIGHT));
        ui.set_sidebar_row(4, format!("      ↙ ↘          {} {}", conf::KEY_DOWN_LEFT, conf::KEY_DOWN_RIGHT));
        ui.set_sidebar_row(6, cols("Examine:", conf::KEY_EXAMINE));
        ui.set_sidebar_row(8, cols("Explore:", conf::KEY_EXPLORE));
        ui.set_sidebar_row(10, cols("Skip:", key_desc(conf::KEY_SKIP)));
        ui.set_sidebar_row(12, cols("Sentry:", conf::KEY_SENTRY));
        ui.set_sidebar_row(14, cols("Disband:", conf::KEY_DISBAND));
        ui.set_sidebar_row(16, cols("Quit:", conf::KEY_QUIT));
    }
}
impl IMode for GetUnitOrdersMode {
    fn run<U:UI>(&self, game: &mut PlayerTurnControl, ui: &mut U, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
        let unit_loc = {
            let unit = {
                let unit = game.current_player_unit_by_id(self.unit_id).unwrap();
                ui.log_message(format!("Requesting orders for {} at {}", unit.medium_desc(), unit.loc));
                // (unit.loc,unit.type_, unit.sym(ui.unicode))
                unit
            };

            if self.first_move {
                ui.play_sound(Sounds::Unit(unit.type_));
                ui.center_map(unit.loc);
            }

            self.write_buf(game, ui);
            ui.draw_no_flush(game);

            let viewport_loc = ui.map_to_viewport_coords(unit.loc).unwrap();
            ui.draw_map_tile_and_flush(game, viewport_loc, false, true, 
                None, Some(Some(unit)), None, None);

            unit.loc
        };

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {

                    if let KeyCode::Char(c) = key.code {
                        if let Ok(dir) = Direction::try_from(c) {
                            if let Some(dest) = unit_loc.shift_wrapped(dir, game.dims(), game.wrapping()) {

                                let proposed_move = game.propose_move_unit_by_id(self.unit_id, dest);

                                match proposed_move.delta {
                                    Ok(ref proposed_move_result) => {
                                        ui.animate_move(game, &proposed_move_result);

                                        let move_ = game.apply_proposal(proposed_move).unwrap();

                                        if let Some(conquered_city) = move_.conquered_city() {
                                            *mode = Mode::SetProduction {city_loc: conquered_city.loc};
                                        } else if game.unit_orders_requests().any(|unit_id| unit_id==self.unit_id) {
                                            *mode = Mode::GetUnitOrders{unit_id:self.unit_id, first_move:false};
                                        } else {
                                            *mode = Mode::GetOrders;
                                        }
                                        
                                        Self::clear_buf(ui);
                                        return ModeStatus::Continue;

                                    },
                                    Err(msg) => {
                                        ui.log_message(format!("Error: {}", msg));
                                    }
                                }
                            }
                        } else if c == conf::KEY_SKIP {
                            game.order_unit_skip(self.unit_id).unwrap();
                            *mode = Mode::GetOrders;
                            Self::clear_buf(ui);
                            return ModeStatus::Continue;
                        } else if c == conf::KEY_SENTRY {
                            ui.log_message("Going sentry");
                            game.order_unit_sentry(self.unit_id).unwrap();
                            *mode = Mode::GetOrders;
                            Self::clear_buf(ui);
                            return ModeStatus::Continue;
                        } else if c == conf::KEY_DISBAND {
                            let unit = game.disband_unit_by_id(self.unit_id).unwrap();
                            ui.log_message(format!("Disbanded unit {}", unit.short_desc()));
                            *mode = Mode::GetOrders;
                            Self::clear_buf(ui);
                            return ModeStatus::Continue;
                        } else if c == conf::KEY_EXPLORE {
                            let proposed_orders_result: Proposed<OrdersResult> = game.propose_order_unit_explore(self.unit_id);
                            let proposed_orders_outcome = proposed_orders_result.delta.as_ref().unwrap();
                            if let Some(ref proposed_move) = proposed_orders_outcome.move_ {
                                ui.animate_move(game, &proposed_move);
                                // proposed_move.take(game);
                            }

                            game.apply_proposal(proposed_orders_result).unwrap();

                            *mode = Mode::GetOrders;
                            return ModeStatus::Continue;
                        }
                    }
                },
                KeyStatus::Handled(state_disposition) => {
                    match state_disposition {
                        StateDisposition::Quit => return ModeStatus::Quit,
                        StateDisposition::Next => return ModeStatus::Continue,
                        StateDisposition::Stay => {}
                    }
                }
            }
        }
    }
}
