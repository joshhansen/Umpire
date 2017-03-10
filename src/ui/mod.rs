//!
//! The user interface.
//!
//! Making use of the abstract game engine, implement a user interface for the game.
extern crate termion;

use std::cell::RefCell;
use std::convert::TryFrom;
use std::io::{Write, stdin, StdoutLock};
use std::rc::Rc;

use termion::event::Key;
use termion::input::TermRead;

use conf;
use conf::HEADER_HEIGHT;
use game::Game;
use util::{Dims,Direction,Rect,Location,Vec2d,sleep_millis};

/// 0-indexed variant of Goto
pub fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

// pub enum Mode {
//     General,
//     SetProduction{loc:Location},
//     MoveUnit{loc:Location},
//     // PanMap,
//     // Help
// }

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
mod set_production;
pub mod sound;

use self::scroll::Scroller;
use self::indicators::{CurrentPlayer,Turn};
use self::log::LogArea;
use self::map::Map;
use self::set_production::SetProduction;
use self::sound::Noisy;

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
        height: term_dims.height - viewport_rect.height - 2
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

type Scene = Vec<Rc<RefCell<Component>>>;
impl Draw for Scene {
    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        for component in self.iter() {
            component.borrow().draw(game, stdout);
        }
    }
}

impl Redraw for Scene {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        for component in self.iter() {
            component.borrow().redraw(game, stdout);
        }
    }
}

struct MoveUnit {
    rect: Rect,
    loc: Location
}

impl MoveUnit {
    fn new(rect: Rect, loc: Location) -> Self {
        MoveUnit {
            rect: rect,
            loc: loc
        }
    }
}

impl Draw for MoveUnit {
    fn draw(&self, _game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        write!(*stdout, "{}Move Unit", self.goto(0, 0)).unwrap();
    }
}

impl Redraw for MoveUnit {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        self.draw(game, stdout);
    }
}

impl Keypress for MoveUnit {
    fn keypress(&mut self, key: &Key, game: &mut Game) {

        if let Key::Char(c) = *key {
            match Direction::try_from(c) {
                Ok(dir) => {

                    println!("Moving {}", c);

                    let src: Vec2d<i32> = Vec2d::new(self.loc.x as i32, self.loc.y as i32);
                    let dest = src + dir.vec2d();

                    let src:  Vec2d<u16> = Vec2d::new(src.x as u16, src.y as u16);
                    let dest: Vec2d<u16> = Vec2d::new(dest.x as u16, dest.y as u16);

                    match game.move_unit(src, dest) {
                        Ok(_combat_outcomes) => {
                            //FIXME do something with these combat outcomes
                            sleep_millis(350);
                        },
                        Err(msg) => {
                            println!("Error: {}", msg);
                        }
                    }
                },
                Err(_msg) => {
                    // println!("Error: {}", msg);
                    // sleep_millis(5000);
                }
            }
        }
    }
}

impl Component for MoveUnit {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}


pub struct UI<'a> {
    // mode: Mode,
    stdout: termion::raw::RawTerminal<StdoutLock<'a>>,
    term_dims: Dims,
    viewport_size: ViewportSize,

    map_scroller: Rc<RefCell<Scroller<Map>>>,
    log: Rc<RefCell<LogArea>>,
    current_player: Rc<RefCell<CurrentPlayer>>,

    scene: Scene,
    keep_going: bool
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
        let map_scroller = Rc::new(RefCell::new( Scroller::new( &map_scroller_rect, map ) ));

        let log_rect = log_area_rect(&viewport_rect, &term_dims);
        let log = Rc::new(RefCell::new(LogArea::new(&log_rect)));

        let cp_rect = current_player_rect();
        let current_player = Rc::new(RefCell::new(CurrentPlayer::new(cp_rect)));

        let mut ui = UI {
            // game: game,
            // mode: Mode::General,
            stdout: stdout,
            term_dims: term_dims,
            viewport_size: viewport_size,

            map_scroller: map_scroller.clone(),
            log: log.clone(),
            current_player: current_player.clone(),

            scene: Scene::new(),
            keep_going: true
        };

        // ui.scene.push(map.clone());
        ui.scene.push(map_scroller.clone());
        ui.scene.push(log.clone());
        ui.scene.push(current_player.clone());

        let turn_rect = turn_rect(&cp_rect);
        ui.scene.push(Rc::new(RefCell::new(Turn::new(&turn_rect))));
        ui
    }

    fn take_input(&mut self, game: &mut Game) {
        let stdin = stdin();
        let c = stdin.keys().next().unwrap().unwrap();

        let mut component_is_done = false;

        if let Some(component) = self.scene.last_mut() {
            component.borrow_mut().keypress(&c, game);
            component_is_done |= component.borrow().is_done();

            if component_is_done {
                component.borrow().clear(&mut self.stdout);
            }
        }

        if component_is_done {
            self.scene.pop();
        }

        self.map_scroller.borrow_mut().keypress(&c, game);
        self.map_scroller.borrow().redraw(game, &mut self.stdout);

        match c {
            Key::Char(conf::KEY_QUIT) => self.keep_going = false,
            Key::Char(conf::KEY_VIEWPORT_SIZE_ROTATE) => {
                let new_size = match self.viewport_size {
                    ViewportSize::REGULAR => ViewportSize::THEATER,
                    ViewportSize::THEATER => ViewportSize::FULLSCREEN,
                    ViewportSize::FULLSCREEN => ViewportSize::REGULAR
                };

                self.set_viewport_size(game, new_size);
                self.scene.redraw(game, &mut self.stdout);
            }
            _ => {}
        }
    }

    fn log_message(&mut self, message: String) {
        self.log.borrow_mut().log_message(message);
        // self.log.borrow_mut().redraw(game, &mut self.stdout);
        self.log.borrow_mut().redraw_lite(&mut self.stdout);
    }

    fn set_productions_for_player(&mut self, game: &mut Game) {
        while self.keep_going {
            if game.production_set_requests().is_empty() {
                self.log_message("Productions set.".to_string());
                break;
            }

            let loc = *game.production_set_requests().iter().next().unwrap();

            self.map_scroller.borrow_mut().scrollable.center_viewport(&loc);

            // self.mode = Mode::SetProduction{loc:loc};
            let viewport_rect = self.viewport_rect();

            {
                let city = game.city(loc).unwrap();
                self.log_message(format!("Requesting production target for {}", city ));
            }

            self.scene.push(Rc::new(RefCell::new(SetProduction::new(
                Rect {
                    left: viewport_rect.width + V_SCROLLBAR_WIDTH + 1,
                    top: HEADER_HEIGHT + 1,
                    width: self.term_dims.width - viewport_rect.width - 2,
                    height: self.term_dims.height - HEADER_HEIGHT
                },
                loc)
            )));

            self.draw(game);
            self.take_input(game);
        }
    }

    fn move_units_for_player(&mut self, game: &mut Game) {
        while self.keep_going && !game.unit_move_requests().is_empty() {

            let loc = *game.unit_move_requests().iter().next().unwrap();

            self.map_scroller.borrow_mut().scrollable.center_viewport(&loc);

            // self.mode = Mode::MoveUnit{loc:loc};
            let viewport_rect = self.viewport_rect();

            {
                let unit = match game.unit(loc) {
                    Some(unit) => unit,
                    None => panic!(format!("Unit not at {}", loc))
                };
                self.log_message(format!("Requesting orders for unit {} at {}", unit, loc));

                // let freq = unit.freq();
                // let amp = unit.amp();
                //
                // thread::spawn(move || {
                //     loop {
                //         let mut stream = sound::make_noise(amp, freq).unwrap();
                //
                //         while let Ok(true) = stream.is_active() {
                //             sleep_millis(100);
                //         }
                //
                //         stream.stop().unwrap();
                //         stream.close().unwrap();
                //     }
                // });
            }

            self.scene.push(Rc::new(RefCell::new(MoveUnit::new(
                sidebar_rect(&viewport_rect, &game.map_dims),
                loc)
            )));

            self.draw(game);
            self.take_input(game);
            self.draw(game);
        }
    }

    pub fn run(&mut self, game: &mut Game) {
        // loop through endless game turns
        while self.keep_going {


            {
                let cp = self.current_player.borrow_mut();
                cp.redraw(game, &mut self.stdout);
            }

            self.log_message(format!("\nTurn {}, player {} go!", game.turn(), game.current_player()));

            // Process production set requests
            if !game.production_set_requests().is_empty() {
                self.set_productions_for_player(game);
            }
            if !game.unit_move_requests().is_empty() {
                self.move_units_for_player(game);
            }

            let mut log_listener = |msg:String| {
                self.log_message(msg);
            };

            let _player_num = match game.end_turn(&mut log_listener) {
                Ok(player_num) => player_num,
                Err(player_num) => player_num
            };
        }

        self.cleanup();
    }

    fn set_viewport_size(&mut self, game: &Game, viewport_size: ViewportSize) {
        self.viewport_size = viewport_size;
        self.map_scroller.borrow_mut().set_rect(self.viewport_size.rect(&game.map_dims));
        self.draw(game);
    }

    pub fn draw(&mut self, game: &Game) {
        write!(self.stdout, "{}{}{}{}{}",
            termion::clear::All,
            goto(0,0),
            termion::style::Underline,
            conf::APP_NAME,
            termion::style::Reset
        ).unwrap();

        self.scene.draw(game, &mut self.stdout);

        write!(self.stdout, "{}{}", termion::style::Reset, termion::cursor::Hide).unwrap();
        self.stdout.flush().unwrap();
    }

    fn viewport_rect(&self) -> Rect {
        self.viewport_size.rect(&self.term_dims)
    }

    fn cleanup(&mut self) {
        write!(self.stdout, "{}{}", goto(0, self.term_dims.height), termion::style::Reset).unwrap();
    }
}
