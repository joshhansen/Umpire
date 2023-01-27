use crossterm::event::{KeyCode, KeyEvent};

use common::{
    conf,
    game::{player::PlayerTurnControl, unit::UnitID, PlayerNum},
    util::{Direction, Location, Rect},
};

use crate::ui::{sidebar_rect, TermUI, UI};

use self::{
    examine::ExamineMode, get_orders::GetOrdersMode, get_unit_orders::GetUnitOrdersMode,
    quit::QuitMode, set_production::SetProductionMode, set_productions::SetProductionsMode,
    turn_over::TurnOverMode, turn_resume::TurnResumeMode, turn_start::TurnStartMode,
    victory::VictoryMode,
};

#[derive(Clone, Copy, Debug)]
pub(in crate::ui) enum Mode {
    TurnStart,
    TurnResume,
    TurnOver,
    SetProductions,
    SetProduction {
        city_loc: Location,
    },
    GetOrders,
    GetUnitOrders {
        unit_id: UnitID,
        first_move: bool,
    },
    Quit,
    Examine {
        cursor_viewport_loc: Location,
        most_recently_active_unit_id: Option<UnitID>,
        first: bool,
    },
    Victory {
        victor: PlayerNum,
    },
}

impl Mode {
    /// Return true if the UI should continue after this mode runs, false if it should quit
    pub fn run<U: UI>(
        &mut self,
        game: &mut PlayerTurnControl,
        ui: &mut U,
        prev_mode: &mut Option<Mode>,
    ) -> ModeStatus {
        if let Mode::Victory { .. } = self {
            // nothing
        } else if let Some(victor) = game.victor() {
            *prev_mode = Some(*self);
            *self = Mode::Victory { victor };
            return ModeStatus::Continue;
        }

        let continue_ = match *self {
            Mode::TurnStart => TurnStartMode {}.run(game, ui, self, prev_mode),
            Mode::TurnResume => TurnResumeMode {}.run(game, ui, self, prev_mode),
            Mode::TurnOver => TurnOverMode {}.run(game, ui, self, prev_mode),
            Mode::SetProductions => SetProductionsMode {}.run(game, ui, self, prev_mode),
            Mode::SetProduction { city_loc } => {
                let viewport_rect = ui.viewport_rect();
                let rect = sidebar_rect(viewport_rect, ui.term_dims());
                SetProductionMode {
                    rect,
                    loc: city_loc,
                    unicode: ui.unicode(),
                }
                .run(game, ui, self, prev_mode)
            }
            Mode::GetOrders => GetOrdersMode {}.run(game, ui, self, prev_mode),
            Mode::GetUnitOrders {
                unit_id,
                first_move,
            } => {
                let viewport_rect = ui.viewport_rect();
                let rect = sidebar_rect(viewport_rect, ui.term_dims());
                GetUnitOrdersMode {
                    rect,
                    unit_id,
                    first_move,
                }
                .run(game, ui, self, prev_mode)
            }
            Mode::Quit => QuitMode {}.run(game, ui, self, prev_mode),
            Mode::Examine {
                cursor_viewport_loc,
                most_recently_active_unit_id,
                first,
            } => ExamineMode::new(cursor_viewport_loc, most_recently_active_unit_id, first)
                .run(game, ui, self, prev_mode),
            Mode::Victory { victor } => VictoryMode { victor }.run(game, ui, self, prev_mode),
        };

        *prev_mode = Some(*self);

        continue_
    }
}

/// The outcome of running a mode
///
/// Says whether we keep going in this user's turn, move to the next user's turn, or quit the game
///
/// FIXME: Why is this separate from StateDisposition?
#[derive(PartialEq)]
pub enum ModeStatus {
    /// Continue to the next mode
    Continue,

    /// End the current player's turn
    TurnOver,

    /// Quit the game
    Quit,
}

/// What the handling of a key event means about how state transitions should proceed
///
/// FIXME: Why is this separate from ModeStatus?
#[derive(PartialEq)]
enum StateDisposition {
    Stay,
    Next,
    Quit,
}

enum KeyStatus {
    Handled(StateDisposition),
    Unhandled(KeyEvent),
}

pub(crate) trait IMode {
    /// Return true if the UI should continue after this mode runs, false if it should quit
    fn run<U: UI>(
        &self,
        game: &mut PlayerTurnControl,
        ui: &mut U,
        mode: &mut Mode,
        prev_mode: &Option<Mode>,
    ) -> ModeStatus;

    fn get_key<U: UI>(&self, game: &PlayerTurnControl, ui: &mut U, mode: &mut Mode) -> KeyStatus {
        let key = ui.get_key();
        if let KeyCode::Char(c) = key.code {
            if let Ok(dir) = Direction::try_from_viewport_shift(c) {
                ui.scroll_map_relative(dir);
                ui.draw_map(game);
                return KeyStatus::Handled(StateDisposition::Stay);
            }

            match c {
                conf::KEY_QUIT => {
                    *mode = Mode::Quit;
                    return KeyStatus::Handled(StateDisposition::Quit);
                }
                conf::KEY_EXAMINE => {
                    // println!("Rect: {:?}", ui.viewport_rect());
                    // println!("Center: {:?}", ui.viewport_rect().center());

                    let cursor_viewport_loc = ui
                        .cursor_viewport_loc(mode, game)
                        .unwrap_or(ui.viewport_rect().center());

                    let most_recently_active_unit_id = if let Some(most_recently_active_unit_loc) =
                        ui.cursor_map_loc(mode, game)
                    {
                        game.current_player_toplevel_unit_by_loc(most_recently_active_unit_loc)
                            .map(|unit| unit.id)
                    } else {
                        None
                    };

                    *mode = Mode::Examine {
                        cursor_viewport_loc,
                        most_recently_active_unit_id,
                        first: true,
                    };
                    return KeyStatus::Handled(StateDisposition::Next);
                }
                conf::KEY_VIEWPORT_SIZE_ROTATE => {
                    ui.rotate_viewport_size(game);
                    return KeyStatus::Handled(StateDisposition::Stay);
                }
                _ => {}
            }
        }
        KeyStatus::Unhandled(key)
    }

    fn map_loc_to_viewport_loc(ui: &mut TermUI, map_loc: Location) -> Option<Location> {
        let map = &ui.map_scroller.scrollable;
        map.map_to_viewport_coords(map_loc)
    }
}

trait IVisibleMode: IMode {
    fn rect(&self) -> Rect;

    // fn buf_mut<U:UI>(ui: &mut U) -> &mut RectBuffer;

    fn height(&self) -> u16 {
        self.rect().height
    }

    fn width(&self) -> u16 {
        self.rect().width
    }

    fn clear_buf<U: UI>(ui: &mut U);
}

const COL_WIDTH: usize = 21;

/// Concatenate two strings in a columnar fashion.
///
/// The width of the left column is set by `COL_WIDTH`. s1 is right-padded up to `COL_WIDTH`, then s2 is appended.
fn cols<S1: ToString, S2: ToString>(s1: S1, s2: S2) -> String {
    let s1 = s1.to_string();
    let s2 = s2.to_string();

    let mut c = String::with_capacity(COL_WIDTH + s2.len());
    c.push_str(s1.as_str());

    while c.len() < COL_WIDTH {
        c.push(' ');
    }
    c.push_str(s2.as_str());
    c
}

mod examine;
mod get_orders;
mod get_unit_orders;
mod quit;
mod set_production;
mod set_productions;
mod turn_over;
mod turn_resume;
mod turn_start;
mod victory;
