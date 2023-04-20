use std::io::{Result as IoResult, Stdout};

use async_trait::async_trait;

use crossterm::{
    queue,
    style::{Attribute, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
};

use common::{
    colors::Colors,
    game::player::PlayerTurn,
    util::{Rect, Vec2d},
};

use crate::{color::Palette, Component, Draw};

pub trait ScrollableComponent: Component {
    fn offset(&self) -> Vec2d<u16>;
    fn scroll_relative<V: Into<Vec2d<i32>>>(&mut self, offset: V);
}

pub struct Scroller<S: ScrollableComponent + Send> {
    rect: Rect,
    pub scrollable: S,
    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>,
}

impl<S: ScrollableComponent + Send> Scroller<S> {
    pub fn new(rect: Rect, scrollable: S) -> Self {
        Scroller {
            rect,
            scrollable,
            old_h_scroll_x: None,
            old_v_scroll_y: None,
        }
    }

    fn h_scroll_x(&self, map_width: u16) -> u16 {
        (f32::from(self.rect.width - 1)
            * (f32::from(self.scrollable.offset().x) / f32::from(map_width))) as u16
    }

    fn v_scroll_y(&self, map_height: u16) -> u16 {
        (f32::from(self.rect.height)
            * (f32::from(self.scrollable.offset().y) / f32::from(map_height))) as u16
    }

    fn draw_scroll_bars(
        &mut self,
        game: &PlayerTurn<'_>,
        stdout: &mut Stdout,
        palette: &Palette,
    ) -> IoResult<()> {
        let dims = game.dims();
        let viewport_rect = self.scrollable.rect();
        let h_scroll_x: u16 = self.h_scroll_x(dims.width);
        let h_scroll_y = viewport_rect.bottom();

        if self.old_h_scroll_x != Some(h_scroll_x) {
            if let Some(old_h_scroll_x) = self.old_h_scroll_x {
                self.erase(stdout, old_h_scroll_x, h_scroll_y, palette)?;
            }
            self.draw_scroll_mark(stdout, h_scroll_x, h_scroll_y, '^', palette)?;

            self.old_h_scroll_x = Some(h_scroll_x);
        }

        let v_scroll_x = viewport_rect.right();
        let v_scroll_y: u16 = self.v_scroll_y(dims.height);

        if self.old_v_scroll_y != Some(v_scroll_y) {
            if let Some(old_v_scroll_y) = self.old_v_scroll_y {
                self.erase(stdout, v_scroll_x, old_v_scroll_y, palette)?;
            }
            self.draw_scroll_mark(stdout, v_scroll_x, v_scroll_y, '<', palette)?;

            self.old_v_scroll_y = Some(v_scroll_y);
        }

        Ok(())
    }

    /// Utility method
    fn draw_scroll_mark(
        &self,
        stdout: &mut Stdout,
        x: u16,
        y: u16,
        sym: char,
        palette: &Palette,
    ) -> IoResult<()> {
        // write!(*stdout, "{}{}{}{}",
        //     StrongReset::new(palette),
        //     self.goto(x,y),
        //     Fg(palette.get_single(Colors::ScrollMarks)),
        //     sym
        // ).unwrap();
        queue!(
            *stdout,
            self.goto(x, y),
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(palette.get_single(Colors::Background)),
            SetForegroundColor(palette.get_single(Colors::ScrollMarks)),
            Print(sym.to_string())
        )
    }

    /// Utility method
    fn erase(&self, stdout: &mut Stdout, x: u16, y: u16, palette: &Palette) -> IoResult<()> {
        // write!(*stdout, "{}{} ",
        //     StrongReset::new(palette),
        //     self.goto(x,y)
        // ).unwrap();
        queue!(
            *stdout,
            self.goto(x, y),
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(palette.get_single(Colors::Background)),
            Print(String::from(" "))
        )
    }

    // pub fn viewport_dims(&self) -> Dims {
    //     Dims {
    //         width: self.rect.width - 1,
    //         height: self.rect.height - 1
    //     }
    // }
}

#[async_trait]
impl<S: ScrollableComponent + Send> Draw for Scroller<S> {
    async fn draw_no_flush(
        &mut self,
        game: &PlayerTurn<'_>,
        stdout: &mut Stdout,
        palette: &Palette,
    ) -> IoResult<()> {
        self.draw_scroll_bars(game, stdout, palette)?;
        self.scrollable.draw_no_flush(game, stdout, palette).await
    }
}

impl<S: ScrollableComponent + Send> Component for Scroller<S> {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.scrollable.set_rect(rect);
    }

    fn rect(&self) -> Rect {
        self.rect
    }
}
