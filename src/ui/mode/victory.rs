use crate::{
    color::Colors,
    game::{
        PlayerNum,
        player::PlayerTurnControl,
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
    Mode,
    ModeStatus,
};

pub(in crate::ui) struct VictoryMode {
    pub(in crate::ui) victor: PlayerNum,
}
impl IMode for VictoryMode {
    fn run(&self, game: &mut PlayerTurnControl, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
        ui.log_message(Message {
            text: format!("Player {} has vanquished all foes. Press any key to quit.", self.victor),
            mark: Some('!'),
            fg_color: Some(Colors::Text),
            bg_color: None,
            source: None
        });
        ui.log.draw(game, &mut ui.stdout, &ui.palette);// this will flush

        // Wait for a keypress
        self.get_key(game, ui, mode);
        
        ModeStatus::Quit 
    }
}