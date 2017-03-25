use std::convert::TryFrom;
use std::io::{Write, stdin, StdoutLock};

use termion::cursor::Goto;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::RawTerminal;

use conf;
use game::Game;
use ui::{Redraw,UI,V_SCROLLBAR_WIDTH,HEADER_HEIGHT};
use unit::{Named,UnitType};
use util::{Direction,Location,Rect};

fn get_key() -> Key {
    let stdin = stdin();
    let c = stdin.keys().next().unwrap().unwrap();
    c
}

#[derive(Clone,Copy,Debug)]
pub enum Mode {
    TurnStart,
    SetProductions,
    SetProduction{loc:Location},
    MoveUnits,
    MoveUnit{loc:Location},
    Quit,
    Examine{cursor_viewport_loc:Location}
}

impl Mode {
    pub fn run<'a>(&mut self, game: &mut Game, ui: &mut UI<'a>) -> bool {
        match *self {
            Mode::TurnStart =>          TurnStartMode{}.run(game, ui, self),
            Mode::SetProductions =>     SetProductionsMode{}.run(game, ui, self),
            Mode::SetProduction{loc} => {
                let viewport_rect = ui.viewport_rect();
                let rect = Rect {
                    left: viewport_rect.width + V_SCROLLBAR_WIDTH + 1,
                    top: HEADER_HEIGHT + 1,
                    width: ui.term_dims.width - viewport_rect.width - 2,
                    height: ui.term_dims.height - HEADER_HEIGHT
                };
                SetProductionMode{loc:loc, rect:rect}.run(game, ui, self)
            },
            Mode::MoveUnits =>          MoveUnitsMode{}.run(game, ui, self),
            Mode::MoveUnit{loc} =>      MoveUnitMode{loc:loc}.run(game, ui, self),
            Mode::Quit =>               QuitMode{}.run(game, ui, self),
            Mode::Examine{cursor_viewport_loc}       => ExamineMode{cursor_viewport_loc:cursor_viewport_loc}.run(game, ui, self)
        }
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
    Unhandled(Key)
}

trait IMode {
    fn run<'a>(&self, game: &mut Game, ui: &mut UI<'a>, mode: &mut Mode) -> bool;

    fn get_key<'a>(&self, game: &mut Game, ui: &mut UI<'a>, mode: &mut Mode) -> KeyStatus {
        let key = get_key();
        if let Key::Char(c) = key {
            if let Ok(dir) = Direction::try_from(c) {
                ui.map_scroller.scrollable.shift_viewport(dir.vec2d());
                ui.map_scroller.redraw(game, &mut ui.stdout);
                return KeyStatus::Handled(StateDisposition::Stay);
            }

            match c {
                conf::KEY_QUIT => {
                    *mode = Mode::Quit;
                    return KeyStatus::Handled(StateDisposition::Quit);
                },
                conf::KEY_EXAMINE => {
                    if let Some(cursor_viewport_loc) = ui.cursor_viewport_loc(*mode) {
                        *mode = Mode::Examine{cursor_viewport_loc: cursor_viewport_loc};
                        return KeyStatus::Handled(StateDisposition::Next);
                    } else {
                        ui.log_message(String::from("Couldn't get cursor loc"));
                    }
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

    fn map_loc_to_viewport_loc<'a>(ui: &mut UI<'a>, map_loc: Location) -> Option<Location> {
        let viewport_dims = ui.map_scroller.viewport_dims();
        let ref map = ui.map_scroller.scrollable;
        map.map_to_viewport_coords(map_loc, viewport_dims)
    }
}

trait IVisibleMode: IMode {
    fn rect(&self) -> Rect;

    fn goto(&self, x: u16, y: u16) -> Goto {
        let rect = self.rect();
        Goto(rect.left + x + 1, rect.top + y + 1)
    }

    fn clear(&self, stdout: &mut RawTerminal<StdoutLock>) {
        let rect = self.rect();
        let blank_string = (0..rect.width).map(|_| " ").collect::<String>();
        for y in 0..rect.height {
            write!(*stdout, "{}{}", self.goto(0, y), blank_string).unwrap();
        }
    }
}

pub struct TurnStartMode {}
impl IMode for TurnStartMode {
    fn run<'a>(&self, game: &mut Game, ui: &mut UI<'a>, mode: &mut Mode) -> bool {
        {
            // let cp = ui.current_player.borrow_mut();
            // cp.redraw(game, &mut ui.stdout);
            ui.current_player.redraw(game, &mut ui.stdout);
        }

        ui.log_message(format!("\nTurn {}, player {} go!", game.turn(), game.current_player()));




        // Process production set requests
        if !game.production_set_requests().is_empty() {
            // ui.set_productions_for_player(game);

            *mode = Mode::SetProductions;
            return true;
        }
        if !game.unit_move_requests().is_empty() {
            // ui.move_units_for_player(game);
            *mode = Mode::MoveUnits;
            return true;
        }

        let mut log_listener = |msg:String| {
            ui.log_message(msg);
        };

        let _player_num = match game.end_turn(&mut log_listener) {
            Ok(player_num) => player_num,
            Err(player_num) => player_num
        };

        true
    }
}

struct SetProductionsMode{}
impl IMode for SetProductionsMode {
    fn run<'a>(&self, game: &mut Game, ui: &mut UI<'a>, mode: &mut Mode) -> bool {

        if game.production_set_requests().is_empty() {
            ui.log_message("Productions set.".to_string());
            *mode = Mode::TurnStart;
            return true;
        }

        let loc = *game.production_set_requests().iter().next().unwrap();

        *mode = Mode::SetProduction{loc:loc};
        return true;
    }
}

struct SetProductionMode {
    loc: Location,
    rect: Rect
}

impl SetProductionMode {
    fn draw<'a>(&self, game: &Game, stdout: &mut RawTerminal<StdoutLock>) {
        let ref tile = game.current_player_tile(self.loc).unwrap();

        if let Some(ref city) = tile.city {
            write!(*stdout, "{}Set Production for {}", self.goto(0, 0), city).unwrap();

            for (i,unit_type) in game.valid_productions(self.loc).iter().enumerate() {
                write!(*stdout, "{}{} - {}",
                    self.goto(1, i as u16 + 2),
                    unit_type.key(),
                    unit_type.name()).unwrap();
            }
        }

        stdout.flush().unwrap();
    }
}

impl IMode for SetProductionMode {
    fn run<'a>(&self, game: &mut Game, ui: &mut UI<'a>, mode: &mut Mode) -> bool {
        ui.map_scroller.scrollable.center_viewport(self.loc);

        ui.draw(game);

        {
            let city = game.city(self.loc).unwrap();
            ui.log_message(format!("Requesting production target for {}", city ));
        }

        self.draw(game, &mut ui.stdout);

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {
                    if let Key::Char(c) = key {
                        if let Some(unit_type) = UnitType::from_key(&c) {
                            game.set_production(&self.loc, &unit_type).unwrap();
                            *mode = Mode::TurnStart;
                            return true;
                        }
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
    }
}

impl IVisibleMode for SetProductionMode {
    fn rect(&self) -> Rect {
        self.rect
    }
}

struct MoveUnitsMode {}
impl IMode for MoveUnitsMode {
    fn run<'a>(&self, game: &mut Game, _ui: &mut UI<'a>, mode: &mut Mode) -> bool {
        if !game.unit_move_requests().is_empty() {

            let loc = *game.unit_move_requests().iter().next().unwrap();

            *mode = Mode::MoveUnit{loc:loc};
            return true;
        }
        *mode = Mode::TurnStart;
        true
    }
}

struct MoveUnitMode{
    loc: Location
}
impl IMode for MoveUnitMode {
    fn run<'a>(&self, game: &mut Game, ui: &mut UI<'a>, mode: &mut Mode) -> bool {
        ui.map_scroller.scrollable.center_viewport(self.loc);

        {
            let unit = game.unit(self.loc).unwrap();
            ui.log_message(format!("Requesting orders for unit {} at {}", unit, self.loc));
        }

        ui.draw(game);

        {
            let unit = game.unit(self.loc).unwrap();
            ui.log_message(format!("Moving unit {}", unit));
        }

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {

                    if let Key::Char(c) = key {
                        match Direction::try_from(c) {
                            Ok(dir) => {

                                // ui.log_message(format!("Moving {}", c));

                                // let src: Vec2d<i32> = Vec2d::new(self.loc.x as i32, self.loc.y as i32);
                                // let dest = src + dir.vec2d();
                                //
                                // let src:  Vec2d<u16> = Vec2d::new(src.x as u16, src.y as u16);
                                // let dest: Vec2d<u16> = Vec2d::new(dest.x as u16, dest.y as u16);

                                let dest = self.loc.shift(dir);

                                match game.move_unit(self.loc, dest) {
                                    Ok(move_result) => {

                                        ui.animate_move(game, move_result);

                                        *mode = Mode::MoveUnits;
                                        return true;
                                    },
                                    Err(msg) => {
                                        ui.log_message(format!("Error: {}", msg));
                                    }
                                }
                            },
                            Err(_msg) => {
                                // println!("Error: {}", msg);
                                // sleep_millis(5000);
                            }
                        }
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
    }
}

struct QuitMode {}
impl IMode for QuitMode {
    fn run<'a>(&self, _game: &mut Game, ui: &mut UI<'a>, _mode: &mut Mode) -> bool {
        ui.cleanup();
        false
    }
}

struct ExamineMode {
    cursor_viewport_loc: Location
}
impl IMode for ExamineMode {
    fn run<'a>(&self, game: &mut Game, ui: &mut UI<'a>, mode: &mut Mode) -> bool {
        if let Some(tile) = {
            let ref map = ui.map_scroller.scrollable;

            map.draw_tile(game, &mut ui.stdout, self.cursor_viewport_loc, true, None);



            map.tile(game, self.cursor_viewport_loc)
        } {
            ui.log_message(format!("{:?}", tile));
            ui.stdout.flush().unwrap();
        }

        return match self.get_key(game, ui, mode) {
            KeyStatus::Unhandled(key) => {
                if key==Key::Esc {
                    *mode = Mode::TurnStart;
                } else if let Key::Char(c) = key {
                    if let Ok(dir) = Direction::try_from(c) {
                        let new_loc = self.cursor_viewport_loc.shift(dir);
                        if ui.viewport_rect().contains(new_loc) {
                            *mode = Mode::Examine{cursor_viewport_loc: new_loc};
                        }
                    }
                }

                {
                    let ref map = ui.map_scroller.scrollable;

                    map.draw_tile(game, &mut ui.stdout, self.cursor_viewport_loc, false, None);
                    ui.stdout.flush().unwrap();
                }

                true
            },
            KeyStatus::Handled(state_disposition) => {
                match state_disposition {
                    StateDisposition::Quit => return false,
                    StateDisposition::Next => return true,
                    StateDisposition::Stay => return true//examine mode doesn't loop, so just move on to the next state
                }
            }
        }
    }
}
