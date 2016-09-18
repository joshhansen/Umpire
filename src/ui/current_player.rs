extern crate termion;

use std::io::{StdoutLock,Write};

use termion::event::Key;


use game::Game;
use ui::{Component,Draw,Keypress,Redraw};
use unit::PlayerNum;
use util::Rect;

pub struct CurrentPlayer {
    rect: Rect,
    pub player: Option<PlayerNum>
}

impl CurrentPlayer {
    pub fn new(rect: Rect, player: Option<PlayerNum>) -> Self {
        CurrentPlayer {
            rect: rect,
            player: player
        }
    }

    pub fn set_player(&mut self, player_num: PlayerNum) {
        self.player = Some(player_num);
    }
}

impl Draw for CurrentPlayer {
    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        write!(*stdout,
            "{}Current Player: {}",
            self.goto(0, 0),
            if let Some(player) = self.player { player.to_string() } else { "None".to_string() }
        ).unwrap();
    }
}

impl Keypress for CurrentPlayer {
    fn keypress(&mut self, key: &Key, game: &mut Game) {
        // do nothing
    }
}

impl Redraw for CurrentPlayer {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
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
