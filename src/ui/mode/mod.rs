use crossterm::{
    KeyEvent,
};

use crate::{
    conf,
    game::{
        Game,
        map::UnitID,
    },
    ui::{
        buf::RectBuffer,
        Draw,
        TermUI,
        sidebar_rect,
        scroll::ScrollableComponent,
    },
    util::{Direction,Location,Rect},
};

use self::{
    carry_out_orders::CarryOutOrdersMode,
    carry_out_unit_orders::CarryOutUnitOrdersMode,
    examine::ExamineMode,
    get_orders::GetOrdersMode,
    get_unit_orders::GetUnitOrdersMode,
    quit::QuitMode,
    set_production::SetProductionMode,
    set_productions::SetProductionsMode,
    turn_over::TurnOverMode,
    turn_start::TurnStartMode,
    turn_resume::TurnResumeMode,
};

#[derive(Clone,Copy,Debug)]
pub enum Mode {
    TurnStart,
    TurnResume,
    TurnOver,
    SetProductions,
    SetProduction{city_loc:Location},
    GetOrders,
    GetUnitOrders{unit_id:UnitID, first_move:bool},
    CarryOutOrders,
    CarryOutUnitOrders{unit_id:UnitID},
    Quit,
    Examine{
        cursor_viewport_loc:Location,
        first: bool,
        most_recently_active_unit_id: Option<UnitID>
    }
}

impl Mode {
    /// Return true if the UI should continue after this mode runs, false if it should quit
    pub fn run(&mut self, game: &mut Game, ui: &mut TermUI, prev_mode: &mut Option<Mode>) -> bool {
        let continue_ = match *self {
            Mode::TurnStart =>          TurnStartMode{}.run(game, ui, self, prev_mode),
            Mode::TurnResume =>         TurnResumeMode{}.run(game, ui, self, prev_mode),
            Mode::TurnOver   =>         TurnOverMode{}.run(game, ui, self, prev_mode),
            Mode::SetProductions =>     SetProductionsMode{}.run(game, ui, self, prev_mode),
            Mode::SetProduction{city_loc} => {
                let viewport_rect = ui.viewport_rect();
                let rect = sidebar_rect(viewport_rect, ui.term_dims);
                SetProductionMode{rect, loc:city_loc, unicode: ui.unicode}.run(game, ui, self, prev_mode)
            },
            Mode::GetOrders =>          GetOrdersMode{}.run(game, ui, self, prev_mode),
            Mode::GetUnitOrders{unit_id,first_move} =>      {
                let viewport_rect = ui.viewport_rect();
                let rect = sidebar_rect(viewport_rect, ui.term_dims);
                GetUnitOrdersMode{rect, unit_id, first_move}.run(game, ui, self, prev_mode)
            },
            Mode::CarryOutOrders =>     CarryOutOrdersMode{}.run(game, ui, self, prev_mode),
            Mode::CarryOutUnitOrders{unit_id} => {
                let viewport_rect = ui.viewport_rect();
                let rect = sidebar_rect(viewport_rect, ui.term_dims);
                CarryOutUnitOrdersMode{rect, unit_id}.run(game, ui, self, prev_mode)
            },
            Mode::Quit =>               QuitMode{}.run(game, ui, self, prev_mode),
            Mode::Examine{cursor_viewport_loc, first, most_recently_active_unit_id} =>
                ExamineMode{cursor_viewport_loc, first, most_recently_active_unit_id}.run(game, ui, self, prev_mode)
        };

        *prev_mode = Some(*self);

        continue_
    }
}

#[derive(PartialEq)]
enum StateDisposition {
    Stay,
    Next,
    Quit
}

enum KeyStatus {
    Handled(StateDisposition),
    Unhandled(KeyEvent)
}

trait IMode {
    /// Return true if the UI should continue after this mode runs, false if it should quit
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, prev_mode: &Option<Mode>) -> bool;

    fn get_key(&self, game: &Game, ui: &mut TermUI, mode: &mut Mode) -> KeyStatus {
        let key = ui.get_key();
        if let KeyEvent::Char(c) = key {
            if let Ok(dir) = Direction::try_from_viewport_shift(c) {
                ui.map_scroller.scrollable.scroll_relative(dir.vec2d());
                ui.map_scroller.draw(game, &mut ui.stdout, &ui.palette);
                return KeyStatus::Handled(StateDisposition::Stay);
            }

            match c {
                conf::KEY_QUIT => {
                    *mode = Mode::Quit;
                    return KeyStatus::Handled(StateDisposition::Quit);
                },
                conf::KEY_EXAMINE => {
                    // println!("Rect: {:?}", ui.viewport_rect());
                    // println!("Center: {:?}", ui.viewport_rect().center());

                    let cursor_viewport_loc = ui.cursor_viewport_loc(mode, game).unwrap_or(
                        ui.viewport_rect().center()
                    );

                    let most_recently_active_unit_id =
                        if let Some(most_recently_active_unit_loc) = ui.cursor_map_loc(mode, game) {
                            game.unit_by_loc(most_recently_active_unit_loc).map(|unit| unit.id)
                        } else {
                            None
                        }
                    ;

                    *mode = Mode::Examine{
                        cursor_viewport_loc,
                        first: true,
                        most_recently_active_unit_id
                    };
                    return KeyStatus::Handled(StateDisposition::Next);
                },
                conf::KEY_VIEWPORT_SIZE_ROTATE => {
                    ui.rotate_viewport_size(game);
                    return KeyStatus::Handled(StateDisposition::Stay);
                },
                _ => {}
            }
        }
        KeyStatus::Unhandled(key)
    }

    fn map_loc_to_viewport_loc(ui: &mut TermUI, map_loc: Location) -> Option<Location> {
        let viewport_dims = ui.map_scroller.viewport_dims();
        let map = &ui.map_scroller.scrollable;
        map.map_to_viewport_coords(map_loc, viewport_dims)
    }
}

trait IVisibleMode: IMode {
    fn rect(&self) -> Rect;

    fn buf_mut(ui: &mut TermUI) -> &mut RectBuffer;

    fn height(&self) -> u16 {
        self.rect().height
    }

    fn width(&self) -> u16 {
        self.rect().width
    }

    fn clear_buf(ui: &mut TermUI) {
        Self::buf_mut(ui).clear();
    }
}

const COL_WIDTH: usize = 21;

/// Concatenate two strings in a columnar fashion.
/// 
/// The width of the left column is set by `COL_WIDTH`. s1 is right-padded up to `COL_WIDTH`, then s2 is appended.
fn cols<S1:ToString,S2:ToString>(s1: S1, s2: S2) -> String {
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

mod carry_out_orders;
mod carry_out_unit_orders;
mod examine;
mod get_orders;
mod get_unit_orders;
mod quit;
mod set_production;
mod set_productions;
mod turn_over;
mod turn_start;
mod turn_resume;