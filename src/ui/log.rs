extern crate termion;

use std::collections::VecDeque;
use std::io::{StdoutLock,Write};

use termion::event::Key;


use game::Game;
use ui::{Component,Draw,Keypress,Redraw};
use util::Rect;

pub struct LogArea {
    rect: Rect,
    messages: VecDeque<String>
}

impl LogArea {
    pub fn new(rect: &Rect) -> Self {
        LogArea{ rect: *rect, messages: VecDeque::new() }
    }

    fn max_messages(&self) -> u16 {
        self.rect.height() - 1
    }

    pub fn log_message(&mut self, message: String) {
        self.messages.push_back(message);
        if self.messages.len() > self.max_messages() as usize {
            self.messages.pop_front();
        }
    }
}

impl Draw for LogArea {
    fn draw(&self, _game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
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
    fn keypress(&mut self, _key: &Key, _game: &mut Game) {
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
