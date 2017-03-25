//!
//! The user interface.
//!
//! Making use of the abstract game engine, implement a user interface for the game.
use std::io::{Write, StdoutLock};

use termion;
use termion::clear;
use termion::event::Key;

use conf;
use conf::HEADER_HEIGHT;
use game::{Game,MoveResult};
use unit::{Sym};
use unit::combat::{CombatCapable,CombatOutcome,CombatParticipant};
use util::{Dims,Rect,Location,sleep_millis};

/// 0-indexed variant of Goto
pub fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

pub trait Draw {
    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>);
}

pub trait Redraw {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>);
}

pub trait Keypress {
    fn keypress(&mut self, key: &Key, game: &mut Game);
}

pub trait Component : Draw+Redraw+Keypress {
    fn set_rect(&mut self, rect: Rect);

    fn rect(&self) -> Rect;

    fn is_done(&self) -> bool;



    fn goto(&self, x: u16, y: u16) -> termion::cursor::Goto {
        let rect = self.rect();
        goto(rect.left + x, rect.top + y)
    }

    fn clear(&self, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        let rect = self.rect();
        let blank_string = (0..rect.width).map(|_| " ").collect::<String>();
        for y in 0..rect.height {
            write!(*stdout, "{}{}", self.goto(0, y), blank_string).unwrap();
        }
    }

    // fn draw_window_frame(&self, title: &str, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
    //
    // }
}

mod scroll;

mod indicators;
mod log;
mod map;
pub mod mode;
// pub mod sound;

use self::scroll::Scroller;
use self::indicators::{CurrentPlayer,Turn};
use self::log::LogArea;
use self::map::Map;
use self::mode::Mode;

enum ViewportSize {
    REGULAR,
    THEATER,
    FULLSCREEN
}

impl ViewportSize {
    fn rect(&self, term_dims: &Dims) -> Rect {
        match *self {
            ViewportSize::REGULAR => Rect {
                left: 0,
                top: HEADER_HEIGHT,
                width: (term_dims.width - V_SCROLLBAR_WIDTH) / 2,
                height: 25
            },
            ViewportSize::THEATER => Rect {
                left: 0,
                top: HEADER_HEIGHT,
                width: term_dims.width - V_SCROLLBAR_WIDTH,
                height: 25
            },
            ViewportSize::FULLSCREEN => Rect {
                left: 0,
                top: 0,
                width: term_dims.width - V_SCROLLBAR_WIDTH,
                height: term_dims.height - H_SCROLLBAR_HEIGHT - 1
            }
        }
    }
}

fn current_player_rect() -> Rect {
    Rect {
        left: 10,
        top: 0,
        width: 21,
        height: 1
    }
}

fn turn_rect(current_player_rect: &Rect) -> Rect {
    Rect {
        left: current_player_rect.right() + 2,
        top: 0,
        width: 11,
        height: 1
    }
}

fn log_area_rect(viewport_rect: &Rect, term_dims: &Dims) -> Rect {
    Rect {
        left: 0,
        top: viewport_rect.bottom() + 2,
        width: viewport_rect.width,
        height: term_dims.height - viewport_rect.height - 3
    }
}

fn sidebar_rect(viewport_rect: &Rect, term_dims: &Dims) -> Rect {
    Rect {
        left: viewport_rect.right() + 1,
        top: viewport_rect.top,
        width: term_dims.width - viewport_rect.left - viewport_rect.width,
        height: term_dims.height
    }
}

const H_SCROLLBAR_HEIGHT: u16 = 1;
const V_SCROLLBAR_WIDTH: u16 = 1;

pub struct UI<'a> {
    stdout: termion::raw::RawTerminal<StdoutLock<'a>>,
    term_dims: Dims,
    viewport_size: ViewportSize,

    map_scroller: Scroller<Map>,
    log: LogArea,
    current_player: CurrentPlayer,
    turn: Turn
}

impl<'b> UI<'b> {
    pub fn new(
        map_dims: &Dims,
        term_dims: Dims,
        stdout: termion::raw::RawTerminal<StdoutLock<'b>>,
    ) -> Self {
        let viewport_size = ViewportSize::REGULAR;
        let viewport_rect = viewport_size.rect(&term_dims);

        let map = Map::new(&viewport_rect, map_dims);

        let map_scroller_rect = Rect {
            left: viewport_rect.left,
            top: viewport_rect.top,
            width: viewport_rect.width + 1,
            height: viewport_rect.height + 1
        };
        let mut map_scroller = Scroller::new(&map_scroller_rect, map);
        map_scroller.set_rect(viewport_size.rect(&map_dims));

        let log_rect = log_area_rect(&viewport_rect, &term_dims);
        let log = LogArea::new(&log_rect);

        let cp_rect = current_player_rect();
        let current_player = CurrentPlayer::new(cp_rect);

        let mut ui = UI {
            stdout: stdout,
            term_dims: term_dims,
            viewport_size: viewport_size,

            map_scroller: map_scroller,
            log: log,
            current_player: current_player,

            turn: Turn::new(&turn_rect(&cp_rect))
        };

        write!(ui.stdout, "{}", clear::All).unwrap();

        ui
    }

    // fn take_input(&mut self, game: &mut Game) {
    //     let stdin = stdin();
    //     let c = stdin.keys().next().unwrap().unwrap();
    //
    //     let mut component_is_done = false;
    //
    //     if let Some(component) = self.scene.last_mut() {
    //         component.borrow_mut().keypress(&c, game);
    //         component_is_done |= component.borrow().is_done();
    //
    //         if component_is_done {
    //             component.borrow().clear(&mut self.stdout);
    //         }
    //     }
    //
    //     if component_is_done {
    //         self.scene.pop();
    //     }
    //
    //     self.map_scroller.borrow_mut().keypress(&c, game);
    //     self.map_scroller.borrow().redraw(game, &mut self.stdout);
    //
    //     match c {
    //         Key::Char(conf::KEY_QUIT) => self.keep_going = false,
    //         Key::Char(conf::KEY_VIEWPORT_SIZE_ROTATE) => {
    //             let new_size = match self.viewport_size {
    //                 ViewportSize::REGULAR => ViewportSize::THEATER,
    //                 ViewportSize::THEATER => ViewportSize::FULLSCREEN,
    //                 ViewportSize::FULLSCREEN => ViewportSize::REGULAR
    //             };
    //
    //             self.set_viewport_size(game, new_size);
    //             self.scene.redraw(game, &mut self.stdout);
    //         }
    //         _ => {}
    //     }
    // }

    pub fn log_message(&mut self, message: String) {
        self.log.log_message(message);
        self.log.redraw_lite(&mut self.stdout);
    }

    fn set_viewport_size(&mut self, game: &Game, viewport_size: ViewportSize) {
        self.viewport_size = viewport_size;
        self.map_scroller.set_rect(self.viewport_size.rect(&game.map_dims()));
        self.draw(game);
    }

    pub fn rotate_viewport_size(&mut self, game: &Game) {
        let new_size = match self.viewport_size {
            ViewportSize::REGULAR => ViewportSize::THEATER,
            ViewportSize::THEATER => ViewportSize::FULLSCREEN,
            ViewportSize::FULLSCREEN => ViewportSize::REGULAR
        };

        self.set_viewport_size(game, new_size);
        self.redraw(game);
    }

    pub fn draw(&mut self, game: &Game) {
        write!(self.stdout, "{}{}{}{}{}",
            termion::clear::All,
            goto(0,0),
            termion::style::Underline,
            conf::APP_NAME,
            termion::style::Reset
        ).unwrap();

        self.log.draw_lite(&mut self.stdout);
        self.current_player.draw(game, &mut self.stdout);
        self.map_scroller.draw(game, &mut self.stdout);
        self.turn.draw(game, &mut self.stdout);

        write!(self.stdout, "{}{}", termion::style::Reset, termion::cursor::Hide).unwrap();
        self.stdout.flush().unwrap();
    }

    fn redraw(&mut self, game: &Game) {
        self.log.redraw_lite(&mut self.stdout);
        self.current_player.redraw(game, &mut self.stdout);
        self.map_scroller.redraw(game, &mut self.stdout);
        self.turn.redraw(game, &mut self.stdout);
    }

    fn animate_move(&mut self, game: &Game, move_result: MoveResult) {
        let unit_symbol = move_result.unit().sym();
        let mut current_loc = move_result.starting_loc();

        for move_ in move_result.moves() {
            let target_loc = move_.loc();

            if let Some(ref combat) = *move_.unit_combat() {
                self.animate_combat(game, combat, current_loc, target_loc);
            }

            if let Some(ref combat) = *move_.city_combat() {
                self.animate_combat(game, combat, current_loc, target_loc);
            }

            self.log_message(format!("Unit {} {}", move_result.unit(), if move_.moved_successfully() {"victorious"} else {"destroyed"}));

            let viewport_dims = self.map_scroller.viewport_dims();
            let ref map = self.map_scroller.scrollable;

            // Erase the unit's symbol at its old location
            if let Some(current_viewport_loc) = map.map_to_viewport_coords(current_loc, viewport_dims) {
                map.draw_tile(game, &mut self.stdout, current_viewport_loc, false, None);//By now the model has no unit in the old location, so just draw that tile as per usual
            }

            if move_.moved_successfully() {
                // Draw the unit's symbol at its new location
                if let Some(target_viewport_loc) = map.map_to_viewport_coords(target_loc, viewport_dims) {
                    map.draw_tile(game, &mut self.stdout, target_viewport_loc, false, Some(unit_symbol));
                }
            }

            current_loc = target_loc;


            self.stdout.flush().unwrap();
            sleep_millis(400);
        }


    }

    fn animate_combat<A:CombatCapable+Sym,D:CombatCapable+Sym>(&mut self, game: &Game, outcome: &CombatOutcome<A,D>, attacker_loc: Location,
                defender_loc: Location) {

        let viewport_dims = self.map_scroller.viewport_dims();
        let ref map = self.map_scroller.scrollable;

        let attacker_viewport_loc = map.map_to_viewport_coords(attacker_loc, viewport_dims);
        let defender_viewport_loc = map.map_to_viewport_coords(defender_loc, viewport_dims);
        let attacker_sym = outcome.attacker().sym();
        let defender_sym = outcome.defender().sym();

        for damage_recipient in outcome.received_damage_sequence() {
            let viewport_loc = match damage_recipient {
                &CombatParticipant::Attacker => attacker_viewport_loc,
                &CombatParticipant::Defender => defender_viewport_loc
            };
            let sym = match damage_recipient {
                &CombatParticipant::Attacker => attacker_sym,
                &CombatParticipant::Defender => defender_sym
            };

            if let Some(viewport_loc) = viewport_loc {
                map.draw_tile(game, &mut self.stdout, viewport_loc, true, Some(sym));
                sleep_millis(100);
                map.draw_tile(game, &mut self.stdout, viewport_loc, false, Some(sym));
            } else {
                sleep_millis(100);
            }
        }
    }

    fn viewport_rect(&self) -> Rect {
        self.viewport_size.rect(&self.term_dims)
    }

    fn cleanup(&mut self) {
        write!(self.stdout, "{}{}", goto(0, self.term_dims.height), termion::style::Reset).unwrap();
    }

    pub fn cursor_viewport_loc(&self, mode: Mode) -> Option<Location> {
        let viewport_dims = self.map_scroller.viewport_dims();
        let ref map = self.map_scroller.scrollable;

        match mode {
            Mode::SetProduction{loc} => map.map_to_viewport_coords(loc, viewport_dims),
            Mode::MoveUnit{loc}      => map.map_to_viewport_coords(loc, viewport_dims),
            _                        => None
        }
    }
}
