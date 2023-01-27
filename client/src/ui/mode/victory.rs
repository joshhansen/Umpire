use common::{
    colors::Colors,
    game::{player::PlayerTurnControl, PlayerNum},
    log::Message,
};

use crate::ui::UI;

use super::{IMode, Mode, ModeStatus};

pub(in crate::ui) struct VictoryMode {
    pub(in crate::ui) victor: PlayerNum,
}
impl IMode for VictoryMode {
    fn run<U: UI>(
        &self,
        ctrl: &mut PlayerTurnControl,
        ui: &mut U,
        mode: &mut Mode,
        _prev_mode: &Option<Mode>,
    ) -> ModeStatus {
        ui.log_message(Message {
            text: format!(
                "Player {} has vanquished all foes. Press any key to quit.",
                self.victor
            ),
            mark: Some('!'),
            fg_color: Some(Colors::Text),
            bg_color: None,
            source: None,
        });
        ui.draw_log(ctrl); // this will flush

        // Wait for a keypress
        self.get_key(ctrl, ui, mode);

        ModeStatus::Quit
    }
}
