use crossterm::event::KeyCode;

use crate::{
    color::Colors,
    game::{
        PlayerNum,
        TurnStart,
        UnitProductionOutcome,
        player::PlayerTurnControl,
        unit::{
            orders::{
                OrdersError,
                OrdersOutcome,
                OrdersResult,
            },
        },
    },
    log::{Message,MessageSource},
    ui::UI,
};

use super::{
    IMode,
    KeyStatus,
    Mode,
    ModeStatus,
    StateDisposition,
};

pub(in crate::ui) struct TurnOverMode {}
impl TurnOverMode {
    fn animate_orders<U:UI>(&self, game: &mut PlayerTurnControl, ui: &mut U, orders_outcome: &OrdersOutcome) {
        // let (id,orders) = match orders_result {
        //     Ok(ref orders_outcome) => (orders_outcome.ordered_unit_id, orders_outcome.orders),
        //     Err(ref err) => match *err {
        //         OrdersError::OrderedUnitDoesNotExist { id, orders } => (id,orders),
        //         OrdersError::MoveError { id, orders, .. } => (id,orders),
        //     }
        // };

        let id = orders_outcome.ordered_unit_id;
        let orders = orders_outcome.orders;

        let unit = game.current_player_unit_by_id(id).unwrap();

        ui.center_map(unit.loc);

        ui.log_message(Message::new(
            format!("Unit {} is {}", unit, orders.present_progressive_description()),
            Some('@'),
            None,
            None,
            None
        ));

        ui.draw(game);

        // match orders_result {
        //     Ok(orders_outcome) => {
                if let Some(move_) = orders_outcome.move_() {
                    ui.animate_move(game, &move_);
                }
        //     },
        //     Err(err) => {
        //         ui.log_message(Message {
        //             text: format!("{}", err),
        //             mark: Some('!'),
        //             fg_color: Some(Colors::Text),
        //             bg_color: Some(Colors::Notice),
        //             source: None,
        //         });
        //     }
        // }
    }

    // fn animate_proposed_orders(&self, game: &mut PlayerTurnControl, ui: &mut TermUI, proposed_orders_result: &ProposedOrdersResult) {
    //     let (id,orders) = match proposed_orders_result {
    //         Ok(ref proposed_orders_outcome) => (proposed_orders_outcome.ordered_unit_id, proposed_orders_outcome.orders),
    //         Err(ref err) => match *err {
    //             OrdersError::OrderedUnitDoesNotExist { id, orders } => (id,orders),
    //             OrdersError::MoveError { id, orders, .. } => (id,orders),
    //         }
    //     };

    //     let unit = game.current_player_unit_by_id(id).unwrap();

    //     ui.map_scroller.scrollable.center_viewport(unit.loc);

    //     ui.log_message(Message::new(
    //         format!("Unit {} is {}", unit, orders.present_progressive_description()),
    //         Some('@'),
    //         None,
    //         None,
    //         None
    //     ));

    //     ui.draw(game);

    //     match proposed_orders_result {
    //         Ok(proposed_orders_outcome) => {
    //             if let Some(ref proposed_move) = proposed_orders_outcome.proposed_move {
    //                 ui.animate_proposed_move(game, proposed_move);
    //                 // proposed_move.take(game);
    //             }
    //         },
    //         Err(err) => {
    //             ui.log_message(Message {
    //                 text: format!("{}", err),
    //                 mark: Some('!'),
    //                 fg_color: Some(Colors::Text),
    //                 bg_color: Some(Colors::Notice),
    //                 source: None,
    //             });
    //         }
    //     }
    // }

    fn process_turn_start<U:UI>(&self, game: &mut PlayerTurnControl, ui: &mut U, turn_start: &TurnStart) {
        // for orders_result in turn_start.orders_results {
        //     self.animate_orders(game, ui, orders_result);
        // }

        for orders_result in &turn_start.orders_results {
            match orders_result {
                Ok(orders_outcome) => self.animate_orders(game, ui, orders_outcome),
                Err(e) => ui.log_message(Message {
                    text: format!("{:?}", e),
                    mark: None,
                    fg_color: Some(Colors::Notice),
                    bg_color: None,
                    source: Some(MessageSource::Game)
                })
            }

            // if orders_result.is_ok() {
            //     self.animate_orders(game, ui, orders_result);
            // } else {
            //     let e = orders_result.as_ref().unwrap_err();
            //     ui.log_message(Message {
            //         text: format!("{:?}", e),
            //         mark: None,
            //         fg_color: Some(Colors::Notice),
            //         bg_color: None,
            //         source: Some(MessageSource::Game)
            //     });
            // }
        }

        for production_outcome in &turn_start.production_outcomes {
            match production_outcome {
                UnitProductionOutcome::UnitProduced { unit, city } => {
                    ui.log_message(format!("{} produced {}", city.short_desc(), unit.medium_desc()));
                },
                UnitProductionOutcome::UnitAlreadyPresent { prior_unit, unit_type_under_production, city} => {
                    ui.log_message(Message {
                        text: format!(
                            "{} would have produced {} but {} was already garrisoned",
                            city.short_desc(),
                            unit_type_under_production,
                            prior_unit
                        ),
                        mark: None,
                        fg_color: Some(Colors::Notice),
                        bg_color: None,
                        source: Some(MessageSource::Game)
                    });
                },
            }
        }
    }

    // fn process_proposed_turn_start(&self, game: &mut PlayerTurnControl, ui: &mut TermUI, turn_start: &ProposedTurnStart) {
    //     // for orders_result in turn_start.orders_results {
    //     //     self.animate_orders(game, ui, orders_result);
    //     // }

    //     for proposed_orders_result in &turn_start.proposed_orders_results {
    //         self.animate_proposed_orders(game, ui, proposed_orders_result);
    //     }

    //     for production_outcome in &turn_start.production_outcomes {
    //         match production_outcome {
    //             UnitProductionOutcome::UnitProduced { unit, city } => {
    //                 ui.log_message(format!("{} produced {}", city.short_desc(), unit.medium_desc()));
    //             },
    //             UnitProductionOutcome::UnitAlreadyPresent { prior_unit, unit_type_under_production, city} => {
    //                 ui.log_message(Message {
    //                     text: format!(
    //                         "{} would have produced {} but {} was already garrisoned",
    //                         city.short_desc(),
    //                         unit_type_under_production,
    //                         prior_unit
    //                     ),
    //                     mark: None,
    //                     fg_color: Some(Colors::Notice),
    //                     bg_color: None,
    //                     source: Some(MessageSource::Game)
    //                 });
    //             },
    //         }
    //     }
    // }
}

impl IMode for TurnOverMode {
    fn run<U:UI>(&self, game: &mut PlayerTurnControl, ui: &mut U, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
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
                        if let KeyCode::Char('\n') = key.code {

                            // If the user has altered productions using examine mode then the turn might not be over anymore
                            // Recheck

                            match game.propose_end_turn().1 {
                                Ok(turn_start) => {
                                    self.process_turn_start(game, ui, &turn_start);
                                    // *mode = Mode::TurnStart;
                                    return ModeStatus::TurnOver;
                                },
                                Err(_not_over_for) => {
                                    *mode = Mode::TurnResume;
                                    return ModeStatus::Continue;
                                }
                            }
                        }
                    },
                    KeyStatus::Handled(state_disposition) => {
                        match state_disposition {
                            StateDisposition::Quit => return ModeStatus::Quit,
                            StateDisposition::Next => return ModeStatus::TurnOver,
                            StateDisposition::Stay => {}
                        }
                    }
                }
            }
        } else {
            // We shouldn't be in the TurnOverMode state unless game.turn_is_done() is true
            // so this unwrap should always succeed
            let (_game2, turn_start) = game.propose_end_turn();
            let turn_start = turn_start.unwrap();

            // for orders_result in turn_start.orders_results.iter() {
            //     match orders_result {
            //         Ok(orders_outcome) => {
            //             debug_assert!(game.current_player_unit_by_id(orders_outcome.ordered_unit_id).is_some());
            //         },
            //         Err(_e) => {},
            //     }
            // }

            self.process_turn_start(game, ui, &turn_start);

            return ModeStatus::TurnOver;
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        sync::{
            Arc,
            RwLock,
        },
    };

    use crate::{
        game::{
            Alignment,
            Game,
            map::{
                MapData,
                Terrain,
            },
            unit::{
                UnitType,
            },
        },
        name::{
            IntNamer,
        },
        ui::DefaultUI,
        util::{Dims,Direction,Location,Wrap2d},
    };

    use super::Mode;

    #[test]
    pub fn test_turn_over_mode() {
    }

    #[test]
    pub fn test_order_unit_skip() {
        let mut map = MapData::new(Dims::new(10, 10), |_loc| Terrain::Land);
        let unit_id = map.new_unit(Location::new(0, 0), UnitType::Infantry, Alignment::Belligerent{player:0}, "Skipper").unwrap();
        let other_unit_id = map.new_unit(Location::new(9, 0), UnitType::Infantry,
            Alignment::Belligerent{player:1}, "Non-Skipper").unwrap();


        let unit_namer = IntNamer::new("unit");
        let mut game = Game::new_with_map(map, 2, false, Arc::new(RwLock::new(unit_namer)), Wrap2d::BOTH);

        {
            let mut ctrl = game.player_turn_control(0);
            ctrl.order_unit_skip(unit_id).unwrap();
        }

        let mut prev_mode = Some(Mode::TurnStart);
        let mut mode = Mode::TurnOver;

        {
            let mut ctrl = game.player_turn_control(1);

            ctrl.order_unit_skip(other_unit_id).unwrap();
        
            mode.run(&mut ctrl, &mut DefaultUI, &mut prev_mode);
        }

        // game.end_turn().unwrap();

        // assert_eq!(game.unit_orders_requests().next(), Some(unit_id));

        // game.current_player_unit_by_id(unit_id).unwrap();

    }
}