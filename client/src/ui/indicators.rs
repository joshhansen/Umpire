use std::io::{Result as IoResult, Stdout};

use async_trait::async_trait;

use crossterm::{
    queue,
    style::{style, PrintStyledContent},
};

use common::{game::player::PlayerTurnControl, util::Rect};

use umpire_tui::{color::Palette, Component, Draw};

pub struct CurrentPlayer {
    rect: Rect,
}

impl CurrentPlayer {
    pub fn new(rect: Rect) -> Self {
        CurrentPlayer { rect }
    }
}

#[async_trait]
impl Draw for CurrentPlayer {
    async fn draw_no_flush(
        &mut self,
        game: &PlayerTurnControl,
        stdout: &mut Stdout,
        _palette: &Palette,
    ) -> IoResult<()> {
        // write!(*stdout,
        //     "{}Current Player: {}  ",
        //     self.goto(0, 0),
        //     game.current_player()
        // ).unwrap();
        queue!(
            *stdout,
            self.goto(0, 0),
            PrintStyledContent(style(format!(
                "Current Player: {}  ",
                game.current_player()
            )))
        )
    }
}

impl Component for CurrentPlayer {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn is_done(&self) -> bool {
        false
    }
}

pub struct Turn {
    rect: Rect,
}

impl Turn {
    pub fn new(rect: Rect) -> Self {
        Turn { rect }
    }
}

#[async_trait]
impl Draw for Turn {
    async fn draw_no_flush(
        &mut self,
        game: &PlayerTurnControl,
        stdout: &mut Stdout,
        _palette: &Palette,
    ) -> IoResult<()> {
        // write!(*stdout, "{}Turn: {}", self.goto(0, 0), game.turn()).unwrap();
        queue!(
            *stdout,
            self.goto(0, 0),
            PrintStyledContent(style(format!("Turn: {}", game.turn())))
        )
    }
}

impl Component for Turn {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn is_done(&self) -> bool {
        false
    }
}
