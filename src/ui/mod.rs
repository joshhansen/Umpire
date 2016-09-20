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

trait ScrollableComponent : Component {
    fn offset(&self) -> Vec2d<u16>;
    fn scroll_relative(&mut self, offset: Vec2d<i32>);
}

struct Scroller<C:ScrollableComponent> {
    rect: Rect,
    scrollable: C,
    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>
}

impl<C:ScrollableComponent> Scroller<C> {
    fn new(rect: &Rect, scrollable: C) -> Self {
        Scroller {
            rect: *rect,
            scrollable: scrollable,
            old_h_scroll_x: None,
            old_v_scroll_y: None
        }
    }

    fn h_scroll_x(&self, map_width: u16) -> u16 {
        ((self.rect.width-1) as f32 * (self.scrollable.offset().x as f32 / map_width as f32)) as u16
    }

    fn v_scroll_y(&self, map_height: u16) -> u16 {
        (self.rect.height as f32 * (self.scrollable.offset().y as f32 / map_height as f32)) as u16
    }

    fn draw_scroll_bars(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        let viewport_rect = self.scrollable.rect();
        let h_scroll_x: u16 = self.h_scroll_x(game.map_dims.width);
        let h_scroll_y = viewport_rect.bottom();

        //FIXME There must be a cleaner way to do this
        if let Some(old_h_scroll_x) = self.old_h_scroll_x {
            if h_scroll_x != old_h_scroll_x {
                self.erase(stdout, old_h_scroll_x, h_scroll_y);
                self.draw_scroll_mark(stdout, h_scroll_x, h_scroll_y, '^');
            }
        } else {
            self.draw_scroll_mark(stdout, h_scroll_x, h_scroll_y, '^');
        }

        let v_scroll_x = viewport_rect.right();
        let v_scroll_y: u16 = self.v_scroll_y(game.map_dims.height);

        //FIXME There must be a cleaner way to do this
        if let Some(old_v_scroll_y) = self.old_v_scroll_y {
            if v_scroll_y != old_v_scroll_y {
                self.erase(stdout, v_scroll_x, old_v_scroll_y);
                self.draw_scroll_mark(stdout, v_scroll_x, v_scroll_y, '<');
            }
        } else {
            self.draw_scroll_mark(stdout, v_scroll_x, v_scroll_y, '<');
        }
    }

    // Utility methods
    fn draw_scroll_mark(&self, stdout: &mut termion::raw::RawTerminal<StdoutLock>, x: u16, y: u16, sym: char) {
        write!(*stdout, "{}{}{}{}", termion::style::Reset, self.goto(x,y), Fg(AnsiValue(11)), sym).unwrap();
    }

    fn erase(&self, stdout: &mut termion::raw::RawTerminal<StdoutLock>, x: u16, y: u16) {
        write!(*stdout, "{}{} ", termion::style::Reset, self.goto(x,y)).unwrap();
    }

    fn scroll_relative(&mut self, game: &Game, offset: Vec2d<i32>) {
        self.old_h_scroll_x = Some(self.h_scroll_x(game.map_dims.width));
        self.old_v_scroll_y = Some(self.v_scroll_y(game.map_dims.height));
        self.scrollable.scroll_relative(offset);

    }
}

impl<C:ScrollableComponent> Draw for Scroller<C> {
    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        self.draw_scroll_bars(game, stdout);
        self.scrollable.draw(game, stdout);
    }
}

impl<C:ScrollableComponent> Redraw for Scroller<C> {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        self.draw_scroll_bars(game, stdout);
        self.scrollable.redraw(game, stdout);
    }
}

impl<C:ScrollableComponent> Keypress for Scroller<C> {
    fn keypress(&mut self, key: &Key, game: &mut Game) {
        match *key {
            Key::Char(conf::KEY_VIEWPORT_SHIFT_LEFT)       => self.scroll_relative(game, Vec2d{x:-1, y: 0}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_RIGHT)      => self.scroll_relative(game, Vec2d{x: 1, y: 0}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_UP)         => self.scroll_relative(game, Vec2d{x: 0, y:-1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN)       => self.scroll_relative(game, Vec2d{x: 0, y: 1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_LEFT)    => self.scroll_relative(game, Vec2d{x:-1, y:-1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_RIGHT)   => self.scroll_relative(game, Vec2d{x: 1, y:-1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_LEFT)  => self.scroll_relative(game, Vec2d{x:-1, y: 1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_RIGHT) => self.scroll_relative(game, Vec2d{x: 1, y: 1}),
            _ => {}
        }
    }
}

impl<C:ScrollableComponent> Component for Scroller<C> {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.scrollable.set_rect(rect);
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
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
    mode: Mode,
    stdout: termion::raw::RawTerminal<StdoutLock<'a>>,
    term_dims: Dims,
    viewport_size: ViewportSize,

    map_scroller: Rc<RefCell<Scroller<Map>>>,
    log: Rc<RefCell<LogArea>>,
    current_player: Rc<RefCell<CurrentPlayer>>,

    scene: Scene
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
        let current_player = Rc::new(RefCell::new(CurrentPlayer::new(cp_rect, None)));

        let mut ui = UI {
            // game: game,
            mode: Mode::General,
            stdout: stdout,
            term_dims: term_dims,
            viewport_size: viewport_size,

            map_scroller: map_scroller.clone(),
            log: log.clone(),
            current_player: current_player.clone(),

            scene: Scene::new()
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
        for c in stdin.keys() {
            let c = c.unwrap();

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
                Key::Char(conf::KEY_QUIT) => self.quit(),
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
    }

    pub fn run(&mut self, game: &mut Game) {
        self.log.borrow_mut().log_message("blah".to_string());

        // loop through endless game turns
        loop {
            let player_num = match game.begin_next_player_turn() {
                Ok(player_num) => player_num,
                Err(player_num) => player_num
            };

            self.current_player.borrow_mut().player = Some(player_num);

            // Process production set requests
            loop {
                if game.production_set_requests().len() < 1 {
                    break;
                }

                let loc = *game.production_set_requests().iter().next().unwrap();

                self.map_scroller.borrow_mut().scrollable.center_viewport(&loc);

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

                self.draw(game);
                self.take_input(game);
            }

            //TODO Process unit move requests
        }
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

    pub fn quit(&mut self) {
        write!(self.stdout, "{}{}Thanks for playing {}!\n\n", goto(0, self.term_dims.height), termion::style::Reset, conf::APP_NAME).unwrap();
        exit(0);
    }

    fn viewport_rect(&self) -> Rect {
        self.viewport_size.rect(&self.term_dims)
    }
}
