use std::io::{StdoutLock,Write};

use termion;
use termion::color::{Fg, AnsiValue};
use termion::event::Key;

use game::Game;
use ui::{Component,Draw,Keypress,Redraw};
use util::{Dims,Direction,Rect,Vec2d};

pub trait ScrollableComponent : Component {
    fn offset(&self) -> Vec2d<u16>;
    fn scroll_relative(&mut self, offset: Vec2d<i32>);
}

pub struct Scroller<C:ScrollableComponent> {
    rect: Rect,
    pub scrollable: C,
    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>
}

impl<C:ScrollableComponent> Scroller<C> {
    pub fn new(rect: &Rect, scrollable: C) -> Self {
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
        let h_scroll_x: u16 = self.h_scroll_x(game.map_dims().width);
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
        let v_scroll_y: u16 = self.v_scroll_y(game.map_dims().height);

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
        self.old_h_scroll_x = Some(self.h_scroll_x(game.map_dims().width));
        self.old_v_scroll_y = Some(self.v_scroll_y(game.map_dims().height));
        self.scrollable.scroll_relative(offset);

    }

    pub fn viewport_dims(&self) -> Dims {
        Dims {
            width: self.rect.width - 1,
            height: self.rect.height - 1
        }
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
        if let Key::Char(c) = *key {
            if let Ok(dir) = Direction::try_from_viewport_shift(c) {
                self.scroll_relative(game, dir.vec2d())
            }
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
