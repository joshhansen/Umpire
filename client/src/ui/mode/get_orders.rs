use async_trait::async_trait;

use common::game::player::PlayerTurn;

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct GetOrdersMode {}

#[async_trait]
impl IMode for GetOrdersMode {
    async fn run<U: UI + Send>(
        &self,
        game: &mut PlayerTurn<'_>,
        _ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        if let Some(unit_id) = game
            .player_unit_orders_requests()
            .await
            .iter()
            .cloned()
            .next()
        {
            *mode = Mode::GetUnitOrders {
                unit_id,
                first_move: true,
            };
        } else {
            *mode = Mode::TurnResume;
        }

        ModeStatus::Continue
    }
}
