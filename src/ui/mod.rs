//!
//! The user interface.
//!
//! Making use of the abstract game engine, implement a user interface for the game.
extern crate termion;

use std::cell::RefCell;
use std::process::exit;
use std::io::{Write, stdout, stdin, StdoutLock};
use std::rc::Rc;

use termion::color::{Fg, AnsiValue};
use termion::event::Key;
use termion::input::TermRead;

use conf;
use conf::HEADER_HEIGHT;
use game::Game;
use map::Tile;
use util::{Dims,Rect,Vec2d,Location};

/// 0-indexed variant of Goto
pub fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

pub enum Mode {
    General,
    SetProduction{loc:Location},
    MoveUnit{loc:Location},
    PanMap,
    Help
}

trait Draw {
    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>);
}

trait Redraw {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>);
}

trait Keypress {
    fn keypress(&mut self, key: &Key, game: &mut Game);
}

trait Component : Draw+Redraw+Keypress {
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

mod indicators;
mod log;
mod map;
mod set_production;

use self::indicators::{CurrentPlayer,Turn};
use self::log::LogArea;
use self::map::Map;
use self::set_production::SetProduction;







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

pub struct UI<'a> {
    game: Game,
    mode: Mode,
    stdout: termion::raw::RawTerminal<StdoutLock<'a>>,
    term_dims: Dims,
    viewport_size: ViewportSize,
    // viewport_rect: Rect,
    viewport_offset: Vec2d<u16>,
    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>,

    map: Rc<RefCell<Map>>,
    log: Rc<RefCell<LogArea>>,
    current_player: Rc<RefCell<CurrentPlayer>>,

    scene: Scene
}

impl<'b> UI<'b> {
    pub fn new(
        game: Game,
        stdout: termion::raw::RawTerminal<StdoutLock<'b>>,
        term_dims: Dims
    ) -> Self {

        let offset = Vec2d{ x: game.map_dims.width/2, y: game.map_dims.height/2 };

        let viewport_size = ViewportSize::REGULAR;
        let viewport_rect = viewport_size.rect(&term_dims);

        let map = Rc::new(RefCell::new(Map::new(&viewport_rect, &game.map_dims)));

        let log_rect = log_area_rect(&viewport_rect, &term_dims);
        let log = Rc::new(RefCell::new(LogArea::new(&log_rect)));

        let cp_rect = current_player_rect();
        let current_player = Rc::new(RefCell::new(CurrentPlayer::new(cp_rect, None)));

        let mut ui = UI {
            game: game,
            mode: Mode::General,
            stdout: stdout,
            term_dims: term_dims,
            viewport_size: viewport_size,
            viewport_offset: offset,
            old_h_scroll_x: None,
            old_v_scroll_y: None,

            map: map.clone(),
            log: log.clone(),
            current_player: current_player.clone(),

            scene: Scene::new()
        };

        ui.scene.push(map.clone());
        ui.scene.push(log.clone());
        ui.scene.push(current_player.clone());

        let turn_rect = turn_rect(&cp_rect);
        ui.scene.push(Rc::new(RefCell::new(Turn::new(&turn_rect))));
        ui
    }

    fn take_input(&mut self) {
        let stdin = stdin();
        for c in stdin.keys() {
            let c = c.unwrap();

            let mut component_is_done = false;

            if let Some(component) = self.scene.last_mut() {
                component.borrow_mut().keypress(&c, &mut self.game);
                component_is_done |= component.borrow().is_done();

                if component_is_done {
                    component.borrow().clear(&mut self.stdout);
                }
            }

            if component_is_done {
                self.scene.pop();
            }

            self.map.borrow_mut().keypress(&c, &mut self.game);
            self.map.borrow().redraw(&self.game, &mut self.stdout);

            match c {
                Key::Char(conf::KEY_QUIT) => self.quit(),
                Key::Char(conf::KEY_VIEWPORT_SIZE_ROTATE) => {
                    let new_size = match self.viewport_size {
                        ViewportSize::REGULAR => ViewportSize::THEATER,
                        ViewportSize::THEATER => ViewportSize::FULLSCREEN,
                        ViewportSize::FULLSCREEN => ViewportSize::REGULAR
                    };

                    self.set_viewport_size(new_size);
                    self.scene.redraw(&self.game, &mut self.stdout);
                }
                _ => {}
            }
        }
    }

    pub fn run(&mut self) {
        self.log.borrow_mut().log_message("blah".to_string());

        // loop through endless game turns
        loop {
            let player_num = match self.game.begin_next_player_turn() {
                Ok(player_num) => player_num,
                Err(player_num) => player_num
            };

            self.current_player.borrow_mut().player = Some(player_num);
            // let mut curplay: &mut CurrentPlayer = self.current_player;
            //
            // curplay.set_player(player_num);

            // Process production set requests
            loop {
                if self.game.production_set_requests().len() < 1 {
                    break;
                }

                let loc = *self.game.production_set_requests().iter().next().unwrap();

                self.map.borrow_mut().center_viewport(&loc);

                self.mode = Mode::SetProduction{loc:loc};
                let viewport_rect = self.viewport_rect();

                self.scene.push(Rc::new(RefCell::new(SetProduction::new(
                    Rect{
                        left: viewport_rect.width + V_SCROLLBAR_WIDTH + 1,
                        top: HEADER_HEIGHT + 1,
                        width: self.term_dims.width - viewport_rect.width - 2,
                        height: self.term_dims.height - HEADER_HEIGHT
                    },
                    loc)
                )));

                self.draw();
                self.take_input();
            }

            //TODO Process unit move requests
        }
    }

    fn set_viewport_size(&mut self, viewport_size: ViewportSize) {
        self.viewport_size = viewport_size;
        self.draw();
    }

    pub fn draw(&mut self) {
        write!(self.stdout, "{}{}{}{}{}",
            termion::clear::All,
            goto(0,0),
            termion::style::Underline,
            conf::APP_NAME,
            termion::style::Reset
        ).unwrap();

        self.draw_scroll_bars();

        self.scene.draw(&self.game, &mut self.stdout);

        write!(self.stdout, "{}{}", termion::style::Reset, termion::cursor::Hide).unwrap();
        self.stdout.flush().unwrap();
    }

    fn draw_scroll_bars(&mut self) {
        let viewport_rect = self.viewport_rect();
        let h_scroll_x: u16 = (viewport_rect.width as f32 * (self.viewport_offset.x as f32 / self.game.map_dims.width as f32)) as u16;
        let h_scroll_y = viewport_rect.bottom() + 1;


        //FIXME There must be a cleaner way to do this
        match self.old_h_scroll_x {
            None => {
                self.draw_scroll_mark(h_scroll_x, h_scroll_y, '^');
            },
            Some(old_h_scroll_x) => {
                if h_scroll_x != old_h_scroll_x {
                    self.erase(old_h_scroll_x, h_scroll_y);
                    self.draw_scroll_mark(h_scroll_x, h_scroll_y, '^');
                }
            }
        }
        self.old_h_scroll_x = Some(h_scroll_x);

        let v_scroll_x = viewport_rect.right();
        let v_scroll_y: u16 = HEADER_HEIGHT + (viewport_rect.height as f32 * (self.viewport_offset.y as f32 / self.game.map_dims.height as f32)) as u16;

        //FIXME There must be a cleaner way to do this
        match self.old_v_scroll_y {
            None => {
                self.draw_scroll_mark(v_scroll_x, v_scroll_y, '<');
            },
            Some(old_v_scroll_y) => {
                if v_scroll_y != old_v_scroll_y {
                    self.erase(v_scroll_x, old_v_scroll_y);
                    self.draw_scroll_mark(v_scroll_x, v_scroll_y, '<');
                }
            }
        }
        self.old_v_scroll_y = Some(v_scroll_y);
    }

    // Utility methods
    fn draw_scroll_mark(&mut self, x: u16, y: u16, sym: char) {
        write!(self.stdout, "{}{}{}{}", termion::style::Reset, goto(x,y), Fg(AnsiValue(11)), sym).unwrap();
    }

    fn erase(&mut self, x: u16, y: u16) {
        write!(self.stdout, "{}{} ", termion::style::Reset, goto(x,y)).unwrap();
    }

    pub fn quit(&mut self) {
        write!(self.stdout, "{}{}Thanks for playing {}!\n\n", goto(0, self.term_dims.height), termion::style::Reset, conf::APP_NAME).unwrap();
        exit(0);
    }

    fn viewport_rect(&self) -> Rect {
        self.viewport_size.rect(&self.term_dims)
    }
}
