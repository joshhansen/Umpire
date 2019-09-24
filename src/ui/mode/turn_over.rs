use crossterm::{
    KeyEvent,
};

use crate::{
    color::Colors,
    game::{
        Game,
        PlayerNum,
    },
    log::{LogTarget,Message},
    ui::TermUI,
};

use super::{
    IMode,
    KeyStatus,
    Mode,
    StateDisposition,
};

pub struct TurnOverMode {}
impl IMode for TurnOverMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {

        let over_for: PlayerNum = game.current_player();

        if ui.confirm_turn_end() {
            ui.log_message(Message {
                text: format!("Turn over for player {}. Press Enter to continue.", over_for),
                mark: Some('X'),
                fg_color: Some(Colors::Text),
                bg_color: None,
                source: None
            });

            loop {
                match self.get_key(game, ui, mode) {
                    KeyStatus::Unhandled(key) => {
                        if let KeyEvent::Char('\n') = key {

                            // If the user has altered productions using examine mode then the turn might not be over anymore
                            // Recheck

                            match game.end_turn(ui) {
                                Ok(_over_for) => {
                                    *mode = Mode::TurnStart;
                                },
                                Err(_not_over_for) => {
                                    *mode = Mode::TurnResume;
                                }
                            }

                            return true;
                        }
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
        } else {
            // We shouldn't be in this state unless game.turn_is_done() is true
            // so this unwrap should always succeed
            game.end_turn(ui).unwrap();
            *mode = Mode::TurnStart;
            true
        }
    }
}