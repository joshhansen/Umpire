use std::io::Write;

use termion::color::{Fg, AnsiValue};

use game::Game;
use ui::{Component,Draw};
use ui::style::StrongReset;
use util::{Dims,Rect,Vec2d};

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
    pub fn new(rect: Rect, scrollable: C) -> Self {
        Scroller {
            rect,
            scrollable,
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

    fn draw_scroll_bars<W:Write>(&mut self, game: &Game, stdout: &mut W) {
        let viewport_rect = self.scrollable.rect();
        let h_scroll_x: u16 = self.h_scroll_x(game.map_dims().width);
        let h_scroll_y = viewport_rect.bottom();

        if self.old_h_scroll_x != Some(h_scroll_x) {
            if let Some(old_h_scroll_x) = self.old_h_scroll_x {
                self.erase(stdout, old_h_scroll_x, h_scroll_y);
            }
            self.draw_scroll_mark(stdout, h_scroll_x, h_scroll_y, '^');

            self.old_h_scroll_x = Some(h_scroll_x);
        }

        let v_scroll_x = viewport_rect.right();
        let v_scroll_y: u16 = self.v_scroll_y(game.map_dims().height);

        if self.old_v_scroll_y != Some(v_scroll_y) {
            if let Some(old_v_scroll_y) = self.old_v_scroll_y {
                self.erase(stdout, v_scroll_x, old_v_scroll_y);
            }
            self.draw_scroll_mark(stdout, v_scroll_x, v_scroll_y, '<');

            self.old_v_scroll_y = Some(v_scroll_y);
        }
    }

    /// Utility method
    fn draw_scroll_mark<W:Write>(&self, stdout: &mut W, x: u16, y: u16, sym: char) {
        write!(*stdout, "{}{}{}{}", StrongReset, self.goto(x,y), Fg(AnsiValue(11)), sym).unwrap();
    }

    /// Utility method
    fn erase<W:Write>(&self, stdout: &mut W, x: u16, y: u16) {
        write!(*stdout, "{}{} ", StrongReset, self.goto(x,y)).unwrap();
    }

    pub fn viewport_dims(&self) -> Dims {
        Dims {
            width: self.rect.width - 1,
            height: self.rect.height - 1
        }
    }
}

impl<C:ScrollableComponent> Draw for Scroller<C> {
    fn draw<W:Write>(&mut self, game: &Game, stdout: &mut W) {
        self.draw_scroll_bars(game, stdout);
        self.scrollable.draw(game, stdout);
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
