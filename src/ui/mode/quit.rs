use crate::{game::player::PlayerTurnControl, ui::UI};

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct QuitMode;
impl IMode for QuitMode {
    fn run<U: UI>(
        &self,
        _game: &mut PlayerTurnControl,
        _ui: &mut U,
        _mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        ModeStatus::Quit
    }
}
