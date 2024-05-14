use common::game::player::PlayerTurn;

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct TurnResumeMode;

impl IMode for TurnResumeMode {
    async fn run<U: UI + Send>(
        &self,
        game: &mut PlayerTurn<'_>,
        _ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        if !game.player_production_set_requests().await.is_empty() {
            *mode = Mode::SetProductions;
            return ModeStatus::Continue;
        }

        if !game.player_unit_orders_requests().await.is_empty() {
            *mode = Mode::GetOrders;
            return ModeStatus::Continue;
        }

        if game.current_turn_is_done().await {
            *mode = Mode::TurnOver;
        }

        ModeStatus::Continue
    }
}
