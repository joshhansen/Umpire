use crate::{
    color::Colors,
    game::{
        Game,
        PlayerNum,
    },
    log::{
        LogTarget,
        Message,
    },
    ui::{
        Draw,
        TermUI,
    },
};

use super::{
    IMode,
    KeyStatus,
    Mode,
    StateDisposition,
};

pub(in crate::ui) struct VictoryMode {
    pub(in crate::ui) victor: PlayerNum,
}
impl IMode for VictoryMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {

        ui.log_message(Message {
            text: format!("Player {} has vanquished all foes. Press any key to quit.", self.victor),
            mark: Some('!'),
            fg_color: Some(Colors::Text),
            bg_color: None,
            source: None
        });
        ui.log.draw(game, &mut ui.stdout, &ui.palette);// this will flush

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(_key_event) => {
                    return false;
                },
                KeyStatus::Handled(state_disposition) => {
                    match state_disposition {
                        StateDisposition::Quit => return false,
                        StateDisposition::Next => return true,
                        StateDisposition::Stay => {}
                    }
                }
            }
        }
    }
}