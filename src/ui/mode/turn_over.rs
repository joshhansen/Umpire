use crossterm::{
    KeyEvent,
};

use crate::{
    color::Colors,
    game::{
        Game,
        PlayerNum,
        TurnStart,
        UnitProductionOutcome,
        unit::{
            UnitID,
            Unit,
            orders::{
                OrdersError,
                OrdersResult,
            },
        },
    },
    log::{LogTarget,Message},
    ui::{
        MoveAnimator,
        TermUI,
        buf::RectBuffer,
        mode::IVisibleMode,
    },
    util::Rect,
};

use super::{
    IMode,
    KeyStatus,
    Mode,
    StateDisposition,
};

pub(in crate::ui) struct TurnOverMode {
    // rect: Rect,
}

// impl IVisibleMode for TurnOverMode {
//     fn rect(&self) -> Rect {
//         self.rect
//     }

//     fn buf_mut(ui: &mut TermUI) -> &mut RectBuffer {
//         ui.sidebar_buf_mut()
//     }
// }
impl TurnOverMode {
    // fn write_buf(&self, game: &Game, ui: &mut TermUI, unit: &Unit) {
    //     // let unit = game.unit_by_id(self.unit_id).unwrap();

    //     let buf = ui.sidebar_buf_mut();
    //     buf.set_row(0, format!("Unit {} is {}", unit, unit.orders.unwrap().present_progressive_description()));
    // }

    fn animate_orders(&self, game: &mut Game, ui: &mut TermUI, orders_result: OrdersResult) {
        let (id,orders) = match orders_result {
            Ok(ref orders_outcome) => (orders_outcome.ordered_unit_id, orders_outcome.orders),
            Err(ref err) => match *err {
                OrdersError::OrderedUnitDoesNotExist { id, orders } => (id,orders),
                OrdersError::MoveError { id, orders, .. } => (id,orders),
            }
        };

        let unit = game.unit_by_id(id).unwrap();

        ui.map_scroller.scrollable.center_viewport(unit.loc);

        // self.write_buf(game, ui, unit);

        ui.log_message(Message::new(
            format!("Unit {} is {}", unit, orders.present_progressive_description()),
            Some('@'),
            None,
            None,
            None
        ));

        ui.draw(game);

        match orders_result {
            Ok(orders_outcome) => {
                if let Some(move_result) = orders_outcome.move_result() {
                    ui.animate_move(game, &move_result);
                }
            },
            Err(err) => {
                ui.log_message(Message {
                    text: format!("{}", err),
                    mark: Some('!'),
                    fg_color: Some(Colors::Text),
                    bg_color: Some(Colors::Notice),
                    source: None,
                });
            }
        }
    }

    fn process_turn_start(&self, game: &mut Game, ui: &mut TermUI, turn_start: TurnStart) {
        for orders_result in turn_start.carried_out_orders {
            self.animate_orders(game, ui, orders_result);
        }
        for production_outcome in turn_start.production_outcomes {
            match production_outcome {
                UnitProductionOutcome::UnitProduced { id, producing_city_id } => {
                    //FIXME Improve this log message
                    ui.log_message(format!("City with ID {:?} produced unit with ID {:?}", producing_city_id, id));
                },
                UnitProductionOutcome::UnitAlreadyPresent { producing_city_id, unit_under_production } => {
                    //FIXME Improve this log message
                    ui.log_message(format!("City with ID {:?} could not produce unit of type {:?} because a unit is already present",
                        producing_city_id, unit_under_production));
                },
            }
        }
    }
}

impl IMode for TurnOverMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {

        let over_for: PlayerNum = game.current_player();

        if ui.confirm_turn_end() {
            ui.log_message(Message {
                text: format!("Turn over for player {}. Press Enter to continue.", over_for),
                mark: Some('X'),
                fg_color: Some(Colors::Text),
                bg_color: None,
                source: None
            });

            loop {
                match self.get_key(game, ui, mode) {
                    KeyStatus::Unhandled(key) => {
                        if let KeyEvent::Char('\n') = key {

                            // If the user has altered productions using examine mode then the turn might not be over anymore
                            // Recheck

                            match game.end_turn() {
                                Ok(turn_start) => {

                                    self.process_turn_start(game, ui, turn_start);

                                    *mode = Mode::TurnStart;
                                },
                                Err(_not_over_for) => {
                                    *mode = Mode::TurnResume;
                                }
                            }

                            return true;
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
        } else {
            // We shouldn't be in the TurnOverMode state unless game.turn_is_done() is true
            // so this unwrap should always succeed
            let turn_start = game.end_turn().unwrap();
            self.process_turn_start(game, ui, turn_start);
            *mode = Mode::TurnStart;
            true
        }
    }
}