use async_trait::async_trait;

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

#[async_trait]
impl IMode for VictoryMode {
    async fn run<U: UI + Send>(
        &self,
        ctrl: &mut PlayerTurnControl<'_>,
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
