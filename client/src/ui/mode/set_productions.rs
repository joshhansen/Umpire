use async_trait::async_trait;

use common::game::player::PlayerTurn;

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct SetProductionsMode {}

#[async_trait]
impl IMode for SetProductionsMode {
    async fn run<U: UI + Send>(
        &self,
        game: &mut PlayerTurn<'_>,
        ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        if game.player_production_set_requests().await.is_empty() {
            ui.log_message("Productions set.".to_string());
            *mode = Mode::TurnResume;
            return ModeStatus::Continue;
        }

        let city_loc = game
            .player_production_set_requests()
            .await
            .iter()
            .cloned()
            .next()
            .unwrap();

        *mode = Mode::SetProduction { city_loc };
        ModeStatus::Continue
    }
}
