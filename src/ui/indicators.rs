use std::io::{Stdout,Write};

use crossterm::{
    style::{
        PrintStyledContent,
        style,
    },
    queue
};

use crate::{
    color::Palette,
    game::player::PlayerTurnControl,
    ui::{Component,Draw},
    util::Rect
};

pub struct CurrentPlayer {
    rect: Rect
}

impl CurrentPlayer {
    pub fn new(rect: Rect) -> Self {
        CurrentPlayer {
            rect
        }
    }
}

impl Draw for CurrentPlayer {
    fn draw_no_flush(&mut self, game: &PlayerTurnControl, stdout: &mut Stdout, _palette: &Palette) {
        // write!(*stdout,
        //     "{}Current Player: {}  ",
        //     self.goto(0, 0),
        //     game.current_player()
        // ).unwrap();
        queue!(*stdout, self.goto(0, 0), PrintStyledContent(style(format!("Current Player: {}  ", game.current_player())))).unwrap();
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
    pub fn new(rect: Rect) -> Self {
        Turn{ rect }
    }
}

impl Draw for Turn {
    fn draw_no_flush(&mut self, game: &PlayerTurnControl, stdout: &mut Stdout, _palette: &Palette) {
        // write!(*stdout, "{}Turn: {}", self.goto(0, 0), game.turn()).unwrap();
        queue!(*stdout, self.goto(0, 0), PrintStyledContent(style(format!("Turn: {}", game.turn())))).unwrap();
    }
}

impl Component for Turn {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}
