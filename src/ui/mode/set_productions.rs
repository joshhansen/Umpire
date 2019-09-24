use crate::{
    game::Game,
    log::LogTarget,
    ui::TermUI,
};

use super::{
    IMode,
    Mode,
};

pub struct SetProductionsMode{}
impl IMode for SetProductionsMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {

        if game.production_set_requests().next().is_none() {
            ui.log_message("Productions set.".to_string());
            *mode = Mode::TurnResume;
            return true;
        }

        let city_loc = game.production_set_requests().next().unwrap();

        *mode = Mode::SetProduction{city_loc};
        true
    }
}