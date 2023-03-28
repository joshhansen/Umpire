use async_trait::async_trait;

use common::game::player::PlayerTurnControl;

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct GetOrdersMode {}

#[async_trait]
impl IMode for GetOrdersMode {
    async fn run<U: UI + Send>(
        &self,
        game: &mut PlayerTurnControl<'_>,
        _ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        if let Some(unit_id) = game.player_unit_orders_requests().await.next() {
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
