//!
//! A text-based user interface implemented for Unix terminals.
//!
//! The abstract game logic is implemented in `game::Game`. This user interface references that
//! game engine but is otherwise independent in realizing a user experience around the game.

use std::{
    io::{Stdout,Write,stdout},
    rc::Rc,
    sync::mpsc::{
        channel,
        Receiver,
        Sender,
    },
    thread,
};

use crossterm::{
    AlternateScreen,
    Attribute,
    Clear,
    ClearType,
    Hide,
    InputEvent,
    KeyEvent,
    Goto,
    Output,
    RawScreen,
    SetAttr,
    SetBg,
    input,
    queue,
    terminal,
};

use crate::{
    color::{Colors,Palette},
    conf::{
        self,
        HEADER_HEIGHT,
    },
    game::{
        Game,
        ProposedAction,
        combat::{CombatCapable,CombatOutcome,CombatParticipant},
        move_::{
            Move,
            ProposedMove,
        },
        obs::{LocatedObs,Obs},
    },
    log::{LogTarget,Message,MessageSource},
    util::{Dims,Rect,Location,sleep_millis}
};

use self::{
    audio::{
        play_sounds,
        Sounds,
    },
    buf::RectBuffer,
    sym::Sym,
};

pub fn run(mut game: Game, use_alt_screen: bool, palette: Palette, unicode: bool, quiet: bool,
    confirm_turn_end: bool
) -> Result<(),String> {
    {//This is here so screen drops completely when the game ends. That lets us print a farewell message to a clean console.

        

        let _alt_screen_maybe: Option<AlternateScreen> = if use_alt_screen {
            Some(
                AlternateScreen::to_alternate(true)
                .map_err(|err| format!("Error obtaining alternate screen in raw mode: {}", err))?
            )
        } else {
            None
        };

        // The input thread
        let (input_thread_tx, input_thread_rx) = channel();
        let _input_thread_handle = thread::Builder::new().name("input".to_string()).spawn(move || {
            let _raw = RawScreen::into_raw_mode().unwrap();
            let input = input();
            let reader = input.read_sync();
            for input_event in reader {
                match input_event {
                    InputEvent::Keyboard(key_event) => {
                        let will_return = key_event==KeyEvent::Char(conf::KEY_QUIT);
                        input_thread_tx.send(key_event).unwrap();

                        if will_return {
                            // It's important to kill this thread upon quitting so the RawMode gets cleaned up promptly
                            return;
                        }
                    }
                    InputEvent::Mouse(_mouse_event) => {
                        // do nothing
                    },
                    InputEvent::Unsupported(_data) => {
                        // do nothing
                    },
                    InputEvent::Unknown => {
                        // do nothing
                    }
                }
            }

            // loop {

            //     // let key = stdin().keys().next().unwrap().unwrap();
            //     input_thread_tx.send(key).unwrap();
            // }
        });

        // The audio thread (if applicable)
        let (audio_thread_handle, audio_thread_tx) = if !quiet {
            let (tx, rx) = channel();
            let handle = thread::Builder::new().name("audio".to_string()).spawn(move || {
                play_sounds(rx, Sounds::Silence).unwrap();
            });
            (Some(handle), Some(tx))
        } else {
            (None, None)
        };

        // let w: Box<dyn Write> = if use_alt_screen {
        //     match stdout().into_raw_mode() {
        //         Ok(stdout_raw) => Box::new(AlternateScreen::from(stdout_raw)),
        //         Err(err) => return Err(format!("Error communicating with terminal: {}", err))
        //     }
        // } else {
        //     match stdout().into_raw_mode() {
        //         Ok(stdout_raw) => Box::new(stdout_raw),
        //         Err(err) => return Err(format!("Error communicating with terminal: {}", err))
        //     }
        // };

        // let _raw_mode = match RawScreen::into_raw_mode().map_err(|err| format!("Error obtaining raw screen access: {}", err))?;

        

        let term = terminal();

        let (width, height) = term.size().map_err(|error_kind| format!("{}", error_kind))?;
        let term_dims = Dims { width, height };

        let stdout = stdout();

        let mut ui = TermUI::new(
            game.dims(),
            term_dims,
            stdout,
            palette,
            unicode,
            confirm_turn_end,
            audio_thread_tx,
            input_thread_rx,
        );

        let mut prev_mode: Option<Mode> = None;
        let mut mode = self::mode::Mode::TurnStart;

        while mode.run(&mut game, &mut ui, &mut prev_mode) {
            // nothing here
        }

        if audio_thread_handle.is_some() {
            ui.audio_thread_tx.unwrap().send(Sounds::Silence).unwrap();
        }
    }

    println!("\n\n\tHe rules a moment: Chaos umpire sits,
\tAnd by decision more embroils the fray
\tBy which he reigns: next him, high arbiter,
\tChance governs all.

\t\t\t\tParadise Lost (2.907-910)\n"
    );

    Ok(())
}

pub trait MoveAnimator {
    #[deprecated = "Use `animate_proposed_move` instead. We want to animate based on the proposal and then actually take the action defined by the move so the game state doesn't reflect the move yet, since it's easier to work relative to the prior game state."]
    fn animate_move(&mut self, game: &Game, move_result: &Move);
    fn animate_proposed_move(&mut self, game: &mut Game, proposed_move: &ProposedMove);
}

pub trait UI : LogTarget + MoveAnimator {

}

pub struct DefaultUI;

impl LogTarget for DefaultUI {
    fn log_message<T>(&mut self, _message: T) where Message:From<T> {
        // do nothing
    }

    fn replace_message<T>(&mut self, _message: T) where Message:From<T> {
        // do nothing
    }
}

impl MoveAnimator for DefaultUI {
    fn animate_move(&mut self, _game: &Game, _move_result: &Move) {
        // do nothing
    }

    fn animate_proposed_move(&mut self, _game: &mut Game, _proposed_move: &ProposedMove) {
        // do nothing
    }
}

impl UI for DefaultUI {

}

trait Draw {
    fn draw(&mut self, game: &Game, stdout: &mut Stdout, palette: &Palette) {
        self.draw_no_flush(game, stdout, palette);
        stdout.flush().unwrap();
    }
    fn draw_no_flush(&mut self, game: &Game, stdout: &mut Stdout, palette: &Palette);
}

trait Component : Draw {
    fn set_rect(&mut self, rect: Rect);

    fn rect(&self) -> Rect;

    fn is_done(&self) -> bool;

    // fn goto(&self, x: u16, y: u16) -> termion::cursor::Goto {
    //     let rect = self.rect();
    //     goto(rect.left + x, rect.top + y)
    // }

    fn goto(&self, x: u16, y: u16) -> Goto {
        let rect = self.rect();
        Goto(rect.left + x, rect.top + y)
    }

    fn clear(&self, stdout: &mut Stdout) {
        let rect = self.rect();
        let blank_string = (0..rect.width).map(|_| " ").collect::<String>();
        for y in 0..rect.height {
            // write!(*stdout, "{}{}", self.goto(0, y), blank_string).unwrap();
            queue!(*stdout, self.goto(0, y), Output(blank_string.clone())).unwrap();//FIXME clear component without cloning a bunch of strings
        }
    }

    // fn draw_window_frame(&self, title: &str, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
    //
    // }
}

mod audio;
mod buf;
mod indicators;
mod log;
mod map;
mod mode;
mod scroll;
mod sym;

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

/// The terminal-based user interface.
struct TermUI {
    stdout: Stdout,
    term_dims: Dims,
    viewport_size: ViewportSize,

    map_scroller: Scroller<Map>,
    log: LogArea,
    sidebar_buf: RectBuffer,
    current_player: CurrentPlayer,
    turn: Turn,
    first_draw: bool,
    palette: Rc<Palette>,
    unicode: bool,
    confirm_turn_end: bool,
    audio_thread_tx: Option<Sender<Sounds>>,
    input_thread_rx: Receiver<KeyEvent>,
}

impl TermUI {
    fn new(
        map_dims: Dims,
        term_dims: Dims,
        stdout: Stdout,
        palette: Palette,
        unicode: bool,
        confirm_turn_end: bool,
        audio_thread_tx: Option<Sender<Sounds>>,
        input_thread_rx: Receiver<KeyEvent>,
    ) -> Self {
        let viewport_size = ViewportSize::REGULAR;
        let viewport_rect = viewport_size.rect(term_dims);
        let sidebar_rect = sidebar_rect(viewport_rect, term_dims);

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
            sidebar_buf: RectBuffer::new(sidebar_rect),
            current_player,

            turn: Turn::new(turn_rect(cp_rect)),

            first_draw: true,
            
            palette,

            unicode,

            confirm_turn_end,

            audio_thread_tx,
            input_thread_rx,
        };

        ui.clear();

        ui
    }

    fn clear(&mut self) {
        // write!(self.stdout, "{}", clear::All).unwrap();
        // self.stdout.queue(Clear(ClearType::All));
        queue!(self.stdout, Clear(ClearType::All), SetBg(self.palette.get_single(Colors::Background))).unwrap();

        // for x in 0..self.term_dims.width {
        //     for y in 0..self.term_dims.height {
        //         // write!(self.stdout, "{}{} ", goto(x,y), Bg(Rgb(0,0,0))).unwrap();
        //         queue!(self.stdout, goto(x,y), Output(String::from(" "))).unwrap();
        //     }
        // }
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
        self.draw_no_flush(game);
        self.stdout.flush().unwrap();
    }

    fn draw_no_flush(&mut self, game: &Game) {
        if self.first_draw {
            // write!(self.stdout, "{}{}{}{}",
            //     // termion::clear::All,
            //     goto(0,0),
            //     termion::style::Underline,
            //     conf::APP_NAME,
            //     StrongReset::new(&self.palette)
            // ).unwrap();
            queue!(self.stdout,
                Goto(0, 0),
                SetAttr(Attribute::Underlined),
                Output(conf::APP_NAME.to_string()),
                SetAttr(Attribute::Reset),
                SetBg(self.palette.get_single(Colors::Background))
            ).unwrap();

            self.first_draw = false;
        }

        self.log.draw_no_flush(game, &mut self.stdout, &self.palette);
        self.current_player.draw_no_flush(game, &mut self.stdout, &self.palette);
        self.map_scroller.draw_no_flush(game, &mut self.stdout, &self.palette);
        self.turn.draw_no_flush(game, &mut self.stdout, &self.palette);
        self.sidebar_buf.draw_no_flush(game, &mut self.stdout, &self.palette);

        // write!(self.stdout, "{}{}", StrongReset::new(&self.palette), termion::cursor::Hide).unwrap();
        queue!(self.stdout,
            SetAttr(Attribute::Reset),
            SetBg(self.palette.get_single(Colors::Background)),
            Hide
        ).unwrap();
    }

    fn draw_located_observations(&mut self, game: &Game, located_obs: &[LocatedObs]) {
        for located_obs in located_obs {
            if let Some(viewport_loc) = self.map_scroller.scrollable.map_to_viewport_coords(located_obs.loc) {
                let (city,unit) = if let Obs::Observed{ref tile,..} = located_obs.obs {
                    (Some(tile.city.as_ref()), Some(tile.unit.as_ref()))
                } else {
                    (Some(None),Some(None))
                };

                self.map_scroller.scrollable.draw_tile_no_flush(game, &mut self.stdout, viewport_loc, false, false, city, unit, None);
            }
        }
    }

    fn animate_combat<A:CombatCapable+Sym,D:CombatCapable+Sym>(
        &mut self,
        game: &Game,
        outcome: &CombatOutcome<A,D>,
        attacker_loc: Location,
        defender_loc: Location) {

        let map = &mut self.map_scroller.scrollable;

        let attacker_viewport_loc = map.map_to_viewport_coords(attacker_loc);
        let defender_viewport_loc = map.map_to_viewport_coords(defender_loc);
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
                map.draw_tile_and_flush(game, &mut self.stdout, viewport_loc, true, false, None, None, Some(sym));
                sleep_millis(100);
                map.draw_tile_and_flush(game, &mut self.stdout, viewport_loc, false, false, None, None, Some(sym));
            } else {
                sleep_millis(100);
            }
        }
    }

    fn viewport_rect(&self) -> Rect {
        self.viewport_size.rect(self.term_dims)
    }

    fn cursor_viewport_loc(&self, mode: &Mode, game: &Game) -> Option<Location> {
        let map = &self.map_scroller.scrollable;

        match *mode {
            Mode::SetProduction{city_loc} => map.map_to_viewport_coords(city_loc),
            Mode::GetUnitOrders{unit_id,..} => {
                let unit_loc = game.current_player_unit_loc(unit_id).unwrap();
                map.map_to_viewport_coords(unit_loc)
            },
            _ => None
        }
    }

    fn ensure_map_loc_visible(&mut self, map_loc: Location) {
        self.map_scroller.scrollable.center_viewport_if_not_visible(map_loc);
    }

    fn cursor_map_loc(&self, mode: &Mode, game: &Game) -> Option<Location> {
        match *mode {
            Mode::SetProduction{city_loc} => Some(city_loc),
            Mode::GetUnitOrders{unit_id,..} => {
                let unit_loc = game.current_player_unit_loc(unit_id).unwrap();
                Some(unit_loc)
            },
            _ => None
        }
    }

    fn play_sound(&self, sound: Sounds) {
        if let Some(tx) = self.audio_thread_tx.as_ref() {
            tx.send(sound).unwrap();
        }
    }

    /// Block until a key is pressed; return that key
    fn get_key(&self) -> KeyEvent {
        self.input_thread_rx.recv().unwrap()
    }

    /// Return Some(key) if a key from the input thread is waiting for us, otherwise return None
    fn try_get_key(&self) -> Option<KeyEvent> {
        self.input_thread_rx.try_recv().ok()
    }

    fn confirm_turn_end(&self) -> bool {
        self.confirm_turn_end
    }

    fn sidebar_buf_mut(&mut self) -> &mut RectBuffer {
        &mut self.sidebar_buf
    }
}

impl LogTarget for TermUI {
    fn log_message<T>(&mut self, message: T) where Message:From<T> {
        self.log.log_message(message);
    }

    fn replace_message<T>(&mut self, message: T) where Message:From<T> {
        self.log.replace_message(message);
    }
}

impl MoveAnimator for TermUI {
    fn animate_move(&mut self, game: &Game, move_result: &Move) {
        let mut current_loc = move_result.starting_loc;

        self.ensure_map_loc_visible(current_loc);
        self.draw(game);

        for (move_idx, move_) in move_result.components.iter().enumerate() {
            let target_loc = move_.loc;
            self.ensure_map_loc_visible(current_loc);

            //FIXME This draw is revealing current game state when we really need to show the past few steps of game state involved with this move
            // self.draw_no_flush(game);

            let mut was_combat = false;
            if let Some(ref combat) = move_.unit_combat {
                self.animate_combat(game, combat, current_loc, target_loc);
                was_combat = true;
            }

            if let Some(ref combat) = move_.city_combat {
                self.animate_combat(game, combat, current_loc, target_loc);
                was_combat = true;
            }

            if move_.distance_moved() > 0 {
                self.log_message(Message {
                    text: format!("Unit {} {}", move_result.unit, if move_.moved_successfully() {
                        if was_combat {"victorious"} else {"moved successfully"}
                    } else {"destroyed"}),
                    mark: Some('*'),
                    fg_color: Some(Colors::Combat),
                    bg_color: None,
                    source: Some(MessageSource::UI)
                });
            }

            if move_.moved_successfully() {
                self.draw_located_observations(game, &move_.observations_after_move);
            }

            current_loc = target_loc;

            self.stdout.flush().unwrap();

            if move_idx < move_result.components.len() - 1 {
                sleep_millis(100);
            }
        }

        if move_result.unit.moves_remaining() == 0 {
            sleep_millis(250);
        }
    }

    fn animate_proposed_move(&mut self, game: &mut Game, proposed_move: &ProposedMove) {
        let mut current_loc = proposed_move.0.starting_loc;

        self.ensure_map_loc_visible(current_loc);
        self.draw(game);

        for (move_idx, move_) in proposed_move.0.components.iter().enumerate() {
            let target_loc = move_.loc;
            self.ensure_map_loc_visible(current_loc);

            //FIXME This draw is revealing current game state when we really need to show the past few steps of game state involved with this move
            // self.draw_no_flush(game);

            let mut was_combat = false;
            if let Some(ref combat) = move_.unit_combat {
                self.animate_combat(game, combat, current_loc, target_loc);
                was_combat = true;
            }

            if let Some(ref combat) = move_.city_combat {
                self.animate_combat(game, combat, current_loc, target_loc);
                was_combat = true;
            }

            if move_.distance_moved() > 0 {
                self.log_message(Message {
                    text: format!("Unit {} {}", proposed_move.0.unit, if move_.moved_successfully() {
                        if was_combat {"victorious"} else {"moved successfully"}
                    } else {"destroyed"}),
                    mark: Some('*'),
                    fg_color: Some(Colors::Combat),
                    bg_color: None,
                    source: Some(MessageSource::UI)
                });
            }

            if move_.moved_successfully() {
                self.draw_located_observations(game, &move_.observations_after_move);
            }

            current_loc = target_loc;

            self.stdout.flush().unwrap();

            if move_idx < proposed_move.0.components.len() - 1 {
                sleep_millis(100);
            }
        }

        if proposed_move.0.unit.moves_remaining() == 0 {
            sleep_millis(250);
        }
    }
}
