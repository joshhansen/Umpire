//! Shared text UI elements

use std::io::{Result as IoResult, Stdout, Write};

use color::Palette;
use common::{game::PlayerTurnControl, util::Rect};
use crossterm::{cursor::MoveTo, queue, style::Print};

pub mod color;
pub mod map;
pub mod obs_draw;
pub mod scroll;
pub mod sym;
pub mod tile;

pub trait Draw {
    fn draw(
        &mut self,
        game: &PlayerTurnControl,
        stdout: &mut Stdout,
        palette: &Palette,
    ) -> IoResult<()> {
        self.draw_no_flush(game, stdout, palette)?;
        stdout.flush()
    }
    fn draw_no_flush(
        &mut self,
        game: &PlayerTurnControl,
        stdout: &mut Stdout,
        palette: &Palette,
    ) -> IoResult<()>;
}

pub trait Component: Draw {
    fn set_rect(&mut self, rect: Rect);

    fn rect(&self) -> Rect;

    fn is_done(&self) -> bool;

    // fn goto(&self, x: u16, y: u16) -> termion::cursor::Goto {
    //     let rect = self.rect();
    //     goto(rect.left + x, rect.top + y)
    // }

    fn goto(&self, x: u16, y: u16) -> MoveTo {
        let rect = self.rect();
        MoveTo(rect.left + x, rect.top + y)
    }

    fn clear(&self, stdout: &mut Stdout) {
        let rect = self.rect();
        let blank_string = (0..rect.width).map(|_| " ").collect::<String>();
        for y in 0..rect.height {
            // write!(*stdout, "{}{}", self.goto(0, y), blank_string).unwrap();
            queue!(*stdout, self.goto(0, y), Print(blank_string.clone())).unwrap();
            //FIXME clear component without cloning a bunch of strings
        }
    }

    // fn draw_window_frame(&self, title: &str, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
    //
    // }
}
