extern crate termion;

use std::io::{StdoutLock,Write};

use termion::event::Key;

use game::Game;
use ui::{Component,Draw,Keypress,Redraw};
use unit::{Named,UnitType};
use util::{Location,Rect};

pub struct SetProduction {
    rect: Rect,
    loc: Location,
    selected: u8,
    done: bool
}

impl SetProduction {
    pub fn new(rect: Rect, loc: Location) -> Self {
        SetProduction{
            rect: rect,
            loc: loc,
            selected: 0,
            done: false
        }
    }
}

impl Draw for SetProduction {
    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        let ref tile = game.tiles[self.loc];

        if let Some(ref city) = tile.city {
            // self.center_viewport(loc);
            write!(*stdout, "{}Set Production for {}", self.goto(0, 0), city).unwrap();

            let unit_types = UnitType::values();
            for (i, unit_type) in unit_types.iter().enumerate() {
                write!(*stdout, "{}{} - {}",
                    self.goto(1, i as u16 + 2),
                    unit_type.key(),
                    // if self.selected==i as u8 { "+" } else { "-" },
                    unit_type.name()).unwrap();

                // write!(*stdout, "{}Enter to accept", self.goto(0, unit_types.len() as u16 + 3)).unwrap();
            }
        }
    }
}

impl Redraw for SetProduction {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        self.clear(stdout);
        self.draw(game, stdout);
    }
}

impl Keypress for SetProduction {
    fn keypress(&mut self, key: &Key, game: &mut Game) {
        if let Key::Char(c) = *key {
            if let Some(unit_type) = UnitType::from_key(&c) {
                game.set_production(&self.loc, &unit_type);
                self.done = true;
            }
        }
    }
}

impl Component for SetProduction {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { self.done }
}
