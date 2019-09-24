use std::convert::TryFrom;
use std::io::Write;

use crossterm::{
    KeyEvent,
};

use crate::{
    conf,
    game::{
        AlignedMaybe,
        Game,
        GameError,
        map::Tile,
        map::newmap::UnitID,
    },
    log::LogTarget,
    ui::{
        Draw,
        MoveAnimator,
        TermUI,
    },
    util::{Direction,Location,WRAP_NEITHER},
};

use super::{
    IMode,
    KeyStatus,
    Mode,
    StateDisposition,
};

pub struct ExamineMode {
    pub cursor_viewport_loc: Location,
    pub first: bool,
    pub most_recently_active_unit_id: Option<UnitID>
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