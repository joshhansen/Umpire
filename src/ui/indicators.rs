use std::io::Write;

use termion::event::Key;

use game::Game;
use ui::{Component,Draw,Keypress,Redraw};
use util::Rect;

pub struct CurrentPlayer {
    rect: Rect
}

impl CurrentPlayer {
    pub fn new(rect: Rect) -> Self {
        CurrentPlayer {
            rect: rect
        }
    }
}

impl Draw for CurrentPlayer {
    fn draw<W:Write>(&self, game: &Game, stdout: &mut W) {
        write!(*stdout,
            "{}Current Player: {}  ",
            self.goto(0, 0),
            game.current_player()
        ).unwrap();
    }
}

impl Keypress for CurrentPlayer {
    fn keypress(&mut self, _key: &Key, _game: &mut Game) {
        // do nothing
    }
}

impl Redraw for CurrentPlayer {
    fn redraw<W:Write>(&self, game: &Game, stdout: &mut W) {
        self.clear(stdout);
        self.draw(game, stdout);
    }
}

impl Component for CurrentPlayer {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}

pub struct Turn {
    rect: Rect
}

impl Turn {
    pub fn new(rect: &Rect) -> Self {
        Turn{ rect: *rect }
    }
}

impl Draw for Turn {
    fn draw<W:Write>(&self, game: &Game, stdout: &mut W) {
        write!(*stdout, "{}Turn: {}", self.goto(0, 0), game.turn()).unwrap();
    }
}

impl Redraw for Turn {
    fn redraw<W:Write>(&self, game: &Game, stdout: &mut W) {
        self.draw(game, stdout);
    }
}

impl Keypress for Turn {
    fn keypress(&mut self, _key: &Key, _game: &mut Game) {
        // do nothing
    }
}

impl Component for Turn {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}
