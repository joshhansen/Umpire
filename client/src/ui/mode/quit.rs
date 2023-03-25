use async_trait::async_trait;

use common::game::player::PlayerTurnControl;

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct QuitMode;

#[async_trait]
impl IMode for QuitMode {
    async fn run<U: UI + Send>(
        &self,
        _game: &mut PlayerTurnControl,
        _ui: &mut U,
        _mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        ModeStatus::Quit
    }
}
