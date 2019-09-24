use std::convert::TryFrom;
use std::io::{Stdout, Write};

use crossterm::{
    ErrorKind,
    Goto,
    KeyEvent,
    Output,
    queue,
};

use crate::{
    color::Colors,
    conf::{self, key_desc},
    game::{
        AlignedMaybe,
        Game,
        GameError,
        PlayerNum,
        map::Tile,
        map::newmap::UnitID,
        unit::{
            UnitType,
        },
    },
    log::{LogTarget,Message,MessageSource},
    ui::{
        audio::Sounds,
        Draw,
        MoveAnimator,
        TermUI,
        sidebar_rect,
        scroll::ScrollableComponent,
        sym::Sym,
    },
    util::{Direction,Location,Rect,WRAP_NEITHER},
};

#[derive(Clone,Copy,Debug)]
pub enum Mode {
    TurnStart,
    TurnResume,
    TurnOver,
    SetProductions,
    SetProduction{city_loc:Location},
    GetOrders,
    GetUnitOrders{unit_id:UnitID, first_move:bool},
    CarryOutOrders,
    CarryOutUnitOrders{unit_id:UnitID},
    Quit,
    Examine{
        cursor_viewport_loc:Location,
        first: bool,
        most_recently_active_unit_id: Option<UnitID>
    }
}

impl Mode {
    /// Return true if the UI should continue after this mode runs, false if it should quit
    pub fn run(&mut self, game: &mut Game, ui: &mut TermUI, prev_mode: &mut Option<Mode>) -> bool {
        let continue_ = match *self {
            Mode::TurnStart =>          TurnStartMode{}.run(game, ui, self, prev_mode),
            Mode::TurnResume =>         TurnResumeMode{}.run(game, ui, self, prev_mode),
            Mode::TurnOver   =>         TurnOverMode{}.run(game, ui, self, prev_mode),
            Mode::SetProductions =>     SetProductionsMode{}.run(game, ui, self, prev_mode),
            Mode::SetProduction{city_loc} => {
                let viewport_rect = ui.viewport_rect();
                let rect = sidebar_rect(viewport_rect, ui.term_dims);
                SetProductionMode{rect, loc:city_loc, unicode: ui.unicode}.run(game, ui, self, prev_mode)
            },
            Mode::GetOrders =>          GetOrdersMode{}.run(game, ui, self, prev_mode),
            Mode::GetUnitOrders{unit_id,first_move} =>      {
                let viewport_rect = ui.viewport_rect();
                let rect = sidebar_rect(viewport_rect, ui.term_dims);
                GetUnitOrdersMode{rect, unit_id, first_move}.run(game, ui, self, prev_mode)
            },
            Mode::CarryOutOrders =>     CarryOutOrdersMode{}.run(game, ui, self, prev_mode),
            Mode::CarryOutUnitOrders{unit_id} => {
                let viewport_rect = ui.viewport_rect();
                let rect = sidebar_rect(viewport_rect, ui.term_dims);
                CarryOutUnitOrdersMode{rect, unit_id}.run(game, ui, self, prev_mode)
            },
            Mode::Quit =>               QuitMode{}.run(game, ui, self, prev_mode),
            Mode::Examine{cursor_viewport_loc, first, most_recently_active_unit_id} =>
                ExamineMode{cursor_viewport_loc, first, most_recently_active_unit_id}.run(game, ui, self, prev_mode)
        };

        *prev_mode = Some(*self);

        continue_
    }
}

#[derive(PartialEq)]
enum StateDisposition {
    Stay,
    Next,
    Quit
}

enum KeyStatus {
    Handled(StateDisposition),
    Unhandled(KeyEvent)
}

trait IMode {
    /// Return true if the UI should continue after this mode runs, false if it should quit
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, prev_mode: &Option<Mode>) -> bool;

    fn get_key(&self, game: &Game, ui: &mut TermUI, mode: &mut Mode) -> KeyStatus {
        let key = ui.get_key();
        if let KeyEvent::Char(c) = key {
            if let Ok(dir) = Direction::try_from_viewport_shift(c) {
                ui.map_scroller.scrollable.scroll_relative(dir.vec2d());
                ui.map_scroller.draw(game, &mut ui.stdout, &ui.palette);
                return KeyStatus::Handled(StateDisposition::Stay);
            }

            match c {
                conf::KEY_QUIT => {
                    *mode = Mode::Quit;
                    return KeyStatus::Handled(StateDisposition::Quit);
                },
                conf::KEY_EXAMINE => {
                    // println!("Rect: {:?}", ui.viewport_rect());
                    // println!("Center: {:?}", ui.viewport_rect().center());

                    let cursor_viewport_loc = ui.cursor_viewport_loc(mode, game).unwrap_or(
                        ui.viewport_rect().center()
                    );

                    let most_recently_active_unit_id =
                        if let Some(most_recently_active_unit_loc) = ui.cursor_map_loc(mode, game) {
                            game.unit_by_loc(most_recently_active_unit_loc).map(|unit| unit.id)
                        } else {
                            None
                        }
                    ;

                    *mode = Mode::Examine{
                        cursor_viewport_loc,
                        first: true,
                        most_recently_active_unit_id
                    };
                    return KeyStatus::Handled(StateDisposition::Next);
                },
                conf::KEY_VIEWPORT_SIZE_ROTATE => {
                    ui.rotate_viewport_size(game);
                    return KeyStatus::Handled(StateDisposition::Stay);
                },
                _ => {}
            }
        }
        KeyStatus::Unhandled(key)
    }

    fn map_loc_to_viewport_loc(ui: &mut TermUI, map_loc: Location) -> Option<Location> {
        let viewport_dims = ui.map_scroller.viewport_dims();
        let map = &ui.map_scroller.scrollable;
        map.map_to_viewport_coords(map_loc, viewport_dims)
    }
}

trait IVisibleMode: IMode {
    fn rect(&self) -> Rect;

    fn goto(&self, x: u16, y: u16) -> Goto {
        let rect = self.rect();
        Goto(rect.left + x + 1, rect.top + y + 1)
    }

    fn clear(&self, stdout: &mut Stdout) {
        let rect = self.rect();
        let blank_string = (0..rect.width).map(|_| " ").collect::<String>();
        for y in 0..rect.height {
            // write!(*stdout, "{}{}", self.goto(0, y), blank_string).unwrap();
            queue!(stdout, self.goto(0, y), Output(blank_string.clone())).unwrap();//FIXME clear widget without cloning repeatedly
        }
    }
}

pub struct TurnStartMode {}
impl IMode for TurnStartMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        ui.current_player.draw(game, &mut ui.stdout, &ui.palette);

        ui.log_message(Message {
            text: format!("Turn {}, player {} go!", game.turn(), game.current_player()),
            mark: Some('_'),
            fg_color: None,
            bg_color: None,
            source: Some(MessageSource::Mode)
        });

        *mode = Mode::TurnResume;

        true
    }
}

struct TurnResumeMode{}
impl IMode for TurnResumeMode {
    fn run(&self, game: &mut Game, _ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        // Process production set requests
        if game.production_set_requests().next().is_some() {
            *mode = Mode::SetProductions;
            return true;
        }

        if game.units_with_pending_orders().next().is_some() {
            *mode = Mode::CarryOutOrders;
            return true;
        }

        if game.unit_orders_requests().next().is_some() {
            *mode = Mode::GetOrders;
            return true;
        }

        if game.turn_is_done() {
            *mode = Mode::TurnOver;
        }

        true
    }
}

struct TurnOverMode {}
impl IMode for TurnOverMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {

        let over_for: PlayerNum = game.current_player();

        if ui.confirm_turn_end() {
            ui.log_message(Message {
                text: format!("Turn over for player {}. Press Enter to continue.", over_for),
                mark: Some('X'),
                fg_color: Some(Colors::Text),
                bg_color: None,
                source: None
            });

            loop {
                match self.get_key(game, ui, mode) {
                    KeyStatus::Unhandled(key) => {
                        if let KeyEvent::Char('\n') = key {

                            // If the user has altered productions using examine mode then the turn might not be over anymore
                            // Recheck

                            match game.end_turn(ui) {
                                Ok(_over_for) => {
                                    *mode = Mode::TurnStart;
                                },
                                Err(_not_over_for) => {
                                    *mode = Mode::TurnResume;
                                }
                            }

                            return true;
                        }
                    },
                    KeyStatus::Handled(state_disposition) => {
                        match state_disposition {
                            StateDisposition::Quit => return false,
                            StateDisposition::Next => return true,
                            StateDisposition::Stay => {}
                        }
                    }
                }
            }
        } else {
            // We shouldn't be in this state unless game.turn_is_done() is true
            // so this unwrap should always succeed
            game.end_turn(ui).unwrap();
            *mode = Mode::TurnStart;
            true
        }
    }
}

struct SetProductionsMode{}
impl IMode for SetProductionsMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {

        if game.production_set_requests().next().is_none() {
            ui.log_message("Productions set.".to_string());
            *mode = Mode::TurnResume;
            return true;
        }

        let city_loc = game.production_set_requests().next().unwrap();

        *mode = Mode::SetProduction{city_loc};
        true
    }
}

const PROD_COL_WIDTH: u16 = 21;
struct SetProductionMode {
    loc: Location,
    rect: Rect,
    unicode: bool,
}
impl SetProductionMode {
    fn char_and_name(&self, key: char, sym: &'static str, name: &'static str) -> String {
        let mut char_and_name = format!(" [{}] {} - {}", key, sym, name);
        while char_and_name.len() < PROD_COL_WIDTH as usize {
            char_and_name.push(' ');
        }
        char_and_name
    }

    fn write_row(&self, stdout: &mut Stdout, key: char, sym: &'static str, name: &'static str, cost: Option<u16>, y: u16) -> Result<(),ErrorKind> {
        let char_and_name = self.char_and_name(key, sym, name);

        // write!(*stdout, "{}{}",
        //         self.goto(0, y),
        //         char_and_name).unwrap();
        queue!(stdout, self.goto(0, y), Output(char_and_name)).unwrap();
        
        if let Some(cost) = cost {
            // write!(*stdout, "{}[{}]       ",
            //     self.goto(PROD_COL_WIDTH, y),
            //     cost)
            queue!(stdout, self.goto(PROD_COL_WIDTH, y), Output(format!("[{}]       ", cost)))
        } else {
            Ok(())
        }
    }

    fn draw(&self, game: &Game, stdout: &mut Stdout) {
        let tile = &game.current_player_tile(self.loc).unwrap();
        let city = tile.city.as_ref().unwrap();

        // write!(*stdout, "{}Set Production for {}          ", self.goto(0, 0), city).unwrap();
        queue!(stdout, self.goto(0, 0), Output(format!("Set Production for {}          ", city))).unwrap();

        
        let mut highest_y = 0;

        for (i,unit_type) in game.valid_productions(self.loc).iter().enumerate() {
            let y = i as u16 + 2;
            self.write_row(stdout, unit_type.key(), unit_type.sym(self.unicode), unit_type.name(), Some(unit_type.cost()), y).unwrap();

            // let mut char_and_name = format!(" [{}] {} - {}", unit_type.key(), unit_type.sym(self.unicode), unit_type.name());
            // while char_and_name.len() < PROD_COL_WIDTH as usize {
            //     char_and_name.push(' ');
            // }
            // let mut char_and_name = self.char_and_name(unit_type);

            // write!(*stdout, "{}{}",
            //     self.goto(0, y),
            //     char_and_name).unwrap();
            // write!(*stdout, "{}[{}]       ",
            //     self.goto(PROD_COL_WIDTH, y),
            //     unit_type.cost()).unwrap();

            highest_y = y;
        }

        self.write_row(stdout, conf::KEY_NO_PRODUCTION, " ", "None", None, highest_y + 2).unwrap();

        stdout.flush().unwrap();
    }
}

impl IMode for SetProductionMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        ui.map_scroller.scrollable.center_viewport(self.loc);

        ui.play_sound(Sounds::Silence);
        ui.draw(game);
        self.draw(game, &mut ui.stdout);

        {
            let city = game.city_by_loc(self.loc).unwrap();
            ui.log_message(format!("Requesting production target for {}", city ));
        }

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {
                    if let KeyEvent::Char(c) = key {
                        if let Some(unit_type) = UnitType::from_key(c) {
                            game.set_production(self.loc, unit_type).unwrap();

                            let city = &game.city_by_loc(self.loc).unwrap();
                            ui.replace_message(Message {
                                text: format!("Set {}'s production to {}", city.name(), unit_type),
                                mark: Some('·'),
                                bg_color: None,
                                fg_color: None,
                                source: Some(MessageSource::Mode)
                            });

                            self.clear(&mut ui.stdout);

                            *mode = Mode::TurnResume;
                            return true;
                        } else if c == conf::KEY_NO_PRODUCTION {
                            
                            if game.player_cities_producing_or_not_ignored() <= 1 {
                                game.clear_production_without_ignoring(self.loc).unwrap();
                                // let cursor_viewport_loc = ui.cursor_viewport_loc(mode, game).unwrap();

                                // *mode = Mode::Examine {
                                //     cursor_viewport_loc,
                                //     first: true,
                                //     most_recently_active_unit_id: None,
                                // };

                            } else {
                                // game.set_production(self.loc, None).unwrap();
                                game.clear_production_and_ignore(self.loc).unwrap();
                                
                            }

                            *mode = Mode::TurnResume;
                            return true;
                        }
                    }
                },
                KeyStatus::Handled(state_disposition) => {
                    match state_disposition {
                        StateDisposition::Quit => return false,
                        StateDisposition::Next => return true,
                        StateDisposition::Stay => {}
                    }
                }
            }
        }
    }
}

impl IVisibleMode for SetProductionMode {
    fn rect(&self) -> Rect {
        self.rect
    }
}

struct GetOrdersMode {}
impl IMode for GetOrdersMode {
    fn run(&self, game: &mut Game, _ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        if let Some(unit_id) = game.unit_orders_requests().next() {
            *mode = Mode::GetUnitOrders{unit_id, first_move:true};
        } else {
            *mode = Mode::TurnResume;
        }
        
        true
    }
}

struct GetUnitOrdersMode{
    rect: Rect,
    unit_id: UnitID,
    first_move: bool
}
impl IVisibleMode for GetUnitOrdersMode {
    fn rect(&self) -> Rect {
        self.rect
    }
}
impl GetUnitOrdersMode {
    fn draw(&self, game: &Game, stdout: &mut Stdout) {
        let unit = game.unit_by_id(self.unit_id).unwrap();

//         write!(*stdout, "{}Get Orders for {}", self.goto(0, 0), unit).unwrap();
//         write!(*stdout,
// "\
// {}Move: ↖ ↗          {} {}
// {}       ← ↓ ↑ →      {} {} {} {}
// {}      ↙ ↘          {} {}",
//             self.goto(0, 2), conf::KEY_UP_LEFT, conf::KEY_UP_RIGHT,
//             self.goto(0, 3), conf::KEY_LEFT, conf::KEY_DOWN, conf::KEY_UP, conf::KEY_RIGHT,
//             self.goto(0, 4), conf::KEY_DOWN_LEFT, conf::KEY_DOWN_RIGHT).unwrap();

//         write!(*stdout, "{}Examine:\t{}", self.goto(0, 6), conf::KEY_EXAMINE).unwrap();

//         write!(*stdout, "{}Explore:\t{}", self.goto(0, 8), conf::KEY_EXPLORE).unwrap();

//         write!(*stdout, "{}Skip:\t{}", self.goto(0, 10), key_desc(conf::KEY_SKIP)).unwrap();

//         write!(*stdout, "{}Sentry:\t{}", self.goto(0, 12), conf::KEY_SENTRY).unwrap();

//         write!(*stdout, "{}Quit:\t{}", self.goto(0, 14), conf::KEY_QUIT).unwrap();

        queue!(stdout,
            self.goto(0, 0), Output(format!("Get Orders for {}", unit)),
            self.goto(0, 2), Output(format!("Move: ↖ ↗          {} {}", conf::KEY_UP_LEFT, conf::KEY_UP_RIGHT)),
            self.goto(0, 3), Output(format!("       ← ↓ ↑ →      {} {} {} {}", conf::KEY_LEFT, conf::KEY_DOWN, conf::KEY_UP, conf::KEY_RIGHT)),
            self.goto(0, 4), Output(format!("      ↙ ↘          {} {}", conf::KEY_DOWN_LEFT, conf::KEY_DOWN_RIGHT)),
            self.goto(0, 6), Output(format!("Examine:\t{}", conf::KEY_EXAMINE)),
            self.goto(0, 8), Output(format!("Explore:\t{}", conf::KEY_EXPLORE)),
            self.goto(0, 10), Output(format!("Skip:\t{}", key_desc(conf::KEY_SKIP))),
            self.goto(0, 12), Output(format!("Sentry:\t{}", conf::KEY_SENTRY)),
            self.goto(0, 14), Output(format!("Quit:\t{}", conf::KEY_QUIT))
        ).unwrap();

        stdout.flush().unwrap();
    }
}
impl IMode for GetUnitOrdersMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        let (unit_loc,unit_type) = {
            let unit = game.unit_by_id(self.unit_id).unwrap();
            ui.log_message(format!("Requesting orders for unit {} at {}", unit, unit.loc));
            (unit.loc,unit.type_)
        };

        if self.first_move {
            ui.play_sound(Sounds::Unit(unit_type));
            ui.map_scroller.scrollable.center_viewport(unit_loc);
        }
        ui.draw(game);

        self.draw(game, &mut ui.stdout);

        let viewport_loc = ui.map_scroller.scrollable.map_to_viewport_coords(unit_loc, ui.viewport_rect().dims()).unwrap();
        ui.map_scroller.scrollable.draw_tile(game, &mut ui.stdout, viewport_loc, false, true, None);

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {

                    if let KeyEvent::Char(c) = key {
                        if let Ok(dir) = Direction::try_from(c) {
                            if let Some(dest) = unit_loc.shift_wrapped(dir, game.map_dims(), game.wrapping()) {
                                match game.move_unit_by_id(self.unit_id, dest) {
                                    Ok(move_result) => {
                                        ui.animate_move(game, &move_result);

                                        if game.unit_orders_requests().any(|unit_id| unit_id==self.unit_id) {
                                            *mode = Mode::GetUnitOrders{unit_id:self.unit_id, first_move:false};
                                        } else {
                                            *mode = Mode::GetOrders;
                                        }
                                        
                                        self.clear(&mut ui.stdout);
                                        return true;
                                    },
                                    Err(msg) => {
                                        ui.log_message(format!("Error: {}", msg));
                                    }
                                }
                            }
                        } else if c == conf::KEY_SKIP {
                            game.order_unit_skip(self.unit_id).unwrap();
                            // game.give_orders(self.unit_id, Some(Orders::Skip), ui, false).unwrap();
                            *mode = Mode::GetOrders;
                            self.clear(&mut ui.stdout);
                            return true;
                        } else if c == conf::KEY_SENTRY {
                            ui.log_message("Going sentry");
                            // game.give_orders(self.unit_id, Some(Orders::Sentry), ui, false).unwrap();
                            game.order_unit_sentry(self.unit_id).unwrap();
                            *mode = Mode::GetOrders;
                            self.clear(&mut ui.stdout);
                            return true;
                        } else if c == conf::KEY_EXPLORE {
                            let outcome = game.order_unit_explore(self.unit_id).unwrap();
                            if let Some(move_result) = outcome.move_result() {
                                ui.animate_move(game, &move_result);
                            }
                            *mode = Mode::GetOrders;
                            return true;
                        }
                    }
                },
                KeyStatus::Handled(state_disposition) => {
                    match state_disposition {
                        StateDisposition::Quit => return false,
                        StateDisposition::Next => return true,
                        StateDisposition::Stay => {}
                    }
                }
            }
        }
    }
}

struct CarryOutOrdersMode {}
impl IMode for CarryOutOrdersMode {
    fn run(&self, game: &mut Game, _ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        if let Some(unit_id) = game.units_with_pending_orders().next() {
            let unit = game.unit_by_id(unit_id).unwrap();
            if unit.moves_remaining() > 0 {
                *mode = Mode::CarryOutUnitOrders{unit_id};
            }
        } else {
            *mode = Mode::TurnResume;
        }

        true
    }
}

struct CarryOutUnitOrdersMode {
    rect: Rect,
    unit_id: UnitID,
}

impl IVisibleMode for CarryOutUnitOrdersMode {
    fn rect(&self) -> Rect {
        self.rect
    }
}
impl CarryOutUnitOrdersMode {
    fn draw(&self, game: &Game, stdout: &mut Stdout) {
        let unit = game.unit_by_id(self.unit_id).unwrap();

        // write!(*stdout, "{}Unit {} carries out its orders", self.goto(0, 0), unit).unwrap();
        queue!(stdout, self.goto(0, 0), Output(format!("Unit {} carries out its orders", unit))).unwrap();

        // write!(*stdout, "{}Cancel:\tESC", self.goto(0, 2)).unwrap();

        // write!(*stdout, "{}Examine:\t{}", self.goto(0, 4), conf::KEY_EXAMINE).unwrap();

        // write!(*stdout, "{}Quit:\t{}", self.goto(0, 6), conf::KEY_QUIT).unwrap();

        stdout.flush().unwrap();
    }
}
impl IMode for CarryOutUnitOrdersMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        let unit = game.unit_by_id(self.unit_id).unwrap();

        ui.map_scroller.scrollable.center_viewport(unit.loc);

        ui.draw(game);

        self.draw(game, &mut ui.stdout);

        let orders = unit.orders.as_ref().unwrap();

        match orders.carry_out(self.unit_id, game) {
            Ok(orders_outcome) => {
                if let Some(move_result) = orders_outcome.move_result() {
                    ui.animate_move(game, &move_result);
                }
                *mode = Mode::CarryOutOrders{};
            },
            Err(msg) => {
                // panic!(msg);
                ui.log_message(Message {
                    text: msg,
                    mark: Some('!'),
                    fg_color: Some(Colors::Text),
                    bg_color: Some(Colors::Notice),
                    source: None,
                });
            }
        }

        self.clear(&mut ui.stdout);

        true
    }
}














struct QuitMode {}
impl IMode for QuitMode {
    fn run(&self, _game: &mut Game, _ui: &mut TermUI, _mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        // ui.cleanup();
        false
    }
}

struct ExamineMode {
    cursor_viewport_loc: Location,
    first: bool,
    most_recently_active_unit_id: Option<UnitID>
}
impl ExamineMode {
    fn clean_up(&self, game: &Game, ui: &mut TermUI) {
        let map = &mut ui.map_scroller.scrollable;
        map.draw_tile(game, &mut ui.stdout, self.cursor_viewport_loc, false, false, None);
        ui.stdout.flush().unwrap();
    }

    fn current_player_tile<'a>(&'a self, game: &'a Game, ui: &TermUI) -> Option<&'a Tile> {
        let map = &ui.map_scroller.scrollable;
        map.current_player_tile(game, self.cursor_viewport_loc)
    }

    fn draw_tile<'a>(&'a self, game: &'a Game, ui: &mut TermUI) {
        let map = &mut ui.map_scroller.scrollable;
        map.draw_tile(game, &mut ui.stdout, self.cursor_viewport_loc, true, false, None);
    }
}
impl IMode for ExamineMode {


    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        self.draw_tile(game, ui);

        let description = {
            if let Some(tile) = self.current_player_tile(game, ui) {
                format!("{}", tile)
            } else {
                "the horrifying void of the unknown (hic sunt dracones)".to_string()
            }
        };


        let message = format!("Examining: {}", description);
        if self.first {
            ui.log_message(message);
        } else {
            ui.replace_message(message);
        }
        ui.stdout.flush().unwrap();

        match self.get_key(game, ui, mode) {
            KeyStatus::Unhandled(key) => {
                if key==KeyEvent::Esc {
                    *mode = Mode::TurnResume;
                } else if key==KeyEvent::Char(conf::KEY_EXAMINE_SELECT) {

                    if let Some(tile) = self.current_player_tile(game, ui).cloned() {// We clone to ease mutating the unit within this block
                        if let Some(ref city) = tile.city {
                            if city.belongs_to_player(game.current_player()) {
                                *mode = Mode::SetProduction{city_loc:city.loc};
                                self.clean_up(game, ui);
                                return true;
                            }
                        }

                        if let Some(ref unit) = tile.unit {
                            if unit.belongs_to_player(game.current_player()) {
                                
                                // Since the unit we get from this tile may be a "memory" of an old observation, get the most recent one in order to activate it

                                match game.activate_unit_by_loc(unit.loc) {
                                    Ok(()) => {
                                        ui.log_message(format!("Activated unit {}", unit));
                                        *mode = Mode::GetUnitOrders { unit_id: unit.id, first_move: true };
                                        return true;
                                    },
                                    Err(GameError::NoSuchUnit{msg:_msg,id:_id}) => {
                                        // The unit we had must have been a stale observation since we can't find it now.
                                        // Doing nothing is fine.
                                    },
                                    Err(err) => {
                                        panic!("Unexpected error attempting to activate unit: {:?}", err);
                                    }
                                }
                            }
                        }
                    }

                    // If there was a recently active unit, see if we can give it orders to move to the current location
                    if let Some(most_recently_active_unit_id) = self.most_recently_active_unit_id {
                        ui.log_message("Might move unit".to_string());
                        let (can_move, dest) = {
                            let unit = game.unit_by_id(most_recently_active_unit_id).unwrap();

                            let can_move = if let Some(tile) = self.current_player_tile(game, ui) {
                                unit.can_move_on_tile(tile)
                            } else {
                                false
                            };
                            let dest = self.current_player_tile(game, ui).map(|tile| tile.loc);
                            (can_move, dest)
                        };

                        if can_move {
                            let dest = dest.unwrap();
                            // game.give_orders(self.most_recently_active_unit_id, Some(Orders::GoTo{dest}), ui, true).unwrap();
                            let outcome = game.order_unit_go_to(most_recently_active_unit_id, dest).unwrap();
                            if let Some(move_result) = outcome.move_result() {
                                ui.animate_move(game, &move_result);
                            }

                            ui.log_message(format!("Ordered unit to go to {}", dest));

                            *mode = Mode::TurnResume;

                            self.clean_up(game, ui);
                            return true;
                        }
                    }
                } else if let KeyEvent::Char(c) = key {
                    if let Ok(dir) = Direction::try_from(c) {

                        if let Some(new_loc) = self.cursor_viewport_loc.shift_wrapped(dir, ui.viewport_rect().dims(), WRAP_NEITHER) {
                            let viewport_rect = ui.viewport_rect();
                            if new_loc.x < viewport_rect.width && new_loc.y <= viewport_rect.height {
                                *mode = Mode::Examine{cursor_viewport_loc: new_loc, first: false, most_recently_active_unit_id: self.most_recently_active_unit_id};
                            }
                        } else {
                            // If shifting without wrapping takes us beyond the viewport then we need to shift the viewport
                            // such that the cursor will still be at its edge

                            ui.map_scroller.scrollable.shift_viewport(dir.vec2d());
                            ui.map_scroller.draw(game, &mut ui.stdout, &ui.palette);
                            // Don't change `mode` since we'll basically pick up where we left off
                        }
                    }
                }

                self.clean_up(game, ui);
                true
            },
            KeyStatus::Handled(state_disposition) => {
                match state_disposition {
                    StateDisposition::Quit => false,
                    StateDisposition::Next | StateDisposition::Stay => true
                }
            }
        }
    }
}
