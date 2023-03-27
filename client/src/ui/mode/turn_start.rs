use async_trait::async_trait;

use common::{
    colors::Colors,
    game::{
        player::PlayerTurnControl, unit::orders::OrdersOutcome, TurnStart, UnitProductionOutcome,
    },
    log::{Message, MessageSource},
};

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct TurnStartMode {}
#[async_trait]
impl IMode for TurnStartMode {
    async fn run<U: UI + Send>(
        &self,
        game: &mut PlayerTurnControl<'_>,
        ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        let turn_start = game.begin_turn().unwrap();
        self.process_turn_start(game, ui, &turn_start);

        ui.draw_current_player(game);

        // A newline for spacing
        ui.log_message("");

        ui.log_message(Message {
            text: format!("Turn {}, player {} go!", game.turn(), game.current_player()),
            mark: Some('_'),
            fg_color: None,
            bg_color: None,
            source: Some(MessageSource::Mode),
        });

        *mode = Mode::TurnResume;

        ModeStatus::Continue
    }
}

impl TurnStartMode {
    async fn animate_orders<U: UI>(
        &self,
        game: &PlayerTurnControl<'_>,
        ui: &mut U,
        orders_outcome: &OrdersOutcome,
    ) {
        let ordered_unit = &orders_outcome.ordered_unit;
        let orders = orders_outcome.orders;

        ui.center_map(ordered_unit.loc);

        ui.log_message(Message::new(
            format!(
                "Unit {} is {}",
                ordered_unit,
                orders.present_progressive_description()
            ),
            Some('@'),
            None,
            None,
            None,
        ));

        ui.draw(game).await;

        if let Some(move_) = orders_outcome.move_() {
            ui.animate_move(game, &move_);
        }
    }

    async fn process_turn_start<U: UI>(
        &self,
        game: &mut PlayerTurnControl<'_>,
        ui: &mut U,
        turn_start: &TurnStart,
    ) {
        for orders_result in &turn_start.orders_results {
            match orders_result {
                Ok(orders_outcome) => self.animate_orders(game, ui, orders_outcome).await,
                Err(e) => ui.log_message(Message {
                    text: format!("{:?}", e),
                    mark: None,
                    fg_color: Some(Colors::Notice),
                    bg_color: None,
                    source: Some(MessageSource::Game),
                }),
            }
        }

        for production_outcome in &turn_start.production_outcomes {
            match production_outcome {
                UnitProductionOutcome::UnitProduced { unit, city } => {
                    ui.log_message(format!(
                        "{} produced {}",
                        city.short_desc(),
                        unit.medium_desc()
                    ));
                }
                UnitProductionOutcome::UnitAlreadyPresent {
                    prior_unit,
                    unit_type_under_production,
                    city,
                } => {
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
                        source: Some(MessageSource::Game),
                    });
                }
            }
        }
    }
}
