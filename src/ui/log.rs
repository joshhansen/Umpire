use std::collections::VecDeque;
use std::io::{StdoutLock,Write};

use termion::event::Key;
use termion::raw::RawTerminal;
use termion::style::{Reset,Underline};

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

    pub fn redraw_lite(&self, stdout: &mut RawTerminal<StdoutLock>) {
        self.draw_lite(stdout);
    }

    fn draw_lite(&self, stdout: &mut RawTerminal<StdoutLock>) {
        write!(*stdout,
            "{}{}Message Log{}",
            self.goto(0, 0),
            Underline,
            Reset
        ).unwrap();

        for i in 0..self.rect.height() {
            let empty = String::from("");
            let message = self.messages.get(i as usize).unwrap_or(&empty);
            let spaces = (0..(self.rect.width - 2)).map(|_| " ").collect::<String>();
            write!(*stdout, "{}┃ {}{}", self.goto(0, i as u16+1), message, spaces).unwrap();
        }

        stdout.flush().unwrap();
    }
}

impl Draw for LogArea {
    fn draw(&self, _game: &Game, stdout: &mut RawTerminal<StdoutLock>) {
        self.draw_lite(stdout);
    }
}

impl Keypress for LogArea {
    fn keypress(&mut self, _key: &Key, _game: &mut Game) {
        // do nothing
    }
}

impl Redraw for LogArea {
    fn redraw(&self, _game: &Game, stdout: &mut RawTerminal<StdoutLock>) {
        self.redraw_lite(stdout);
    }
}

impl Component for LogArea {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}
