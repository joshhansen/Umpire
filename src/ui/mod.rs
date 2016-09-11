//!
//! The user interface.
//!
//! Making use of the abstract game engine, implement a user interface for the game.
extern crate termion;

use std::process::exit;
use std::io::{Write, stdout, stdin, StdoutLock};
use std::ops::{Add,Rem};

use termion::color::{Fg, Bg, AnsiValue};
use termion::event::Key;
use termion::input::TermRead;

use conf;
use game::Game;
use map::Tile;
use unit::{City,Named,PlayerNum,UnitType};
use util::{Dims,Rect,Vec2d,Location};

/// 0-indexed variant of Goto
pub fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

fn goto_within_rect(rect: Rect, x: u16, y: u16) -> termion::cursor::Goto {
    goto(rect.left + x, rect.top + y)
}

pub enum Mode {
    General,
    SetProduction{loc:Location},
    MoveUnit{loc:Location},
    PanMap,
    Help
}

trait Component {
    fn set_rect(&mut self, rect: Rect);

    fn rect(&self) -> Rect;

    fn is_done(&self) -> bool;

    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>);

    fn keypress(&mut self, key: &Key, game: &mut Game);

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

struct Map {

}

// impl Component for Map {
//     fn rect(&self, ui: &UI) -> Rect {
//         ui.viewport_rect()
//     }
//
//     fn draw(&self, ui: &UI) {
//
//     }
//
//     fn keypress(&self, key: Key) -> Option<Key> {
//         None
//     }
// }

struct SetProduction {
    rect: Rect,
    loc: Location,
    selected: u8,
    done: bool
}

impl SetProduction {
    fn new(rect: Rect, loc: Location) -> Self {
        SetProduction{
            rect: rect,
            loc: loc,
            selected: 0,
            done: false
        }
    }
}

impl Component for SetProduction {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { self.done }

    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        let ref tile = game.tiles[self.loc];

        if let Some(ref city) = tile.city {
            // self.center_viewport(loc);
            write!(*stdout, "{}Set Production for {}", self.goto(0, 0), city).unwrap();

            let unit_types = UnitType::values();
            for (i, unit_type) in unit_types.iter().enumerate() {
                write!(*stdout, "{}{} - {}",
                    self.goto(1, i as u16 + 2),
                    unit_type.key(),
                    // if self.selected==i as u8 { "+" } else { "-" },
                    unit_type.name()).unwrap();

                // write!(*stdout, "{}Enter to accept", self.goto(0, unit_types.len() as u16 + 3)).unwrap();
            }
        }
    }

    fn keypress(&mut self, key: &Key, game: &mut Game) {
        if let Key::Char(c) = *key {
            if let Some(unit_type) = UnitType::from_key(&c) {
                game.set_production(&self.loc, &unit_type);
                self.done = true;
            }
        }
    }
}

struct CurrentPlayer {
    rect: Rect,
    player: Option<PlayerNum>
}

impl CurrentPlayer {
    fn new(rect: Rect, player: Option<PlayerNum>) -> Self {
        CurrentPlayer {
            rect: rect,
            player: player
        }
    }
}

impl Component for CurrentPlayer {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }

    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        write!(*stdout,
            "{}Current Player: {}",
            self.goto(0, 0),
            if let Some(player) = self.player { player.to_string() } else { "None".to_string() }
        ).unwrap();
    }

    fn keypress(&mut self, key: &Key, game: &mut Game) {
        // do nothing
    }
}

pub struct UI<'a> {
    game: Game,
    mode: Mode,
    stdout: termion::raw::RawTerminal<StdoutLock<'a>>,
    term_dims: Dims,
    header_height: u16,
    h_scrollbar_height: u16,
    v_scrollbar_width: u16,
    viewport_size: ViewportSize,
    // viewport_rect: Rect,
    viewport_offset: Vec2d<u16>,
    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>,

    components: Vec<Box<Component>>
}

enum ViewportSize {
    REGULAR,
    THEATER,
    FULLSCREEN
}

fn nonnegative_mod(x: i32, max: u16) -> u16 {
    let mut result = x;

    while result < 0 {
        result += max as i32;
    }

    return (result % max as i32) as u16;
}

impl<'b> UI<'b> {
    pub fn new(
        game: Game,
        stdout: termion::raw::RawTerminal<StdoutLock<'b>>,
        term_dims: Dims, header_height: u16
    ) -> Self {

        let h_scrollbar_height = 1;
        let v_scrollbar_width = 1;

        let offset = Vec2d{ x: game.map_dims.width/2, y: game.map_dims.height/2 };

        let mut ui = UI {
            game: game,
            mode: Mode::General,
            stdout: stdout,
            term_dims: term_dims,
            header_height: header_height,
            h_scrollbar_height: h_scrollbar_height,
            v_scrollbar_width: v_scrollbar_width,
            viewport_size: ViewportSize::REGULAR,
            // viewport_rect: viewport_rect(&ViewportSize::REGULAR, header_height, h_scrollbar_height, v_scrollbar_width, &term_dims),
            viewport_offset: offset,
            old_h_scroll_x: None,
            old_v_scroll_y: None,

            components: Vec::new()
        };

        let cp_rect = ui.current_player_rect();

        ui.components.push(Box::new(CurrentPlayer::new(cp_rect, None)));

        ui
    }

    fn take_input(&mut self) {
        let stdin = stdin();
        for c in stdin.keys() {
            let c = c.unwrap();

            let mut component_is_done = false;

            if let Some(mut component) = self.components.last_mut() {
                component.keypress(&c, &mut self.game);
                component_is_done |= component.is_done();

                if component_is_done {
                    component.clear(&mut self.stdout);
                }
            }

            if component_is_done {
                self.components.pop();
            }

            match c {
                Key::Char(conf::KEY_VIEWPORT_SHIFT_LEFT)       => self.shift_viewport(Vec2d{x:-1, y: 0}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_RIGHT)      => self.shift_viewport(Vec2d{x: 1, y: 0}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP)         => self.shift_viewport(Vec2d{x: 0, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN)       => self.shift_viewport(Vec2d{x: 0, y: 1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_LEFT)    => self.shift_viewport(Vec2d{x:-1, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_RIGHT)   => self.shift_viewport(Vec2d{x: 1, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_LEFT)  => self.shift_viewport(Vec2d{x:-1, y: 1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_RIGHT) => self.shift_viewport(Vec2d{x: 1, y: 1}),
                Key::Char(conf::KEY_QUIT) => self.quit(),
                Key::Char(conf::KEY_VIEWPORT_SIZE_ROTATE) => {
                    let new_size = match self.viewport_size {
                        ViewportSize::REGULAR => ViewportSize::THEATER,
                        ViewportSize::THEATER => ViewportSize::FULLSCREEN,
                        ViewportSize::FULLSCREEN => ViewportSize::REGULAR
                    };

                    self.set_viewport_size(new_size);
                }
                _ => {}
            }
        }
    }

    pub fn run(&mut self) {
        // loop through endless game turns
        loop {
            let player_num = self.game.begin_next_player_turn();

            // Process production set requests
            loop {
                if self.game.production_set_requests().len() < 1 {
                    break;
                }

                let loc = *self.game.production_set_requests().iter().next().unwrap();

                self.center_viewport(&loc);

                self.mode = Mode::SetProduction{loc:loc};
                let viewport_rect = self.viewport_rect();
                self.components.push(Box::new(SetProduction::new(
                    Rect{
                        left: viewport_rect.width + self.v_scrollbar_width + 1,
                        top: self.header_height + 1,
                        width: self.term_dims.width - viewport_rect.width - 2,
                        height: self.term_dims.height - self.header_height
                    },
                    loc)
                ));

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

        self.draw_map();
        self.draw_scroll_bars();

        for component in self.components.iter_mut() {
            component.draw(&self.game, &mut self.stdout);
        }

        write!(self.stdout, "{}Turn: {}", goto(0, 45), self.game.turn).unwrap();

        write!(self.stdout, "{}{}", termion::style::Reset, termion::cursor::Hide).unwrap();
        self.stdout.flush().unwrap();
    }

    fn viewport_to_map_coords(&self, viewport_offset: Vec2d<u16>, viewport_x: &u16, viewport_y: &u16) -> Location {
        let map_x:u16 = (viewport_x + viewport_offset.x) % self.game.map_dims.width;// mod implements wrapping
        let map_y:u16 = (viewport_y + viewport_offset.y) % self.game.map_dims.height;// mod implements wrapping
        Location{
            x: map_x,
            y: map_y
        }
    }

    fn draw_map(&mut self) {
        let viewport_rect = self.viewport_rect();
        for viewport_x in 0_u16..viewport_rect.width {
            for viewport_y in 0_u16..(viewport_rect.height+1) {
                let map_location = self.viewport_to_map_coords(self.viewport_offset, &viewport_x, &viewport_y);

                self.draw_tile(map_location, viewport_x, viewport_y);
            }
        }
    }

    fn draw_tile(&mut self, tile_loc: Location, viewport_x: u16, viewport_y: u16) {
        let tile = &self.game.tiles[tile_loc];

        if tile.loc.y == self.game.map_dims.height - 1 {
            write!(self.stdout, "{}", termion::style::Underline).unwrap();
        }

        if let Some(fg_color) = tile.fg_color() {
            write!(self.stdout, "{}", Fg(fg_color)).unwrap();
        }

        let viewport_rect = self.viewport_rect();
        write!(self.stdout, "{}{}{}{}",
            goto(viewport_x + viewport_rect.left, viewport_y + viewport_rect.top),
            Bg(tile.bg_color()),
            tile.sym(),
            termion::style::Reset
        ).unwrap();
    }

    /// Update the map to reflect the current viewport offset
    fn update_map(&mut self, old_viewport_offset: Vec2d<u16>, new_viewport_offset: Vec2d<u16>) {
        let viewport_rect = self.viewport_rect();
        for viewport_x in 0_u16..viewport_rect.width {
            for viewport_y in 0_u16..(viewport_rect.height+1) {
                let old_map_loc = self.viewport_to_map_coords(old_viewport_offset, &viewport_x, &viewport_y);
                let new_map_loc = self.viewport_to_map_coords(new_viewport_offset, &viewport_x, &viewport_y);

                let should_draw_tile = {
                    let old_tile = &self.game.tiles[old_map_loc];
                    let new_tile = &self.game.tiles[new_map_loc];

                    let redraw_for_border =
                    old_map_loc.y != new_map_loc.y && (
                        old_map_loc.y == self.game.map_dims.height - 1 ||
                        new_map_loc.y == self.game.map_dims.height - 1
                    );

                    let redraw_for_mismatch = !(
                        old_tile.terrain==new_tile.terrain &&
                        old_tile.sym() == new_tile.sym() &&
                        old_tile.alignment() == new_tile.alignment()
                    );

                    redraw_for_border || redraw_for_mismatch
                };

                if should_draw_tile {
                    self.draw_tile(new_map_loc, viewport_x, viewport_y);
                }

            }
        }

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
        let v_scroll_y: u16 = self.header_height + (viewport_rect.height as f32 * (self.viewport_offset.y as f32 / self.game.map_dims.height as f32)) as u16;

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



    pub fn shift_viewport(&mut self, shift: Vec2d<i32>) {
        let mut new_x_offset:i32 = ( self.viewport_offset.x as i32 ) + shift.x;
        let mut new_y_offset:i32 = ( self.viewport_offset.y as i32 ) + shift.y;

        while new_x_offset < 0 {
            new_x_offset += self.game.map_dims.width as i32;
        }
        while new_y_offset < 0 {
            new_y_offset += self.game.map_dims.height as i32;
        }

        let new_viewport_offset = Vec2d{
            x: (new_x_offset as u16) % self.game.map_dims.width,
            y: (new_y_offset as u16) % self.game.map_dims.height
        };

        self.set_viewport_offset(new_viewport_offset);

        // self.update_map(self.viewport_offset, new_viewport_offset);
        //
        // // self.viewport_offset.x = (new_x_offset as u16) % self.game.map_dims.width;
        // // self.viewport_offset.y = (new_y_offset as u16) % self.game.map_dims.height;
        //
        // self.viewport_offset = new_viewport_offset;
        //
        // // self.draw_map();
        // self.draw_scroll_bars();
    }

    fn set_viewport_offset(&mut self, new_viewport_offset: Vec2d<u16>) {
        let old_viewport_offset = self.viewport_offset;
        self.update_map(old_viewport_offset, new_viewport_offset);
        self.viewport_offset = new_viewport_offset;
        self.draw_scroll_bars();
    }

    pub fn center_viewport(&mut self, map_location: &Location) {
        let viewport_rect = self.viewport_rect();

        let new_viewport_offset = Vec2d{
            x: nonnegative_mod(
                map_location.x as i32 - (viewport_rect.width as i32 / 2),
                self.game.map_dims.width
            ),
            y: nonnegative_mod(
                map_location.y as i32 - (viewport_rect.height as i32 / 2),
                self.game.map_dims.height
            )
        };

        self.set_viewport_offset(new_viewport_offset);
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
        match self.viewport_size {
            ViewportSize::REGULAR => Rect {
                left: 0,
                top: self.header_height,
                width: (self.term_dims.width - self.v_scrollbar_width) / 2,
                height: 25
            },
            ViewportSize::THEATER => Rect {
                left: 0,
                top: self.header_height,
                width: self.term_dims.width - self.v_scrollbar_width,
                height: 25
            },
            ViewportSize::FULLSCREEN => Rect {
                left: 0,
                top: 0,
                width: self.term_dims.width - self.v_scrollbar_width,
                height: self.term_dims.height - self.h_scrollbar_height - 1
            }
        }
    }

    fn log_area_rect(&self) -> Rect {
        let vp = self.viewport_rect();
        Rect {
            left: 0,
            top: vp.top - vp.height,
            width: vp.width,
            height: self.term_dims.height - vp.height
        }
    }

    fn sidebar_rect(&self) -> Rect {
        let vp = self.viewport_rect();
        Rect {
            left: vp.right() + 1,
            top: vp.top,
            width: self.term_dims.width - vp.left - vp.width,
            height: self.term_dims.height
        }
    }

    fn current_player_rect(&self) -> Rect {
        let vp = self.viewport_rect();
        Rect {
            left: 0,
            top: vp.top + vp.height + 2,
            width: vp.width,
            height: 1
        }
    }
}
