extern crate termion;

use std::io::{StdoutLock,Write};

use termion::event::Key;


use game::Game;
use ui::{Component,Draw,Keypress,Redraw};
use util::Rect;

pub struct LogArea {
    rect: Rect,
    messages: Vec<String>
}

impl LogArea {
    pub fn new(rect: &Rect) -> Self {
        LogArea{ rect: *rect, messages: Vec::new() }
    }

    pub fn log_message(&mut self, message: String) {
        self.messages.push(message);
    }
}

impl Draw for LogArea {
    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        write!(*stdout,
            "{}{}Message Log{}",
            self.goto(0, 0),
            termion::style::Underline,
            termion::style::Reset
        ).unwrap();

        for i in 0..self.rect.height() {
            write!(*stdout, "{}┃", self.goto(0, i as u16+1)).unwrap();
        }

        for (i, message) in self.messages.iter().enumerate() {
            write!(*stdout, "{}{}", self.goto(2, i as u16+1), message).unwrap();
        }
    }
}

impl Keypress for LogArea {
    fn keypress(&mut self, key: &Key, game: &mut Game) {
        // do nothing
    }
}

impl Redraw for LogArea {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        self.clear(stdout);
        self.draw(game, stdout);
    }
}

impl Component for LogArea {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}
