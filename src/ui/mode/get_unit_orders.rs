use std::convert::TryFrom;

use crossterm::event::KeyCode;

use crate::{
    conf::{self, key_desc},
    game::{
        Game,
        ProposedAction,
        unit::{
            UnitID,
            orders::ProposedSetAndFollowOrders,
        },
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
        let unit = game.current_player_unit_by_id(self.unit_id).unwrap();

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
        let unit_loc = {
            let unit = {
                let unit = game.current_player_unit_by_id(self.unit_id).unwrap();
                ui.log_message(format!("Requesting orders for {} at {}", unit.medium_desc(), unit.loc));
                // (unit.loc,unit.type_, unit.sym(ui.unicode))
                unit
            };

            if self.first_move {
                ui.play_sound(Sounds::Unit(unit.type_));
                ui.map_scroller.scrollable.center_viewport(unit.loc);
            }

            self.write_buf(game, ui);
            ui.draw_no_flush(game);

            let viewport_loc = ui.map_scroller.scrollable.map_to_viewport_coords(unit.loc).unwrap();
            ui.map_scroller.scrollable.draw_tile_and_flush(game, &mut ui.stdout, viewport_loc, false, true, None, Some(Some(unit)), None);

            unit.loc
        };

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {

                    if let KeyCode::Char(c) = key.code {
                        if let Ok(dir) = Direction::try_from(c) {
                            if let Some(dest) = unit_loc.shift_wrapped(dir, game.dims(), game.wrapping()) {

                                match game.propose_move_unit_by_id(self.unit_id, dest) {
                                    Ok(proposed_move) => {
                                        ui.animate_proposed_move(game, &proposed_move);

                                        let move_ = proposed_move.take(game);

                                        if let Some(conquered_city) = move_.conquered_city() {
                                            *mode = Mode::SetProduction {city_loc: conquered_city.loc};
                                        } else if game.unit_orders_requests().any(|unit_id| unit_id==self.unit_id) {
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
                            *mode = Mode::GetOrders;
                            Self::clear_buf(ui);
                            return true;
                        } else if c == conf::KEY_SENTRY {
                            ui.log_message("Going sentry");
                            game.order_unit_sentry(self.unit_id).unwrap();
                            *mode = Mode::GetOrders;
                            Self::clear_buf(ui);
                            return true;
                        } else if c == conf::KEY_EXPLORE {
                            let proposed_outcome: ProposedSetAndFollowOrders = game.propose_order_unit_explore(self.unit_id);
                            let proposed_orders_outcome = proposed_outcome.proposed_orders_result.as_ref().unwrap();
                            if let Some(ref proposed_move) = proposed_orders_outcome.proposed_move {
                                ui.animate_proposed_move(game, &proposed_move);
                                // proposed_move.take(game);
                            }

                            proposed_outcome.take(game).unwrap();

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
