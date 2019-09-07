use conf::key_desc;
use std::convert::TryFrom;
use std::io::{Write, stdin};

use termion::color::Color;
use termion::cursor::Goto;
use termion::event::Key;
use termion::input::TermRead;

use conf;
use game::{AlignedMaybe,Game,GameError};
use log::{LogTarget,Message,MessageSource};
use map::Tile;
use map::newmap::UnitID;
use ui::{Draw,MoveAnimator,TermUI,sidebar_rect};
use ui::scroll::ScrollableComponent;
use ui::sym::Sym;
use unit::{UnitType};
use unit::orders::Orders;
use util::{Direction,Location,Rect,WRAP_NEITHER};

fn get_key() -> Key {
    let stdin = stdin();
    stdin.keys().next().unwrap().unwrap()
}

#[derive(Clone,Copy,Debug)]
pub enum Mode {
    TurnStart,
    TurnResume,
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
        most_recently_active_unit_id: UnitID
    }
}

impl Mode {
    /// Return true if the UI should continue after this mode runs, false if it should quit
    pub fn run<C:Color+Copy,W:Write>(&mut self, game: &mut Game, ui: &mut TermUI<C,W>, prev_mode: &mut Option<Mode>) -> bool {
        let continue_ = match *self {
            Mode::TurnStart =>          TurnStartMode{}.run(game, ui, self, prev_mode),
            Mode::TurnResume =>         TurnResumeMode{}.run(game, ui, self, prev_mode),
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
    Unhandled(Key)
}

trait IMode {
    /// Return true if the UI should continue after this mode runs, false if it should quit
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, ui: &mut TermUI<C,W>, mode: &mut Mode, prev_mode: &Option<Mode>) -> bool;

    fn get_key<C:Color+Copy,W:Write>(&self, game: &Game, ui: &mut TermUI<C,W>, mode: &mut Mode) -> KeyStatus {
        let key = get_key();
        if let Key::Char(c) = key {
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
                    if let Some(cursor_viewport_loc) = ui.cursor_viewport_loc(mode, game) {
                        let most_recently_active_unit_loc = ui.cursor_map_loc(mode, game).unwrap();
                        let most_recently_active_unit_id = game.unit_by_loc(most_recently_active_unit_loc).unwrap().id;

                        *mode = Mode::Examine{
                            cursor_viewport_loc,
                            first: true,
                            most_recently_active_unit_id
                        };
                        return KeyStatus::Handled(StateDisposition::Next);
                    } else {
                        ui.log_message(String::from("Couldn't get cursor loc"));
                    }
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

    fn map_loc_to_viewport_loc<C:Color+Copy,W:Write>(ui: &mut TermUI<C,W>, map_loc: Location) -> Option<Location> {
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

    fn clear<W:Write>(&self, stdout: &mut W) {
        let rect = self.rect();
        let blank_string = (0..rect.width).map(|_| " ").collect::<String>();
        for y in 0..rect.height {
            write!(*stdout, "{}{}", self.goto(0, y), blank_string).unwrap();
        }
    }
}

pub struct TurnStartMode {}
impl IMode for TurnStartMode {
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
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
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
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

        if let Ok(_player_num) = game.end_turn(ui) {
            *mode = Mode::TurnStart;
        }

        true
    }
}

struct SetProductionsMode{}
impl IMode for SetProductionsMode {
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {

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
    fn draw<W:Write>(&self, game: &Game, stdout: &mut W) {
        let tile = &game.current_player_tile(self.loc).unwrap();
        let city = tile.city.as_ref().unwrap();

        write!(*stdout, "{}Set Production for {}          ", self.goto(0, 0), city).unwrap();

        for (i,unit_type) in game.valid_productions(self.loc).iter().enumerate() {
            let y = i as u16 + 2;

            let mut char_and_name = format!(" [{}] {} - {}", unit_type.key(), unit_type.sym(self.unicode), unit_type.name());
            while char_and_name.len() < PROD_COL_WIDTH as usize {
                char_and_name.push(' ');
            }

            write!(*stdout, "{}{}",
                self.goto(0, y),
                char_and_name).unwrap();
            write!(*stdout, "{}[{}]       ",
                self.goto(PROD_COL_WIDTH, y),
                unit_type.cost()).unwrap();
        }

        stdout.flush().unwrap();
    }
}

impl IMode for SetProductionMode {
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        ui.map_scroller.scrollable.center_viewport(self.loc);

        ui.draw(game);
        self.draw(game, &mut ui.stdout);

        {
            let city = game.city_by_loc(self.loc).unwrap();
            ui.log_message(format!("Requesting production target for {}", city ));
        }

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {
                    if let Key::Char(c) = key {
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
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, _ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
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
    fn draw<W:Write>(&self, game: &Game, stdout: &mut W) {
        let unit = game.unit_by_id(self.unit_id).unwrap();

        write!(*stdout, "{}Get Orders for {}", self.goto(0, 0), unit).unwrap();
        write!(*stdout,
"\
{}Move: ↖ ↗          {} {}
{}       ← ↓ ↑ →      {} {} {} {}
{}      ↙ ↘          {} {}",
            self.goto(0, 2), conf::KEY_UP_LEFT, conf::KEY_UP_RIGHT,
            self.goto(0, 3), conf::KEY_LEFT, conf::KEY_DOWN, conf::KEY_UP, conf::KEY_RIGHT,
            self.goto(0, 4), conf::KEY_DOWN_LEFT, conf::KEY_DOWN_RIGHT).unwrap();

        write!(*stdout, "{}Examine:\t{}", self.goto(0, 6), conf::KEY_EXAMINE).unwrap();

        write!(*stdout, "{}Explore:\t{}", self.goto(0, 8), conf::KEY_EXPLORE).unwrap();

        write!(*stdout, "{}Skip:\t{}", self.goto(0, 10), key_desc(conf::KEY_SKIP)).unwrap();

        write!(*stdout, "{}Sentry:\t{}", self.goto(0, 12), conf::KEY_SENTRY).unwrap();

        write!(*stdout, "{}Quit:\t{}", self.goto(0, 14), conf::KEY_QUIT).unwrap();

        stdout.flush().unwrap();
    }
}
impl IMode for GetUnitOrdersMode {
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        let unit_loc = {
            let unit = game.unit_by_id(self.unit_id).unwrap();
            ui.log_message(format!("Requesting orders for unit {} at {}", unit, unit.loc));
            unit.loc
        };

        if self.first_move {
            ui.map_scroller.scrollable.center_viewport(unit_loc);
        }
        ui.draw(game);

        self.draw(game, &mut ui.stdout);

        let viewport_loc = ui.map_scroller.scrollable.map_to_viewport_coords(unit_loc, ui.viewport_rect().dims()).unwrap();
        ui.map_scroller.scrollable.draw_tile(game, &mut ui.stdout, viewport_loc, false, true, None);

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {

                    if let Key::Char(c) = key {
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
                            game.give_orders(self.unit_id, Some(Orders::Skip), ui, false).unwrap();
                            *mode = Mode::GetOrders;
                            self.clear(&mut ui.stdout);
                            return true;
                        } else if c == conf::KEY_SENTRY {
                            ui.log_message("Going sentry");
                            game.give_orders(self.unit_id, Some(Orders::Sentry), ui, false).unwrap();
                            *mode = Mode::GetOrders;
                            self.clear(&mut ui.stdout);
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
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, _ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        if let Some(unit_id) = game.units_with_pending_orders().next() {
            let unit = game.unit_by_id(unit_id).unwrap();
            if unit.moves_remaining > 0 {
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
    fn draw<W:Write>(&self, game: &Game, stdout: &mut W) {
        let unit = game.unit_by_id(self.unit_id).unwrap();

        write!(*stdout, "{}Unit {} carries out its orders", self.goto(0, 0), unit).unwrap();

        // write!(*stdout, "{}Cancel:\tESC", self.goto(0, 2)).unwrap();

        // write!(*stdout, "{}Examine:\t{}", self.goto(0, 4), conf::KEY_EXAMINE).unwrap();

        // write!(*stdout, "{}Quit:\t{}", self.goto(0, 6), conf::KEY_QUIT).unwrap();

        stdout.flush().unwrap();
    }
}
impl IMode for CarryOutUnitOrdersMode {
    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        let unit = game.unit_by_id(self.unit_id).unwrap();

        ui.map_scroller.scrollable.center_viewport(unit.loc);

        ui.draw(game);

        self.draw(game, &mut ui.stdout);

        let orders = unit.orders().unwrap().clone();

        match orders.carry_out(self.unit_id, game, ui) {
            Ok(_orders_status) => {
                *mode = Mode::CarryOutOrders{};
            },
            Err(msg) => {
                panic!(msg);
            }
        }

        self.clear(&mut ui.stdout);

        true
    }
}














struct QuitMode {}
impl IMode for QuitMode {
    fn run<C:Color+Copy,W:Write>(&self, _game: &mut Game, ui: &mut TermUI<C,W>, _mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        ui.cleanup();
        false
    }
}

struct ExamineMode {
    cursor_viewport_loc: Location,
    first: bool,
    most_recently_active_unit_id: UnitID
}
impl ExamineMode {
    fn clean_up<C:Color+Copy,W:Write>(&self, game: &Game, ui: &mut TermUI<C,W>) {
        let map = &mut ui.map_scroller.scrollable;
        map.draw_tile(game, &mut ui.stdout, self.cursor_viewport_loc, false, false, None);
        ui.stdout.flush().unwrap();
    }

    fn current_player_tile<'a, C:Color+Copy, W:Write>(&'a self, game: &'a Game, ui: &TermUI<C,W>) -> Option<&'a Tile> {
        let map = &ui.map_scroller.scrollable;
        map.current_player_tile(game, self.cursor_viewport_loc)
    }

    fn draw_tile<'a, C:Color+Copy,W:Write>(&'a self, game: &'a Game, ui: &mut TermUI<C,W>) {
        let map = &mut ui.map_scroller.scrollable;
        map.draw_tile(game, &mut ui.stdout, self.cursor_viewport_loc, true, false, None);
    }
}
impl IMode for ExamineMode {


    fn run<C:Color+Copy,W:Write>(&self, game: &mut Game, ui: &mut TermUI<C,W>, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
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
                if key==Key::Esc {
                    *mode = Mode::TurnResume;
                } else if key==Key::Char(conf::KEY_EXAMINE_SELECT) {

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

                    ui.log_message("Might move unit".to_string());
                    let (can_move, dest) = {
                        let unit = game.unit_by_id(self.most_recently_active_unit_id).unwrap();

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
                        game.give_orders(self.most_recently_active_unit_id, Some(Orders::GoTo{dest}), ui, true).unwrap();
                        ui.log_message(format!("Ordered unit to go to {}", dest));

                        *mode = Mode::TurnResume;

                        self.clean_up(game, ui);
                        return true;
                    }
                } else if let Key::Char(c) = key {
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
