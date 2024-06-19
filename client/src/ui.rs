//!
//! A text-based user interface implemented for Unix terminals.
//!
//! The abstract game logic is implemented in `game::Game`. This user interface references that
//! game engine but is otherwise independent in realizing a user experience around the game.

use std::{
    borrow::Cow,
    cmp,
    io::{stdout, Result as IoResult, Stdout, Write},
    sync::{
        mpsc::{channel, sync_channel, Receiver, RecvError, SyncSender},
        Mutex,
    },
    thread::{self, JoinHandle},
};

use async_trait::async_trait;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{read as read_event, Event, KeyCode, KeyEvent},
    queue,
    style::{Attribute, Print, SetAttribute, SetBackgroundColor},
    terminal::{
        disable_raw_mode, enable_raw_mode, size as terminal_size, Clear, ClearType,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};

use common::{
    colors::Colors,
    conf::{self, HEADER_HEIGHT},
    game::{
        ai::AiDevice,
        city::City,
        combat::{CombatCapable, CombatOutcome, CombatParticipant},
        map::Tile,
        move_::Move,
        obs::{LocatedObs, Obs},
        player::PlayerTurn,
        turn::TurnOutcome,
        turn_async::TurnTaker,
        unit::Unit,
    },
    log::{LogTarget, Message, MessageSource},
    util::{sleep_millis, Dims, Location, Rect, Vec2d},
};

use umpire_tui::{
    color::Palette,
    map::Map,
    scroll::{ScrollableComponent, Scroller},
    sym::Sym,
    Component, Draw,
};

use self::{
    audio::{play_sounds, Sounds},
    buf::RectBuffer,
    mode::ModeStatus,
};

#[async_trait]
pub trait MoveAnimator {
    async fn animate_move(&mut self, game: &PlayerTurn, move_result: &Move) -> IoResult<()>;
}

/// An abstraction on the terminal UI, basically for test mocking purposes
#[async_trait]
pub trait UI: LogTarget + MoveAnimator {
    fn confirm_turn_end(&self) -> bool;

    /// Center the map view on the given map location
    fn center_map(&mut self, map_loc: Location);

    fn clear_sidebar(&mut self);

    fn viewport_rect(&self) -> Rect;

    fn viewport_size(&self) -> ViewportSize;

    fn term_dims(&self) -> Dims;

    fn unicode(&self) -> bool;

    async fn cursor_map_loc(&self, mode: &Mode, game: &PlayerTurn) -> Option<Location>;

    async fn cursor_viewport_loc(&self, mode: &Mode, game: &PlayerTurn) -> Option<Location>;

    async fn current_player_map_tile<'a>(
        &self,
        ctrl: &'a PlayerTurn,
        viewport_loc: Location,
    ) -> Option<Cow<'a, Tile>>;

    async fn draw(&mut self, game: &PlayerTurn) -> IoResult<()>;

    async fn draw_no_flush(&mut self, game: &PlayerTurn) -> IoResult<()>;

    async fn draw_current_player(&mut self, ctrl: &PlayerTurn) -> IoResult<()>;

    async fn draw_log(&mut self, ctrl: &PlayerTurn) -> IoResult<()>;

    async fn draw_map(&mut self, ctrl: &PlayerTurn) -> IoResult<()>;

    /// Renders a particular location in the map viewport
    ///
    /// Flushes stdout for convenience
    async fn draw_map_tile_and_flush(
        &mut self,
        game: &PlayerTurn,
        viewport_loc: Location,
        highlight: bool,   // Highlighting as for a cursor
        unit_active: bool, // Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        unit_override: Option<Option<&Unit>>,

        // A symbol to display instead of what's really here
        symbol_override: Option<&str>,

        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        obs_override: Option<&Obs>,
    ) -> IoResult<()>;

    /// Block until a key is pressed; return that key
    fn get_key(&self) -> Result<KeyEvent, RecvError>;

    fn map_to_viewport_coords(&self, map_loc: Location) -> Option<Location>;

    fn play_sound(&self, sound: Sounds);

    fn pop_log_message(&mut self) -> Option<Message>;

    async fn rotate_viewport_size(&mut self, game: &PlayerTurn) -> IoResult<()>;

    fn scroll_map_relative<V: Into<Vec2d<i32>>>(&mut self, direction: V);

    fn set_sidebar_row(&mut self, row_idx: usize, row: String);

    async fn viewport_to_map_coords(
        &self,
        game: &PlayerTurn,
        viewport_loc: Location,
    ) -> Option<Location>;
}

struct DefaultUI;

impl LogTarget for DefaultUI {
    fn log_message<T>(&mut self, _message: T)
    where
        Message: From<T>,
    {
        // do nothing
    }

    fn replace_message<T>(&mut self, _message: T)
    where
        Message: From<T>,
    {
        // do nothing
    }
}

#[async_trait]
impl MoveAnimator for DefaultUI {
    async fn animate_move(&mut self, _game: &PlayerTurn, _move_result: &Move) -> IoResult<()> {
        Ok(()) // do nothing
    }
}

#[async_trait]
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

    fn viewport_size(&self) -> ViewportSize {
        ViewportSize::Regular
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

    async fn cursor_map_loc(&self, _mode: &Mode, _game: &PlayerTurn) -> Option<Location> {
        None
    }

    async fn cursor_viewport_loc(&self, _mode: &Mode, _game: &PlayerTurn) -> Option<Location> {
        None
    }

    async fn current_player_map_tile<'a>(
        &self,
        _ctrl: &'a PlayerTurn,
        _viewport_loc: Location,
    ) -> Option<Cow<'a, Tile>> {
        None
    }

    async fn draw(&mut self, _game: &PlayerTurn) -> IoResult<()> {
        Ok(()) // do nothing
    }

    async fn draw_no_flush(&mut self, _game: &PlayerTurn) -> IoResult<()> {
        Ok(()) // do nothing
    }

    async fn draw_current_player(&mut self, _ctrl: &PlayerTurn) -> IoResult<()> {
        Ok(()) // do nothing
    }

    async fn draw_log(&mut self, _ctrl: &PlayerTurn) -> IoResult<()> {
        Ok(()) // do nothing
    }

    async fn draw_map(&mut self, _ctrl: &PlayerTurn) -> IoResult<()> {
        Ok(()) // do nothing
    }

    /// Renders a particular location in the map viewport
    ///
    /// Flushes stdout for convenience
    async fn draw_map_tile_and_flush(
        &mut self,
        _game: &PlayerTurn,
        _viewport_loc: Location,
        _highlight: bool,   // Highlighting as for a cursor
        _unit_active: bool, // Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        _city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        _unit_override: Option<Option<&Unit>>,

        // A symbol to display instead of what's really here
        _symbol_override: Option<&str>,

        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        _obs_override: Option<&Obs>,
    ) -> IoResult<()> {
        // do nothing
        Ok(())
    }

    /// Block until a key is pressed; return that key
    fn get_key(&self) -> Result<KeyEvent, RecvError> {
        Ok(KeyEvent::from(KeyCode::Null))
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

    async fn rotate_viewport_size(&mut self, _game: &PlayerTurn) -> IoResult<()> {
        Ok(()) // do nothing
    }

    fn set_sidebar_row(&mut self, _row_idx: usize, _row: String) {
        // do nothing
    }

    fn scroll_map_relative<V: Into<Vec2d<i32>>>(&mut self, _direction: V) {
        // do nothing
    }

    async fn viewport_to_map_coords(
        &self,
        _game: &PlayerTurn,
        _viewport_loc: Location,
    ) -> Option<Location> {
        None
    }
}

mod audio;
mod buf;
mod indicators;
mod log;
mod mode;

use self::indicators::{CurrentPlayer, Turn};
use self::log::LogArea;
use self::mode::Mode;

const MAX_MID_HEIGHT: u16 = 25;

#[derive(Clone, Copy)]
pub enum ViewportSize {
    Regular,
    Theater,
    Fullscreen,
}

impl ViewportSize {
    fn rect(&self, term_dims: Dims) -> Rect {
        let mid_y = match term_dims.height {
            0 => 0,
            1 | 2 => 1,
            3 => 2,
            4 | 5 => 3,
            x => cmp::min(x / 2, MAX_MID_HEIGHT),
        };
        match *self {
            ViewportSize::Regular => Rect {
                left: 0,
                top: HEADER_HEIGHT,
                width: (term_dims.width - V_SCROLLBAR_WIDTH) / 2,
                height: mid_y,
            },
            ViewportSize::Theater => Rect {
                left: 0,
                top: HEADER_HEIGHT,
                width: term_dims.width - V_SCROLLBAR_WIDTH,
                height: mid_y,
            },
            ViewportSize::Fullscreen => Rect {
                left: 0,
                top: 0,
                width: term_dims.width - V_SCROLLBAR_WIDTH,
                height: term_dims.height - H_SCROLLBAR_HEIGHT - 1,
            },
        }
    }
}

fn current_player_rect() -> Rect {
    Rect {
        left: 10,
        top: 0,
        width: 21,
        height: 1,
    }
}

fn turn_rect(current_player_rect: Rect) -> Rect {
    Rect {
        left: current_player_rect.right() + 2,
        top: 0,
        width: 11,
        height: 1,
    }
}

fn log_area_rect(term_dims: Dims, viewport_size: ViewportSize) -> Rect {
    let viewport_rect = viewport_size.rect(term_dims);

    match viewport_size {
        ViewportSize::Regular => Rect {
            left: 0,
            top: viewport_rect.bottom() + 2,
            width: viewport_rect.width,
            height: term_dims.height - viewport_rect.height - 4,
        },
        ViewportSize::Theater => {
            log_area_rect(term_dims, ViewportSize::Regular).set_width(viewport_rect.width / 2)
        }
        ViewportSize::Fullscreen => {
            log_area_rect(term_dims, ViewportSize::Theater).set_top(viewport_rect.bottom() + 1)
        }
    }
}

/// Rectangle in which to draw the sidebar
fn sidebar_rect(term_dims: Dims, viewport_size: ViewportSize) -> Rect {
    let viewport_rect = viewport_size.rect(term_dims);

    match viewport_size {
        ViewportSize::Regular => Rect {
            left: viewport_rect.width + V_SCROLLBAR_WIDTH + 1,
            top: HEADER_HEIGHT + 1,
            width: term_dims.width - viewport_rect.width - 2,
            height: term_dims.height - HEADER_HEIGHT,
        },
        ViewportSize::Theater => Rect {
            left: sidebar_rect(term_dims, ViewportSize::Regular).left,
            top: viewport_rect.bottom() + 2,
            width: sidebar_rect(term_dims, ViewportSize::Regular).width,
            height: term_dims.height - viewport_rect.height - 4,
        },
        ViewportSize::Fullscreen => Rect {
            left: sidebar_rect(term_dims, ViewportSize::Regular).left,
            top: term_dims.height,
            width: sidebar_rect(term_dims, ViewportSize::Regular).width,
            height: 1,
        },
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
    palette: Palette,
    unicode: bool,
    confirm_turn_end: bool,

    /// Whether or not to use Crossterm's alternate screen. Useful to disable this when debugging messages are desired.
    use_alt_screen: bool,

    /// Sender by which to send sound events to the audio thread (if not quieted)
    audio_thread_tx: Option<SyncSender<Sounds>>,

    /// Receiver by which to get input events from the input thread
    input_thread_rx: Mutex<Receiver<KeyEvent>>,

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
    ) -> Result<Self, crossterm::ErrorKind> {
        let (width, height) = terminal_size()?;
        let term_dims = Dims { width, height };
        // let term_dims = Dims::new(120, 60);

        let viewport_size = ViewportSize::Regular;
        let viewport_rect = viewport_size.rect(term_dims);
        let sidebar_rect = sidebar_rect(term_dims, viewport_size);

        let map = Map::new(viewport_rect, map_dims, unicode);

        // The scroller has the same dimensions as the scrolled, just draws over it
        let map_scroller = Scroller::new(viewport_rect, map);

        let log_rect = log_area_rect(term_dims, viewport_size);
        let log = LogArea::new(log_rect);

        let cp_rect = current_player_rect();
        let current_player = CurrentPlayer::new(cp_rect);

        // The input thread
        let (input_thread_tx, input_thread_rx) = channel();
        let input_thread_handle = thread::Builder::new()
            .name("input".to_string())
            .spawn(move || {
                enable_raw_mode().unwrap();

                loop {
                    match read_event() {
                        Ok(event) => {
                            match event {
                                Event::FocusGained => {}
                                Event::FocusLost => {}
                                Event::Paste(_) => {}
                                Event::Key(key_event) => {
                                    let will_return =
                                        key_event.code == KeyCode::Char(conf::KEY_QUIT);
                                    input_thread_tx.send(key_event).unwrap();

                                    if will_return {
                                        break;
                                    }
                                }
                                Event::Mouse(_mouse_event) => {}
                                Event::Resize(_columns, _rows) => {
                                    //TODO Handle resize events
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("Error reading event: {}", err);
                            break;
                        }
                    }
                }

                disable_raw_mode().unwrap();
            })
            .unwrap();

        // The audio thread (if applicable)
        let (audio_thread_handle, audio_thread_tx) = if !quiet {
            let (tx, rx) = sync_channel(2048);
            let handle = thread::Builder::new()
                .name("audio".to_string())
                .spawn(move || {
                    play_sounds(rx, Sounds::Silence).unwrap();
                })
                .unwrap();
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

            palette,

            unicode,

            confirm_turn_end,

            use_alt_screen,

            audio_thread_tx,
            input_thread_rx: Mutex::new(input_thread_rx),
            _audio_thread_handle: audio_thread_handle,
            _input_thread_handle: input_thread_handle,
        };

        ui.clear();

        Ok(ui)
    }

    fn clear(&mut self) {
        // write!(self.stdout, "{}", clear::All).unwrap();
        // self.stdout.queue(Clear(ClearType::All));
        queue!(
            self.stdout,
            Clear(ClearType::All),
            SetBackgroundColor(self.palette.get_single(Colors::Background))
        )
        .unwrap();

        // for x in 0..self.term_dims.width {
        //     for y in 0..self.term_dims.height {
        //         // write!(self.stdout, "{}{} ", goto(x,y), Bg(Rgb(0,0,0))).unwrap();
        //         queue!(self.stdout, goto(x,y), Output(String::from(" "))).unwrap();
        //     }
        // }
    }

    async fn set_viewport_size(
        &mut self,
        game: &PlayerTurn<'_>,
        viewport_size: ViewportSize,
    ) -> IoResult<()> {
        self.viewport_size = viewport_size;

        self.clear();

        self.log
            .set_rect(log_area_rect(self.term_dims, self.viewport_size));

        self.map_scroller
            .set_rect(self.viewport_size.rect(self.term_dims));

        self.sidebar_buf
            .set_rect(sidebar_rect(self.term_dims, self.viewport_size));

        self.sidebar_buf.dirty();

        self.draw(game).await
    }

    async fn draw_located_observations(
        &mut self,
        game: &PlayerTurn<'_>,
        located_obs: &[LocatedObs],
    ) -> IoResult<()> {
        for located_obs in located_obs {
            if let Some(viewport_loc) = self
                .map_scroller
                .scrollable
                .map_to_viewport_coords(located_obs.loc)
            {
                // let (city,unit) = if let Obs::Observed{ref tile,..} = located_obs.item {
                //     (Some(tile.city.as_ref()), Some(tile.unit.as_ref()))
                // } else {
                //     (Some(None),Some(None))
                // };

                self.map_scroller.scrollable.draw_tile_no_flush(
                    game,
                    &mut self.stdout,
                    viewport_loc,
                    false,
                    false,
                    None,
                    None,
                    None,
                    Some(&located_obs.obs),
                    &self.palette,
                )?;
            }
        }
        Ok(())
    }

    async fn animate_combat<A: CombatCapable + Sym, D: CombatCapable + Sym>(
        &mut self,
        game: &PlayerTurn<'_>,
        outcome: &CombatOutcome<A, D>,
        attacker_loc: Location,
        defender_loc: Location,
    ) -> IoResult<()> {
        let map = &mut self.map_scroller.scrollable;

        let attacker_viewport_loc = map.map_to_viewport_coords(attacker_loc);
        let defender_viewport_loc = map.map_to_viewport_coords(defender_loc);
        let attacker_sym = outcome.attacker().sym(self.unicode);
        let defender_sym = outcome.defender().sym(self.unicode);

        for damage_recipient in outcome.received_damage_sequence() {
            let viewport_loc = match *damage_recipient {
                CombatParticipant::Attacker => attacker_viewport_loc,
                CombatParticipant::Defender => defender_viewport_loc,
            };
            let sym = match *damage_recipient {
                CombatParticipant::Attacker => attacker_sym,
                CombatParticipant::Defender => defender_sym,
            };

            if let Some(viewport_loc) = viewport_loc {
                map.draw_tile_and_flush(
                    game,
                    &mut self.stdout,
                    viewport_loc,
                    true,
                    false,
                    None,
                    None,
                    Some(sym),
                    None,
                    &self.palette,
                )?;
                sleep_millis(100);
                map.draw_tile_and_flush(
                    game,
                    &mut self.stdout,
                    viewport_loc,
                    false,
                    false,
                    None,
                    None,
                    Some(sym),
                    None,
                    &self.palette,
                )?;
            } else {
                sleep_millis(100);
            }
        }

        Ok(())
    }

    fn ensure_map_loc_visible(&mut self, map_loc: Location) {
        self.map_scroller
            .scrollable
            .center_viewport_if_not_visible(map_loc);
    }

    fn map(&self) -> &Map {
        &self.map_scroller.scrollable
    }
}

impl LogTarget for TermUI {
    fn log_message<T>(&mut self, message: T)
    where
        Message: From<T>,
    {
        self.log.log_message(message);
    }

    fn replace_message<T>(&mut self, message: T)
    where
        Message: From<T>,
    {
        self.log.replace_message(message);
    }
}

#[async_trait]
impl MoveAnimator for TermUI {
    async fn animate_move(&mut self, game: &PlayerTurn, move_result: &Move) -> IoResult<()> {
        let mut current_loc = move_result.starting_loc;

        self.ensure_map_loc_visible(current_loc);
        self.draw(game).await?;

        for (move_idx, move_) in move_result.components.iter().enumerate() {
            let target_loc = move_.loc;
            self.ensure_map_loc_visible(current_loc);

            //FIXME This draw is revealing current game state when we really need to show the past few steps of game state involved with this move
            // self.draw_no_flush(game);

            let mut was_combat = false;
            if let Some(ref combat) = move_.unit_combat {
                self.animate_combat(game, combat, current_loc, target_loc)
                    .await?;
                was_combat = true;
            }

            if let Some(ref combat) = move_.city_combat {
                self.animate_combat(game, combat, current_loc, target_loc)
                    .await?;
                was_combat = true;
            }

            if move_.distance_moved() > 0 {
                self.log_message(Message {
                    text: format!(
                        "Unit {} {}",
                        move_result.unit,
                        if move_.moved_successfully() {
                            if was_combat {
                                "victorious"
                            } else {
                                "moved successfully"
                            }
                        } else {
                            "destroyed"
                        }
                    ),
                    mark: Some('*'),
                    fg_color: Some(Colors::Combat),
                    bg_color: None,
                    source: Some(MessageSource::UI),
                });
            }

            self.draw_located_observations(game, &move_.observations_after_move)
                .await?;

            current_loc = target_loc;

            self.stdout.flush().unwrap();

            if move_idx < move_result.components.len() - 1 {
                sleep_millis(100);
            }
        }

        if move_result.unit.moves_remaining() == 0 {
            sleep_millis(250);
        }

        Ok(())
    }
}

#[async_trait]
impl UI for TermUI {
    fn viewport_rect(&self) -> Rect {
        self.viewport_size.rect(self.term_dims)
    }

    fn viewport_size(&self) -> ViewportSize {
        self.viewport_size
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
        RectBuffer::clear(&mut self.sidebar_buf);
    }

    async fn cursor_map_loc(&self, mode: &Mode, game: &PlayerTurn) -> Option<Location> {
        match *mode {
            Mode::SetProduction { city_loc } => Some(city_loc),
            Mode::GetUnitOrders { unit_id, .. } => {
                let unit_loc = game.player_unit_loc(unit_id).await.unwrap();
                Some(unit_loc)
            }
            _ => None,
        }
    }

    async fn cursor_viewport_loc(&self, mode: &Mode, game: &PlayerTurn) -> Option<Location> {
        let map = &self.map_scroller.scrollable;

        match *mode {
            Mode::SetProduction { city_loc } => map.map_to_viewport_coords(city_loc),
            Mode::GetUnitOrders { unit_id, .. } => {
                let unit_loc = game.player_unit_loc(unit_id).await.unwrap();
                map.map_to_viewport_coords(unit_loc)
            }
            _ => None,
        }
    }

    async fn current_player_map_tile<'a>(
        &self,
        ctrl: &'a PlayerTurn,
        viewport_loc: Location,
    ) -> Option<Cow<'a, Tile>> {
        self.map_scroller
            .scrollable
            .current_player_tile(ctrl, viewport_loc)
            .await
    }

    async fn draw_current_player(&mut self, ctrl: &PlayerTurn) -> IoResult<()> {
        self.current_player
            .draw(ctrl, &mut self.stdout, &self.palette)
            .await
    }

    async fn draw_log(&mut self, ctrl: &PlayerTurn) -> IoResult<()> {
        self.log.draw(ctrl, &mut self.stdout, &self.palette).await // this will flush
    }

    async fn draw_map(&mut self, ctrl: &PlayerTurn) -> IoResult<()> {
        self.map_scroller
            .draw(ctrl, &mut self.stdout, &self.palette)
            .await
    }

    fn confirm_turn_end(&self) -> bool {
        self.confirm_turn_end
    }

    async fn draw(&mut self, game: &PlayerTurn) -> IoResult<()> {
        self.draw_no_flush(game).await?;
        self.stdout.flush()
    }

    async fn draw_map_tile_and_flush(
        &mut self,
        game: &PlayerTurn,
        viewport_loc: Location,
        highlight: bool,   // Highlighting as for a cursor
        unit_active: bool, // Indicate that the unit (if present) is active, i.e. ready to respond to orders

        // Pretend the given city is actually at this location (instead of what might really be there)
        city_override: Option<Option<&City>>,

        // Pretend the given unit is actually at this location (instead of what might really be there)
        unit_override: Option<Option<&Unit>>,

        // A symbol to display instead of what's really here
        symbol_override: Option<&str>,

        // Override the entire observation that would be at this location, instead of using the current player's
        // observations.
        obs_override: Option<&Obs>,
    ) -> IoResult<()> {
        self.map_scroller.scrollable.draw_tile_and_flush(
            game,
            &mut self.stdout,
            viewport_loc,
            highlight,
            unit_active,
            city_override,
            unit_override,
            symbol_override,
            obs_override,
            &self.palette,
        )
    }

    async fn draw_no_flush(&mut self, game: &PlayerTurn) -> IoResult<()> {
        // write!(self.stdout, "{}{}{}{}",
        //     // termion::clear::All,
        //     goto(0,0),
        //     termion::style::Underline,
        //     conf::APP_NAME,
        //     StrongReset::new(&self.palette)
        // ).unwrap();
        queue!(
            self.stdout,
            MoveTo(0, 0),
            SetAttribute(Attribute::Underlined),
            Print(conf::APP_NAME.to_string()),
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(self.palette.get_single(Colors::Background))
        )?;

        self.log
            .draw_no_flush(game, &mut self.stdout, &self.palette)
            .await?;
        self.current_player
            .draw_no_flush(game, &mut self.stdout, &self.palette)
            .await?;
        self.map_scroller
            .draw_no_flush(game, &mut self.stdout, &self.palette)
            .await?;
        self.turn
            .draw_no_flush(game, &mut self.stdout, &self.palette)
            .await?;
        self.sidebar_buf
            .draw_no_flush(game, &mut self.stdout, &self.palette)
            .await?;

        // write!(self.stdout, "{}{}", StrongReset::new(&self.palette), termion::cursor::Hide).unwrap();
        queue!(
            self.stdout,
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(self.palette.get_single(Colors::Background)),
            Hide
        )?;

        Ok(())
    }

    /// Block until a key is pressed; return that key
    fn get_key(&self) -> Result<KeyEvent, RecvError> {
        self.input_thread_rx.lock().unwrap().recv()
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

    async fn rotate_viewport_size(&mut self, game: &PlayerTurn) -> IoResult<()> {
        let new_size = match self.viewport_size {
            ViewportSize::Regular => ViewportSize::Theater,
            ViewportSize::Theater => ViewportSize::Fullscreen,
            ViewportSize::Fullscreen => ViewportSize::Regular,
        };

        self.set_viewport_size(game, new_size).await?;
        self.draw(game).await
    }

    fn scroll_map_relative<V: Into<Vec2d<i32>>>(&mut self, direction: V) {
        self.map_scroller
            .scrollable
            .scroll_relative(direction.into());
    }

    fn set_sidebar_row(&mut self, row_idx: usize, row: String) {
        self.sidebar_buf.set_row(row_idx, row)
    }

    async fn viewport_to_map_coords(
        &self,
        game: &PlayerTurn,
        viewport_loc: Location,
    ) -> Option<Location> {
        self.map().viewport_to_map_coords(game, viewport_loc)
    }
}

#[async_trait]
impl TurnTaker for TermUI {
    async fn take_turn(
        &mut self,
        ctrl: &mut PlayerTurn,
        datagen_prob: Option<f64>,
        _device: AiDevice,
    ) -> TurnOutcome {
        if datagen_prob.is_some() {
            eprintln!("TermUI doesn't generate training data but generate_data was true");
            //FIXME Code smell: refused bequest
        }

        let mut prev_mode: Option<Mode> = None;
        let mut mode = self::mode::Mode::TurnStart;

        let mut quit = false;

        loop {
            match mode.run(ctrl, self, &mut prev_mode).await {
                ModeStatus::TurnOver => break,
                ModeStatus::Quit => {
                    quit = true;
                    break;
                }
                ModeStatus::Continue => {
                    // keep on truckin'
                }
            }
        }

        TurnOutcome {
            training_instances: None, // TODO: Propagate training data from the terminal UI
            quit,
        }
    }
}

impl Drop for TermUI {
    fn drop(&mut self) {
        if self.use_alt_screen {
            queue!(self.stdout, LeaveAlternateScreen).unwrap();
            queue!(self.stdout, Show).unwrap();
        }

        if let Some(ref tx) = self.audio_thread_tx {
            tx.send(Sounds::Silence).unwrap();
        }

        // if audio_thread_handle.is_some() {
        //     ui.audio_thread_tx.unwrap().send(Sounds::Silence).unwrap();
        // }
    }
}
