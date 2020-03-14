use std::{
    convert::TryFrom,
};

use crossterm::event::KeyCode;

use crate::{
    color::Colors,
    game::{
        AlignedMaybe,
        GameError,
        map::Tile,
        player::{
            PlayerTurnControl,
            ProposedActionWrapper,
        },
        unit::orders::ProposedSetAndFollowOrders,
        unit::UnitID,
    },
    log::{
        LogTarget,
        Message,
        MessageSource,
    },
    ui::{
        Draw,
        MoveAnimator,
        TermUI,
    },
    util::{Direction,Location,Wrap2d},
};

use super::{
    IMode,
    KeyStatus,
    Mode,
    ModeStatus,
    StateDisposition,
};

pub(in crate::ui) struct ExamineMode {
    cursor_viewport_loc: Location,
    most_recently_active_unit_id: Option<UnitID>,
    /// This is the first examine mode state we've been in since being in non-examine-mode states
    first: bool,
}
impl ExamineMode {
    pub(in crate::ui::mode) fn new(cursor_viewport_loc: Location, most_recently_active_unit_id: Option<UnitID>, first: bool) -> Self {
        Self {
            cursor_viewport_loc,
            most_recently_active_unit_id,
            first,
        }
    }
    fn clean_up(&self, game: &PlayerTurnControl, ui: &mut TermUI) {
        let map = &mut ui.map_scroller.scrollable;
        map.draw_tile_and_flush(game, &mut ui.stdout, self.cursor_viewport_loc, false, false, 
            None, None, None, None);
    }

    /// The tile visible to the current player under the examine cursor, if any
    fn current_player_tile<'a>(&'a self, game: &'a PlayerTurnControl, ui: &TermUI) -> Option<&'a Tile> {
        let map = &ui.map_scroller.scrollable;
        map.current_player_tile(game, self.cursor_viewport_loc)
    }

    fn draw_tile<'a>(&'a self, game: &'a PlayerTurnControl, ui: &mut TermUI) {
        let map = &mut ui.map_scroller.scrollable;
        map.draw_tile_and_flush(game, &mut ui.stdout, self.cursor_viewport_loc, true, false, 
            None, None, None, None);
    }

    fn next_examine_mode(&self, new_loc: Location) -> Mode {
        Mode::Examine{
            cursor_viewport_loc: new_loc,
            most_recently_active_unit_id: self.most_recently_active_unit_id,
            first: false
        }
    }
}
impl IMode for ExamineMode {
    fn run(&self, game: &mut PlayerTurnControl, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {

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
        ui.log.draw(game, &mut ui.stdout, &ui.palette);// this will flush

        match self.get_key(game, ui, mode) {
            KeyStatus::Unhandled(key) => {
                if key.code==KeyCode::Esc {
                    // Don't leave the examine-mode log message hanging around. They accumulate and get really ugly.
                    ui.log.pop_message();

                    // Also pop the last message prior to the examine-mode message since we're going to end up back in the prior state
                    // and it will re-print the relevant message anyway
                    ui.log.pop_message();

                    // Don't flush here because the mode we resume should do so---we want to avoid flickers

                    *mode = Mode::TurnResume;

                } else if key.code==KeyCode::Enter {
                    if let Some(tile) = self.current_player_tile(game, ui).cloned() {// We clone to ease mutating the unit within this block
                        if let Some(ref city) = tile.city {
                            if city.belongs_to_player(game.current_player()) {
                                *mode = Mode::SetProduction{city_loc:city.loc};
                                self.clean_up(game, ui);
                                return ModeStatus::Continue;
                            }
                        }

                        if let Some(ref unit) = tile.unit {
                            if unit.belongs_to_player(game.current_player()) {
                                
                                // Since the unit we get from this tile may be a "memory" of an old observation, get the most recent one in order to activate it

                                match game.activate_unit_by_loc(unit.loc) {
                                    Ok(()) => {
                                        ui.log_message(format!("Activated unit {}", unit));
                                        *mode = Mode::GetUnitOrders { unit_id: unit.id, first_move: true };
                                        return ModeStatus::Continue;
                                    },
                                    Err(GameError::NoUnitAtLocation{..}) => {
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

                        let dest = ui.map().viewport_to_map_coords(game, self.cursor_viewport_loc).unwrap();

                        let proposed_outcome: ProposedActionWrapper<ProposedSetAndFollowOrders> = game.propose_order_unit_go_to(most_recently_active_unit_id, dest);

                        match proposed_outcome.item.proposed_orders_result {
                            Ok(ref proposed_orders_outcome) => {

                                if let Some(ref proposed_move) = proposed_orders_outcome.proposed_move {
                                    ui.animate_proposed_move(game, proposed_move);
                                }
                                ui.log_message(format!("Ordered unit to go to {}", dest));
                            },
                            Err(ref orders_err) => ui.log_message(Message {
                                text: format!("{}", orders_err),
                                mark: Some('-'),
                                fg_color: Some(Colors::Notice),
                                bg_color: Some(Colors::Background),
                                source: Some(MessageSource::UI),
                            })
                        };

                        // We need to actually take the action contemplated for bookkeeping reasons
                        // Make sure the outcome of actually running it is the same as expected
                        let error_expected = proposed_outcome.item.proposed_orders_result.is_err();
                        match proposed_outcome.take(game) {
                            Ok(_) => {
                                debug_assert!(!error_expected);
                            },
                            Err(_) => {
                                debug_assert!(error_expected);
                            }
                        }

                        *mode = Mode::TurnResume;

                        self.clean_up(game, ui);
                        return ModeStatus::Continue;
                    }
                } else if let KeyCode::Char(c) = key.code {
                    if let Ok(dir) = Direction::try_from(c) {

                        if let Some(new_loc) = self.cursor_viewport_loc.shift_wrapped(dir, ui.viewport_rect().dims(), Wrap2d::NEITHER) {
                            let viewport_rect = ui.viewport_rect();
                            if new_loc.x < viewport_rect.width && new_loc.y <= viewport_rect.height {
                                *mode = self.next_examine_mode(new_loc);
                            }
                        } else {
                            // If shifting without wrapping takes us beyond the viewport then we need to shift the viewport
                            // such that the cursor will still be at its edge

                            ui.map_scroller.scrollable.shift_viewport(dir.into());
                            ui.map_scroller.draw(game, &mut ui.stdout, &ui.palette);
                            // Don't change `mode` since we'll basically pick up where we left off
                        }
                    }
                }

                self.clean_up(game, ui);
                ModeStatus::Continue
            },
            KeyStatus::Handled(state_disposition) => {
                match state_disposition {
                    StateDisposition::Quit => ModeStatus::Quit,
                    StateDisposition::Next | StateDisposition::Stay => ModeStatus::Continue,
                }
            }
        }
    }
}