use async_trait::async_trait;
use crossterm::event::KeyCode;

use common::{
    colors::Colors,
    game::{player::PlayerTurnControl, PlayerNum},
    log::Message,
};

use crate::ui::UI;

use super::{IMode, KeyStatus, Mode, ModeStatus, StateDisposition};

pub(in crate::ui) struct TurnOverMode {}
impl TurnOverMode {
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

#[async_trait]
impl IMode for TurnOverMode {
    async fn run<U: UI + Send>(
        &self,
        game: &mut PlayerTurnControl,
        ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        let over_for: PlayerNum = game.current_player();

        if ui.confirm_turn_end() {
            ui.log_message(Message {
                text: format!(
                    "Turn over for player {}. Press Enter to continue.",
                    over_for
                ),
                mark: Some('X'),
                fg_color: Some(Colors::Text),
                bg_color: None,
                source: None,
            });

            loop {
                match self.get_key(game, ui, mode) {
                    KeyStatus::Unhandled(key) => {
                        if let KeyCode::Char('\n') = key.code {
                            // If the user has altered productions using examine mode then the turn might not be over anymore
                            // Recheck

                            match game.end_turn() {
                                Ok(_) => {
                                    // *mode = Mode::TurnStart;
                                    return ModeStatus::TurnOver;
                                }
                                Err(_not_over_for) => {
                                    *mode = Mode::TurnResume;
                                    return ModeStatus::Continue;
                                }
                            }
                        }
                    }
                    KeyStatus::Handled(state_disposition) => match state_disposition {
                        StateDisposition::Quit => return ModeStatus::Quit,
                        StateDisposition::Next => return ModeStatus::TurnOver,
                        StateDisposition::Stay => {}
                    },
                }
            }
        } else {
            // We shouldn't be in the TurnOverMode state unless game.turn_is_done() is true
            // so this unwrap should always succeed
            game.end_turn().unwrap();

            // for orders_result in turn_start.orders_results.iter() {
            //     match orders_result {
            //         Ok(orders_outcome) => {
            //             debug_assert!(game.current_player_unit_by_id(orders_outcome.ordered_unit_id).is_some());
            //         },
            //         Err(_e) => {},
            //     }
            // }

            ModeStatus::TurnOver
        }
    }
}

#[cfg(test)]
mod test {
    use common::{
        game::{
            alignment::Alignment,
            map::{MapData, Terrain},
            unit::UnitType,
            Game,
        },
        util::{Dims, Location, Wrap2d},
    };

    use crate::ui::DefaultUI;

    use super::Mode;

    #[test]
    pub fn test_turn_over_mode() {
        //TODO
    }

    #[test]
    pub fn test_order_unit_skip() {
        let mut map = MapData::new(Dims::new(10, 10), |_loc| Terrain::Land);
        let unit_id = map
            .new_unit(
                Location::new(0, 0),
                UnitType::Infantry,
                Alignment::Belligerent { player: 0 },
                "Skipper",
            )
            .unwrap();
        let other_unit_id = map
            .new_unit(
                Location::new(9, 0),
                UnitType::Infantry,
                Alignment::Belligerent { player: 1 },
                "Non-Skipper",
            )
            .unwrap();

        let (mut game, secrets) = Game::new_with_map(map, 2, false, None, Wrap2d::BOTH);

        {
            let (mut ctrl, _turn_start) = game.player_turn_control(secrets[0]).unwrap();
            ctrl.order_unit_skip(unit_id).unwrap();
        }

        let mut prev_mode = Some(Mode::TurnStart);
        let mut mode = Mode::TurnOver;

        {
            let (mut ctrl, _turn_start) = game.player_turn_control(secrets[1]).unwrap();

            ctrl.order_unit_skip(other_unit_id).unwrap();

            mode.run(&mut ctrl, &mut DefaultUI, &mut prev_mode);
        }
    }
}
