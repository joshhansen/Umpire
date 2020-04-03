//!
//! A text-based user interface implemented for Unix terminals.
//!
//! The abstract game logic is implemented in `game::Game`. This user interface references that
//! game engine but is otherwise independent in realizing a user experience around the game.

use std::{
    io::{Stdout,Write,stdout},
    rc::{
        Rc,
    },
    sync::{
        mpsc::{
            Receiver,
            Sender,
            channel,
        },
    },
    thread::{
        self,
        JoinHandle,
    },
};

use crossterm::{
    cursor::{
        Hide,
        MoveTo,
    },
    event::{
        Event,
        KeyEvent,
        KeyCode,
        read as read_event,
    },
    queue,
    style::{
        Attribute,
        Print,
        SetAttribute,
        SetBackgroundColor,
    },
    terminal::{
        Clear,
        ClearType,
        EnterAlternateScreen,
        LeaveAlternateScreen,
        enable_raw_mode,
        disable_raw_mode,
        size as terminal_size,
    },
};

use crate::{
    color::{Colors,Palette},
    conf::{
        self,
        HEADER_HEIGHT,
    },
    game::{
        combat::{CombatCapable,CombatOutcome,CombatParticipant},
        city::City,
        map::Tile,
        move_::{
            Move,
            ProposedMove,
        },
        obs::{
            LocatedObs,
            Obs,
        },
        player::{
            PlayerTurnControl,
            TurnTaker,
        },
        unit::Unit,
    },
    log::{LogTarget,Message,MessageSource},
    util::{Dims,Rect,Location,Vec2d,sleep_millis}
};

use self::{
    audio::{
        play_sounds,
        Sounds,
    },
    buf::RectBuffer,
    mode::ModeStatus,
    scroll::ScrollableComponent,
    sym::Sym,
};

pub trait MoveAnimator {
    fn animate_move(&mut self, game: &PlayerTurnControl, move_result: &Move);

    #[deprecated = "Proposed moves now ARE moves, just not applied to the principal game state yet."]
    fn animate_proposed_move(&mut self, game: &mut PlayerTurnControl, proposed_move: &ProposedMove);
}

/// An abstraction on the terminal UI, basically for test mocking purposes
pub(in crate::ui) trait UI : LogTarget + MoveAnimator {
    fn confirm_turn_end(&self) -> bool;

    /// Center the map view on the given map location
    fn center_map(&mut self, map_loc: Location);

    fn clear_sidebar(&mut self);

    fn viewport_rect(&self) -> Rect;

    fn term_dims(&self) -> Dims;

    fn unicode(&self) -> bool;

    fn cursor_map_loc(&self, mode: &Mode, game: &PlayerTurnControl) -> Option<Location>;
    
    fn cursor_viewport_loc(&self, mode: &Mode, game: &PlayerTurnControl) -> Option<Location>;
    
    fn current_player_map_tile<'a>(&self, ctrl: &'a PlayerTurnControl, viewport_loc: Location) -> Option<&'a Tile>;

    fn draw(&mut self, game: &PlayerTurnControl);

    fn draw_no_flush(&mut self, game: &PlayerTurnControl);

    fn draw_current_player(&mut self, ctrl: &PlayerTurnControl);

    fn draw_log(&mut self, ctrl: &PlayerTurnControl);

    fn draw_map(&mut self, ctrl: &PlayerTurnControl);

    /// Renders a particular location in the map viewport
    ///
    /// Flushes stdout for convenience
    fn draw_map_tile_and_flush(&mut self,
        game: &PlayerTurnControl,
        viewport_loc: Location,
        highlight: bool,// Highlighting as for a cursor
        unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        unit_override: Option<Option<&Unit>>,
        
        // A symbol to display instead of what's really here
        symbol_override: Option<&'static str>,
    
        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        obs_override: Option<&Obs>,
    );
    
    /// Renders a particular location in the map viewport
    fn draw_map_tile_no_flush(&mut self,
        game: &PlayerTurnControl,
        viewport_loc: Location,
        highlight: bool,// Highlighting as for a cursor
        unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        unit_override: Option<Option<&Unit>>,
        
        // A symbol to display instead of what's really here
        symbol_override: Option<&'static str>,
    
        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        obs_override: Option<&Obs>,
    );

    /// Block until a key is pressed; return that key
    fn get_key(&self) -> KeyEvent;

    fn map_to_viewport_coords(&self, map_loc: Location) -> Option<Location>;

    fn play_sound(&self, sound: Sounds);

    fn pop_log_message(&mut self) -> Option<Message>;

    fn rotate_viewport_size(&mut self, game: &PlayerTurnControl);

    // fn sidebar_buf_mut(&mut self) -> &mut RectBuffer;

    fn scroll_map_relative<V:Into<Vec2d<i32>>>(&mut self, direction: V);

    fn set_sidebar_row(&mut self, row_idx: usize, row: String);

    #[deprecated]
    fn shift_map_viewport<V:Into<Vec2d<i32>>>(&mut self, direction: V);

    fn viewport_to_map_coords(&self, game: &PlayerTurnControl, viewport_loc: Location) -> Option<Location>;
}

struct DefaultUI;

impl LogTarget for DefaultUI {
    fn log_message<T>(&mut self, _message: T) where Message:From<T> {
        // do nothing
    }

    fn replace_message<T>(&mut self, _message: T) where Message:From<T> {
        // do nothing
    }
}

impl MoveAnimator for DefaultUI {
    fn animate_move(&mut self, _game: &PlayerTurnControl, _move_result: &Move) {
        // do nothing
    }

    fn animate_proposed_move(&mut self, _game: &mut PlayerTurnControl, _proposed_move: &ProposedMove) {
        // do nothing
    }
}

impl UI for DefaultUI {
    fn confirm_turn_end(&self) -> bool {
        false
    }

    fn center_map(&mut self, _map_loc: Location) {
        // do nothing
    }

    fn viewport_rect(&self) -> Rect {
        Rect::new(0, 0, 0, 0)
    }

    fn term_dims(&self) -> Dims {
        Dims::new(0, 0)
    }

    fn unicode(&self) -> bool {
        false
    }

    fn clear_sidebar(&mut self) {
        // do nothing
    }

    fn cursor_map_loc(&self, _mode: &Mode, _game: &PlayerTurnControl) -> Option<Location> {
        None
    }
    
    fn cursor_viewport_loc(&self, _mode: &Mode, game: &PlayerTurnControl) -> Option<Location> {
        None
    }
    
    fn current_player_map_tile<'a>(&self, _ctrl: &'a PlayerTurnControl, _viewport_loc: Location) -> Option<&'a Tile> {
        None
    }

    fn draw(&mut self, _game: &PlayerTurnControl) {
        // do nothing
    }

    fn draw_no_flush(&mut self, _game: &PlayerTurnControl) {
        // do nothing
    }

    fn draw_current_player(&mut self, _ctrl: &PlayerTurnControl) {
        // do nothing
    }

    fn draw_log(&mut self, _ctrl: &PlayerTurnControl) {
        // do nothing
    }

    fn draw_map(&mut self, _ctrl: &PlayerTurnControl) {
        // do nothing
    }

    /// Renders a particular location in the map viewport
    ///
    /// Flushes stdout for convenience
    fn draw_map_tile_and_flush(&mut self,
        _game: &PlayerTurnControl,
        _viewport_loc: Location,
        _highlight: bool,// Highlighting as for a cursor
        _unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        _city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        _unit_override: Option<Option<&Unit>>,
        
        // A symbol to display instead of what's really here
        _symbol_override: Option<&'static str>,
    
        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        _obs_override: Option<&Obs>,
    ) {
        // do nothing
    }
    
    /// Renders a particular location in the map viewport
    fn draw_map_tile_no_flush(&mut self,
        _game: &PlayerTurnControl,
        _viewport_loc: Location,
        _highlight: bool,// Highlighting as for a cursor
        _unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        _city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        _unit_override: Option<Option<&Unit>>,
        
        // A symbol to display instead of what's really here
        _symbol_override: Option<&'static str>,
    
        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        _obs_override: Option<&Obs>,
    ) {
        // do nothing
    }

    /// Block until a key is pressed; return that key
    fn get_key(&self) -> KeyEvent {
        KeyEvent::from(KeyCode::Null)
    }

    fn map_to_viewport_coords(&self, _map_loc: Location) -> Option<Location> {
        None
    }

    fn play_sound(&self, _sound: Sounds) {
        // do nothing
    }

    fn pop_log_message(&mut self) -> Option<Message> {
        None
    }

    fn rotate_viewport_size(&mut self, _game: &PlayerTurnControl) {
        // do nothing
    }

    fn set_sidebar_row(&mut self, _row_idx: usize, _row: String) {
        // do nothing
    }

    fn scroll_map_relative<V:Into<Vec2d<i32>>>(&mut self, _direction: V) {
        // do nothing
    }

    fn shift_map_viewport<V:Into<Vec2d<i32>>>(&mut self, _direction: V) {
        // do nothing
    }

    fn viewport_to_map_coords(&self, _game: &PlayerTurnControl, _viewport_loc: Location) -> Option<Location> {
        None
    }
}

trait Draw {
    fn draw(&mut self, game: &PlayerTurnControl, stdout: &mut Stdout, palette: &Palette) {
        self.draw_no_flush(game, stdout, palette);
        stdout.flush().unwrap();
    }
    fn draw_no_flush(&mut self, game: &PlayerTurnControl, stdout: &mut Stdout, palette: &Palette);
}

trait Component : Draw {
    fn set_rect(&mut self, rect: Rect);

    fn rect(&self) -> Rect;

    fn is_done(&self) -> bool;

    // fn goto(&self, x: u16, y: u16) -> termion::cursor::Goto {
    //     let rect = self.rect();
    //     goto(rect.left + x, rect.top + y)
    // }

    fn goto(&self, x: u16, y: u16) -> MoveTo {
        let rect = self.rect();
        MoveTo(rect.left + x, rect.top + y)
    }

    fn clear(&self, stdout: &mut Stdout) {
        let rect = self.rect();
        let blank_string = (0..rect.width).map(|_| " ").collect::<String>();
        for y in 0..rect.height {
            // write!(*stdout, "{}{}", self.goto(0, y), blank_string).unwrap();
            queue!(*stdout, self.goto(0, y), Print(blank_string.clone())).unwrap();//FIXME clear component without cloning a bunch of strings
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
pub struct TermUI {
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

    /// Whether or not to use Crossterm's alternate screen. Useful to disable this when debugging messages are desired.
    use_alt_screen: bool,

    /// Sender by which to send sound events to the audio thread (if not quieted)
    audio_thread_tx: Option<Sender<Sounds>>,

    /// Receiver by which to get input events from the input thread
    input_thread_rx: Receiver<KeyEvent>,

    /// We need to keep the audio thread handle because the thread is killed when it goes out of scope.
    _audio_thread_handle: Option<JoinHandle<()>>,

    /// We need to keep the input thread handle because the thread is killed when it goes out of scope.
    _input_thread_handle: JoinHandle<()>,
}

impl TermUI {
    /// This method initializes the terminal user interface.
    /// 
    /// It will be de-initialized when this struct goes out of scope. See the `Drop` implementation.
    pub fn new(
        map_dims: Dims,
        palette: Palette,
        unicode: bool,
        confirm_turn_end: bool,
        quiet: bool,
        use_alt_screen: bool,
    ) -> Result<Self,crossterm::ErrorKind> {
        let (width, height) = terminal_size()?;
        let term_dims = Dims { width, height };
        // let term_dims = Dims::new(120, 60);

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

        // The input thread
        let (input_thread_tx, input_thread_rx) = channel();
        let input_thread_handle = thread::Builder::new().name("input".to_string()).spawn(move || {
            enable_raw_mode().unwrap();

            loop {
                match read_event() {
                    Ok(event) => {
                        match event {
                            Event::Key(key_event) => {
                                let will_return = key_event.code==KeyCode::Char(conf::KEY_QUIT);
                                input_thread_tx.send(key_event).unwrap();
            
                                if will_return {
                                    disable_raw_mode().unwrap();
                                    return;
                                }
                            },
                            Event::Mouse(_mouse_event) => {},
                            Event::Resize(_columns, _rows) => {
                                //TODO Handle resize events
                            },
                        }
                    },
                    Err(err) => {
                        eprintln!("Error reading event: {}", err);
                        break;
                    },
                }
            }

            disable_raw_mode().unwrap();
        }).unwrap();

        // The audio thread (if applicable)
        let (audio_thread_handle, audio_thread_tx) = if !quiet {
            let (tx, rx) = channel();
            let handle = thread::Builder::new().name("audio".to_string()).spawn(move || {
                play_sounds(rx, Sounds::Silence).unwrap();
            }).unwrap();
            (Some(handle), Some(tx))
        } else {
            (None, None)
        };

        let mut stdout = stdout();

        if use_alt_screen {
            queue!(stdout, EnterAlternateScreen).unwrap();
        }


        let mut ui = Self {
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

            use_alt_screen,

            audio_thread_tx,
            input_thread_rx,
            _audio_thread_handle: audio_thread_handle,
            _input_thread_handle: input_thread_handle,
        };

        ui.clear();

        Ok(ui)
    }

    fn clear(&mut self) {
        // write!(self.stdout, "{}", clear::All).unwrap();
        // self.stdout.queue(Clear(ClearType::All));
        queue!(self.stdout, Clear(ClearType::All), SetBackgroundColor(self.palette.get_single(Colors::Background))).unwrap();

        // for x in 0..self.term_dims.width {
        //     for y in 0..self.term_dims.height {
        //         // write!(self.stdout, "{}{} ", goto(x,y), Bg(Rgb(0,0,0))).unwrap();
        //         queue!(self.stdout, goto(x,y), Output(String::from(" "))).unwrap();
        //     }
        // }
    }

    fn set_viewport_size(&mut self, game: &PlayerTurnControl, viewport_size: ViewportSize) {
        self.viewport_size = viewport_size;
        self.map_scroller.set_rect(self.viewport_size.rect(self.term_dims));
        self.draw(game);
    }



    fn draw_located_observations(&mut self, game: &PlayerTurnControl, located_obs: &[LocatedObs]) {
        for located_obs in located_obs {
            if let Some(viewport_loc) = self.map_scroller.scrollable.map_to_viewport_coords(located_obs.loc) {
                // let (city,unit) = if let Obs::Observed{ref tile,..} = located_obs.item {
                //     (Some(tile.city.as_ref()), Some(tile.unit.as_ref()))
                // } else {
                //     (Some(None),Some(None))
                // };

                self.map_scroller.scrollable.draw_tile_no_flush(game, &mut self.stdout, viewport_loc, false, 
                    false, None, None, None, Some(&located_obs.item));
            }
        }
    }

    fn animate_combat<A:CombatCapable+Sym,D:CombatCapable+Sym>(
        &mut self,
        game: &PlayerTurnControl,
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
                map.draw_tile_and_flush(game, &mut self.stdout, viewport_loc, true, false, None, None, Some(sym), None);
                sleep_millis(100);
                map.draw_tile_and_flush(game, &mut self.stdout, viewport_loc, false, false, None, None, Some(sym), None);
            } else {
                sleep_millis(100);
            }
        }
    }

    fn ensure_map_loc_visible(&mut self, map_loc: Location) {
        self.map_scroller.scrollable.center_viewport_if_not_visible(map_loc);
    }

    fn confirm_turn_end(&self) -> bool {
        self.confirm_turn_end
    }

    fn map(&self) -> &Map {
        &self.map_scroller.scrollable
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
    fn animate_move(&mut self, game: &PlayerTurnControl, move_result: &Move) {
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

    fn animate_proposed_move(&mut self, game: &mut PlayerTurnControl, proposed_move: &ProposedMove) {
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

            if was_combat {
                self.log_message(Message {
                    text: format!("Unit {} was {}", proposed_move.0.unit, if move_.moved_successfully() {
                        "victorious"
                    } else {
                        "destroyed"
                    }),
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

        if proposed_move.0.moved_successfully() {
            self.log_message(Message {
                text: format!("Unit {} moved successfully", proposed_move.0.unit),
                mark: None,
                fg_color: Some(Colors::Combat),
                bg_color: None,
                source: Some(MessageSource::UI)
            });
        }

        if proposed_move.0.unit.moves_remaining() == 0 {
            sleep_millis(250);
        }
    }
}

impl UI for TermUI {
    fn viewport_rect(&self) -> Rect {
        self.viewport_size.rect(self.term_dims)
    }

    fn term_dims(&self) -> Dims {
        self.term_dims
    }

    fn unicode(&self) -> bool {
        self.unicode
    }

    fn center_map(&mut self, map_loc: Location) {
        self.map_scroller.scrollable.center_viewport(map_loc);
    }

    fn clear_sidebar(&mut self) {
        self.sidebar_buf.clear();
    }

    fn cursor_map_loc(&self, mode: &Mode, game: &PlayerTurnControl) -> Option<Location> {
        match *mode {
            Mode::SetProduction{city_loc} => Some(city_loc),
            Mode::GetUnitOrders{unit_id,..} => {
                let unit_loc = game.current_player_unit_loc(unit_id).unwrap();
                Some(unit_loc)
            },
            _ => None
        }
    }
    
    fn cursor_viewport_loc(&self, mode: &Mode, game: &PlayerTurnControl) -> Option<Location> {
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

    fn current_player_map_tile<'a>(&self, ctrl: &'a PlayerTurnControl, viewport_loc: Location) -> Option<&'a Tile> {
        self.map_scroller.scrollable.current_player_tile(ctrl, viewport_loc)
    }

    fn draw_current_player(&mut self, ctrl: &PlayerTurnControl) {
        self.current_player.draw(ctrl, &mut self.stdout, &self.palette);
    }

    fn draw_log(&mut self, ctrl: &PlayerTurnControl) {
        self.log.draw(ctrl, &mut self.stdout, &self.palette);// this will flush
    }

    fn draw_map(&mut self, ctrl: &PlayerTurnControl) {
        self.map_scroller.draw(ctrl, &mut self.stdout, &self.palette);
    }

    fn confirm_turn_end(&self) -> bool {
        self.confirm_turn_end
    }

    fn draw(&mut self, game: &PlayerTurnControl) {
        self.draw_no_flush(game);
        self.stdout.flush().unwrap();
    }

    fn draw_map_tile_and_flush(&mut self,
        game: &PlayerTurnControl,
        viewport_loc: Location,
        highlight: bool,// Highlighting as for a cursor
        unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        unit_override: Option<Option<&Unit>>,
        
        // A symbol to display instead of what's really here
        symbol_override: Option<&'static str>,
    
        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        obs_override: Option<&Obs>,
    ) {
        self.map_scroller.scrollable.draw_tile_and_flush(
            game, &mut self.stdout, viewport_loc, highlight, unit_active, city_override, unit_override, symbol_override,
            obs_override
        )
    }
    
    fn draw_map_tile_no_flush(&mut self,
        game: &PlayerTurnControl,
        viewport_loc: Location,
        highlight: bool,// Highlighting as for a cursor
        unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        unit_override: Option<Option<&Unit>>,
        
        // A symbol to display instead of what's really here
        symbol_override: Option<&'static str>,
    
        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        obs_override: Option<&Obs>,
    ) {
        self.map_scroller.scrollable.draw_tile_no_flush(
            game, &mut self.stdout, viewport_loc, highlight, unit_active, city_override, unit_override, symbol_override,
            obs_override
        )
    }

    fn draw_no_flush(&mut self, game: &PlayerTurnControl) {
        if self.first_draw {
            // write!(self.stdout, "{}{}{}{}",
            //     // termion::clear::All,
            //     goto(0,0),
            //     termion::style::Underline,
            //     conf::APP_NAME,
            //     StrongReset::new(&self.palette)
            // ).unwrap();
            queue!(self.stdout,
                MoveTo(0, 0),
                SetAttribute(Attribute::Underlined),
                Print(conf::APP_NAME.to_string()),
                SetAttribute(Attribute::Reset),
                SetBackgroundColor(self.palette.get_single(Colors::Background))
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
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(self.palette.get_single(Colors::Background)),
            Hide
        ).unwrap();
    }

    /// Block until a key is pressed; return that key
    fn get_key(&self) -> KeyEvent {
        self.input_thread_rx.recv().unwrap()
    }

    fn map_to_viewport_coords(&self, map_loc: Location) -> Option<Location> {
        self.map_scroller.scrollable.map_to_viewport_coords(map_loc)
    }

    // /// Return Some(key) if a key from the input thread is waiting for us, otherwise return None
    // fn try_get_key(&self) -> Option<KeyEvent> {
    //     self.input_thread_rx.try_recv().ok()
    // }

    fn play_sound(&self, sound: Sounds) {
        if let Some(tx) = self.audio_thread_tx.as_ref() {
            tx.send(sound).unwrap();
        }
    }

    fn pop_log_message(&mut self) -> Option<Message> {
        self.log.pop_message()
    }

    fn rotate_viewport_size(&mut self, game: &PlayerTurnControl) {
        let new_size = match self.viewport_size {
            ViewportSize::REGULAR => ViewportSize::THEATER,
            ViewportSize::THEATER => ViewportSize::FULLSCREEN,
            ViewportSize::FULLSCREEN => ViewportSize::REGULAR
        };

        self.set_viewport_size(game, new_size);
        self.draw(game);
    }

    fn scroll_map_relative<V:Into<Vec2d<i32>>>(&mut self, direction: V)  {
        self.map_scroller.scrollable.scroll_relative(direction.into());
    }

    fn set_sidebar_row(&mut self, row_idx: usize, row: String) {
        self.sidebar_buf.set_row(row_idx, row)
    }

    fn shift_map_viewport<V:Into<Vec2d<i32>>>(&mut self, direction: V) {
        self.map_scroller.scrollable.shift_viewport(direction);
    }

    fn viewport_to_map_coords(&self, game: &PlayerTurnControl, viewport_loc: Location) -> Option<Location> {
        self.map().viewport_to_map_coords(game, viewport_loc)
    }
}

impl TurnTaker for TermUI {
    fn take_turn(&mut self, ctrl: &mut PlayerTurnControl) {
        let mut prev_mode: Option<Mode> = None;
        let mut mode = self::mode::Mode::TurnStart;
        while mode.run(ctrl, self, &mut prev_mode) == ModeStatus::Continue {
            // nothing here
        }
    }
}

impl Drop for TermUI {
    fn drop(&mut self) {
        if self.use_alt_screen {
            queue!(self.stdout, LeaveAlternateScreen).unwrap();
        }
    
        if let Some(ref tx) = self.audio_thread_tx {
            tx.send(Sounds::Silence).unwrap();
        }

        // if audio_thread_handle.is_some() {
        //     ui.audio_thread_tx.unwrap().send(Sounds::Silence).unwrap();
        // }
    }
}