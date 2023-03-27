use async_trait::async_trait;

use common::game::player::PlayerTurnControl;

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct TurnResumeMode {}

#[async_trait]
impl IMode for TurnResumeMode {
    async fn run<U: UI + Send>(
        &self,
        game: &mut PlayerTurnControl<'_>,
        _ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        if game.production_set_requests().next().is_some() {
            *mode = Mode::SetProductions;
            return ModeStatus::Continue;
        }

        if game.player_unit_orders_requests().next().is_some() {
            *mode = Mode::GetOrders;
            return ModeStatus::Continue;
        }

        if game.turn_is_done() {
            *mode = Mode::TurnOver;
        }

        ModeStatus::Continue
    }
}
