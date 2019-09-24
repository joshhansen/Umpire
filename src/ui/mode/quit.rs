use crate::{
    game::Game,
    ui::TermUI,
};

use super::{
    IMode,
    Mode,
};

pub struct QuitMode;
impl IMode for QuitMode {
    fn run(&self, _game: &mut Game, _ui: &mut TermUI, _mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        false
    }
}