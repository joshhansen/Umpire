//!
//! A text-based user interface implemented for Unix terminals.
//!
//! The abstract game logic is implemented in `game::Game`. This user interface references that
//! game engine but is otherwise independent in realizing a user experience around the game.

use std::io::{Write,stdout};
use std::rc::Rc;

use termion;
use termion::clear;
use termion::color::{Bg,Color,Rgb};
use termion::raw::IntoRawMode;
use termion::screen::{AlternateScreen,ToMainScreen};

use crate::{
    color::{Colors,Palette},
    conf::{
        self,
        HEADER_HEIGHT,
    },
    game::{
        Game,
        MoveResult,
        obs::{Observer,visible_coords_iter},
        unit::combat::{CombatCapable,CombatOutcome,CombatParticipant},
    },
    log::{LogTarget,Message,MessageSource},
    ui::{
        style::StrongReset,
        sym::Sym,
    },
};


use util::{Dims,Rect,Location,sleep_millis,wrapped_add};

pub fn run<C:Color+Copy>(mut game: Game, term_dims: Dims, use_alt_screen: bool, palette: Palette<C>, unicode: bool) -> Result<(),String> {
    {//This is here so screen drops completely when the game ends. That lets us print a farewell message to a clean console.

        let mut prev_mode: Option<Mode> = None;
        let mut mode = self::mode::Mode::TurnStart;
        if use_alt_screen {//FIXME find a way to not duplicate code in both arms of this if statement
            let mut ui = TermUI::new(
                game.map_dims(),
                term_dims,
                AlternateScreen::from(stdout().into_raw_mode().unwrap()),
                palette,
                unicode,
            );

            while mode.run(&mut game, &mut ui, &mut prev_mode) {
                // nothing here
            }
        } else {
            let mut ui = TermUI::new(
                game.map_dims(),
                term_dims,
                stdout().into_raw_mode().unwrap(),
                palette,
                unicode,
            );

            while mode.run(&mut game, &mut ui, &mut prev_mode) {
                // nothing here
            }
        }
    }

println!("\n\n\tHe rules a moment: Chaos umpire sits,
\tAnd by decision more embroils the fray
\tBy which he reigns: next him, high arbiter,
\tChance governs all.

\t\t\t\tParadise Lost (2.907-910)\n");

    Ok(())
}

pub trait MoveAnimator {
    fn animate_move(&mut self, game: &Game, move_result: &MoveResult);
}

pub trait UI : LogTarget + MoveAnimator {

}

pub struct DefaultUI;

impl LogTarget for DefaultUI {
    fn log_message<T>(&mut self, message: T) where Message:From<T> {
        println!("{}", Message::from(message).text);
    }
    fn replace_message<T>(&mut self, message: T) where Message:From<T> {
        println!("\r{}", Message::from(message).text);
    }
}

impl MoveAnimator for DefaultUI {
    fn animate_move(&mut self, _game: &Game, move_result: &MoveResult) {
        println!("Moving: {:?}", *move_result);
    }
}

impl UI for DefaultUI {

}

/// 0-indexed variant of Goto
pub fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

pub trait Draw {
    fn draw<C:Color+Copy,W:Write>(&mut self, game: &Game, stdout: &mut W, palette: &Palette<C>);
}

pub trait Component : Draw {
    fn set_rect(&mut self, rect: Rect);

    fn rect(&self) -> Rect;

    fn is_done(&self) -> bool;

    fn goto(&self, x: u16, y: u16) -> termion::cursor::Goto {
        let rect = self.rect();
        goto(rect.left + x, rect.top + y)
    }

    fn clear<W:Write>(&self, stdout: &mut W) {
        let rect = self.rect();
        let blank_string = (0..rect.width).map(|_| " ").collect::<String>();
        for y in 0..rect.height {
            write!(*stdout, "{}{}", self.goto(0, y), blank_string).unwrap();
        }
    }

    // fn draw_window_frame(&self, title: &str, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
    //
    // }
}

mod scroll;

mod indicators;
pub mod log;
mod map;
pub mod mode;
// pub mod sound;
mod style;
pub mod sym;

use self::scroll::Scroller;
use self::indicators::{CurrentPlayer,Turn};
use self::log::LogArea;
use self::map::Map;
use self::mode::Mode;

enum ViewportSize {
    REGULAR,
    THEATER,
    FULLSCREEN
}

impl ViewportSize {
    fn rect(&self, term_dims: Dims) -> Rect {
        match *self {
            ViewportSize::REGULAR => Rect {
                left: 0,
                top: HEADER_HEIGHT,
                width: (term_dims.width - V_SCROLLBAR_WIDTH) / 2,
                height: 25
            },
            ViewportSize::THEATER => Rect {
                left: 0,
                top: HEADER_HEIGHT,
                width: term_dims.width - V_SCROLLBAR_WIDTH,
                height: 25
            },
            ViewportSize::FULLSCREEN => Rect {
                left: 0,
                top: 0,
                width: term_dims.width - V_SCROLLBAR_WIDTH,
                height: term_dims.height - H_SCROLLBAR_HEIGHT - 1
            }
        }
    }
}

fn current_player_rect() -> Rect {
    Rect {
        left: 10,
        top: 0,
        width: 21,
        height: 1
    }
}

fn turn_rect(current_player_rect: Rect) -> Rect {
    Rect {
        left: current_player_rect.right() + 2,
        top: 0,
        width: 11,
        height: 1
    }
}

fn log_area_rect(viewport_rect: Rect, term_dims: Dims) -> Rect {
    Rect {
        left: 0,
        top: viewport_rect.bottom() + 2,
        width: viewport_rect.width,
        height: term_dims.height - viewport_rect.height - 10
    }
}

fn sidebar_rect(viewport_rect: Rect, term_dims: Dims) -> Rect {
    // Rect {
    //     left: viewport_rect.right() + 1,
    //     top: viewport_rect.top,
    //     width: term_dims.width - viewport_rect.width,
    //     height: term_dims.height
    // }
    Rect {
        left: viewport_rect.width + V_SCROLLBAR_WIDTH + 1,
        top: HEADER_HEIGHT + 1,
        width: term_dims.width - viewport_rect.width - 2,
        height: term_dims.height - HEADER_HEIGHT
    }
}

const H_SCROLLBAR_HEIGHT: u16 = 1;
const V_SCROLLBAR_WIDTH: u16 = 1;

/// The termion-based user interface.
pub struct TermUI<C:Color+Copy,W:Write> {
    stdout: W,
    term_dims: Dims,
    viewport_size: ViewportSize,

    map_scroller: Scroller<Map<C>>,
    log: LogArea,
    current_player: CurrentPlayer,
    turn: Turn,
    first_draw: bool,
    palette: Rc<Palette<C>>,
    unicode: bool,
}

impl<C:Color+Copy,W:Write> TermUI<C,W> {
    pub fn new(
        map_dims: Dims,
        term_dims: Dims,
        stdout: W,
        palette: Palette<C>,
        unicode: bool,
    ) -> Self {
        let viewport_size = ViewportSize::REGULAR;
        let viewport_rect = viewport_size.rect(term_dims);

        let palette = Rc::new(palette);

        let map = Map::new(viewport_rect, map_dims, palette.clone(), unicode);

        let map_scroller_rect = Rect {
            left: viewport_rect.left,
            top: viewport_rect.top,
            width: viewport_rect.width + 1,
            height: viewport_rect.height + 1
        };
        let mut map_scroller = Scroller::new(map_scroller_rect, map);
        map_scroller.set_rect(viewport_rect);

        let log_rect = log_area_rect(viewport_rect, term_dims);
        let log = LogArea::new(log_rect);

        let cp_rect = current_player_rect();
        let current_player = CurrentPlayer::new(cp_rect);

        let mut ui = TermUI {
            stdout,
            term_dims,
            viewport_size,

            map_scroller,
            log,
            current_player,

            turn: Turn::new(turn_rect(cp_rect)),

            first_draw: true,
            
            palette,

            unicode
        };

        ui.clear();

        ui
    }

    fn clear(&mut self) {
        write!(self.stdout, "{}", clear::All).unwrap();

        for x in 0..self.term_dims.width {
            for y in 0..self.term_dims.height {
                write!(self.stdout, "{}{} ", goto(x,y), Bg(Rgb(0,0,0))).unwrap();
            }
        }
    }

    fn set_viewport_size(&mut self, game: &Game, viewport_size: ViewportSize) {
        self.viewport_size = viewport_size;
        self.map_scroller.set_rect(self.viewport_size.rect(self.term_dims));
        self.draw(game);
    }

    pub fn rotate_viewport_size(&mut self, game: &Game) {
        let new_size = match self.viewport_size {
            ViewportSize::REGULAR => ViewportSize::THEATER,
            ViewportSize::THEATER => ViewportSize::FULLSCREEN,
            ViewportSize::FULLSCREEN => ViewportSize::REGULAR
        };

        self.set_viewport_size(game, new_size);
        self.draw(game);
    }

    fn draw(&mut self, game: &Game) {
        if self.first_draw {
            write!(self.stdout, "{}{}{}{}",
                // termion::clear::All,
                goto(0,0),
                termion::style::Underline,
                conf::APP_NAME,
                StrongReset::new(&self.palette)
            ).unwrap();
            self.first_draw = false;
        }

        self.log.draw_lite(&mut self.stdout, &self.palette);
        self.current_player.draw(game, &mut self.stdout, &self.palette);
        self.map_scroller.draw(game, &mut self.stdout, &self.palette);
        self.turn.draw(game, &mut self.stdout, &self.palette);

        write!(self.stdout, "{}{}", StrongReset::new(&self.palette), termion::cursor::Hide).unwrap();
        self.stdout.flush().unwrap();
    }

    fn draw_unit_observations(&mut self, game: &Game, unit_loc: Location, unit_sight_distance: u16) {
        // let unit = game.unit_by_loc(unit_loc).unwrap();
        for inc in visible_coords_iter(unit_sight_distance) {
            if let Some(loc) = wrapped_add(unit_loc, inc, game.map_dims(), game.wrapping()) {

                if let Some(viewport_loc) = self.map_scroller.scrollable.map_to_viewport_coords(loc, self.viewport_rect().dims()) {
                    self.map_scroller.scrollable.draw_tile(game, &mut self.stdout, viewport_loc, false, false, None);
                }
            }
        }
    }

    fn animate_combat<A:CombatCapable+Sym,D:CombatCapable+Sym>(
        &mut self,
        game: &Game,
        outcome: &CombatOutcome<A,D>,
        attacker_loc: Location,
        defender_loc: Location) {

        let viewport_dims = self.map_scroller.viewport_dims();
        let map = &mut self.map_scroller.scrollable;

        let attacker_viewport_loc = map.map_to_viewport_coords(attacker_loc, viewport_dims);
        let defender_viewport_loc = map.map_to_viewport_coords(defender_loc, viewport_dims);
        let attacker_sym = outcome.attacker().sym(self.unicode);
        let defender_sym = outcome.defender().sym(self.unicode);

        for damage_recipient in outcome.received_damage_sequence() {
            let viewport_loc = match *damage_recipient {
                CombatParticipant::Attacker => attacker_viewport_loc,
                CombatParticipant::Defender => defender_viewport_loc
            };
            let sym = match *damage_recipient {
                CombatParticipant::Attacker => attacker_sym,
                CombatParticipant::Defender => defender_sym
            };

            if let Some(viewport_loc) = viewport_loc {
                map.draw_tile(game, &mut self.stdout, viewport_loc, true, false, Some(sym));
                sleep_millis(100);
                map.draw_tile(game, &mut self.stdout, viewport_loc, false, false, Some(sym));
            } else {
                sleep_millis(100);
            }
        }
    }

    fn viewport_rect(&self) -> Rect {
        self.viewport_size.rect(self.term_dims)
    }

    fn cleanup(&mut self) {
        write!(self.stdout, "{}", ToMainScreen).unwrap();
    }

    pub fn cursor_viewport_loc(&self, mode: &Mode, game: &Game) -> Option<Location> {
        let viewport_dims = self.map_scroller.viewport_dims();
        let map = &self.map_scroller.scrollable;

        match *mode {
            Mode::SetProduction{city_loc} => map.map_to_viewport_coords(city_loc, viewport_dims),
            Mode::GetUnitOrders{unit_id,..} => {
                let unit_loc = game.unit_loc(unit_id).unwrap();
                map.map_to_viewport_coords(unit_loc, viewport_dims)
            },
            _ => None
        }
    }

    pub fn cursor_map_loc(&self, mode: &Mode, game: &Game) -> Option<Location> {
        match *mode {
            Mode::SetProduction{city_loc} => Some(city_loc),
            Mode::GetUnitOrders{unit_id,..} => {
                let unit_loc = game.unit_loc(unit_id).unwrap();
                Some(unit_loc)
            },
            _ => None
        }
    }
}

impl <C:Color+Copy,W:Write> LogTarget for TermUI<C,W> {
    fn log_message<T>(&mut self, message: T) where Message:From<T> {
        self.log.log(Message::from(message));
        self.log.draw_lite(&mut self.stdout, &self.palette);
    }

    fn replace_message<T>(&mut self, message: T) where Message:From<T> {
        self.log.replace(Message::from(message));
        self.log.draw_lite(&mut self.stdout, &self.palette);
    }
}

impl <C:Color+Copy,W:Write> MoveAnimator for TermUI<C,W> {
    fn animate_move(&mut self, game: &Game, move_result: &MoveResult) {
        let mut current_loc = move_result.starting_loc();

        for move_ in move_result.moves() {
            let target_loc = move_.loc();

            let mut was_combat = false;
            if let Some(ref combat) = *move_.unit_combat() {
                self.animate_combat(game, combat, current_loc, target_loc);
                was_combat = true;
            }

            if let Some(ref combat) = *move_.city_combat() {
                self.animate_combat(game, combat, current_loc, target_loc);
                was_combat = true;
            }

            self.log_message(Message {
                text: format!("Unit {} {}", move_result.unit(), if move_.moved_successfully() {
                    if was_combat {"victorious"} else {"moved successfully"}
                } else {"destroyed"}),
                mark: Some('*'),
                fg_color: Some(Colors::Combat),
                bg_color: None,
                source: Some(MessageSource::UI)
            });

            {
                let viewport_dims = self.map_scroller.viewport_dims();
                let map = &mut self.map_scroller.scrollable;

                // Erase the unit's symbol at its old location
                if let Some(current_viewport_loc) = map.map_to_viewport_coords(current_loc, viewport_dims) {
                    map.draw_tile(game, &mut self.stdout, current_viewport_loc, false, false, None);//By now the model has no unit in the old location, so just draw that tile as per usual
                }
            }

            if move_.moved_successfully() {
                self.draw_unit_observations(game, target_loc, move_result.unit().sight_distance());
            }

            current_loc = target_loc;

            self.stdout.flush().unwrap();
        }

        if move_result.unit().moves_remaining() == 0 {
            sleep_millis(250);
        }
    }
}
