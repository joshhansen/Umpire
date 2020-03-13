use crate::{
    game::player::PlayerTurnControl,
    ui::TermUI,
};

use super::{
    IMode,
    Mode,
    ModeStatus,
};

pub(in crate::ui) struct QuitMode;
impl IMode for QuitMode {
    fn run(&self, _game: &mut PlayerTurnControl, _ui: &mut TermUI, _mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
        ModeStatus::Quit
    }
}