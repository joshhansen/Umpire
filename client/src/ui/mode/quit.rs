use common::game::player::PlayerTurn;

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct QuitMode;

impl IMode for QuitMode {
    async fn run<U: UI + Send>(
        &self,
        _game: &mut PlayerTurn<'_>,
        _ui: &mut U,
        _mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        ModeStatus::Quit
    }
}
