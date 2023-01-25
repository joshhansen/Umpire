use crossterm::event::{
    KeyCode,
};

use crate::{
    conf,
    game::{
        player::PlayerTurnControl,
        unit::UnitType,
    },
    log::{Message,MessageSource},
    ui::{
        audio::Sounds,
        UI,
        sym::Sym,
    },
    util::{Location,Rect},
};

use super::{
    IMode,
    IVisibleMode,
    KeyStatus,
    Mode,
    ModeStatus,
    StateDisposition,
    COL_WIDTH,
};

pub(in crate::ui) struct SetProductionMode {
    pub loc: Location,
    pub rect: Rect,
    pub unicode: bool,
}
impl SetProductionMode {
    fn char_and_name(key: char, sym: &'static str, name: &'static str) -> String {
        let mut char_and_name = format!(" [{}] {} - {}", key, sym, name);
        while char_and_name.len() < COL_WIDTH as usize {
            char_and_name.push(' ');
        }
        char_and_name
    }

    fn row(&self, key: char, sym: &'static str, name: &'static str, cost: Option<u16>) -> String {
        let mut row = Self::char_and_name(key, sym, name);
        if let Some(cost) = cost {
            row.push('[');
            row.push_str(format!("{}", cost).as_str());
            row.push(']');
        }
        row
    }

    fn write_buf<U:UI>(&self, game: &PlayerTurnControl, ui: &mut U) {
        let tile = &game.current_player_tile(self.loc).unwrap();
        let city = tile.city.as_ref().unwrap();

        ui.clear_sidebar();
        ui.set_sidebar_row(0, format!("Set Production for {}", city));

        let mut highest_y = 0;

        for (i,unit_type) in game.valid_productions(self.loc).enumerate() {
            let y = i + 2;
            let row = self.row(unit_type.key(), unit_type.sym(self.unicode), unit_type.name(), Some(unit_type.cost()));
            ui.set_sidebar_row(y, row);
            highest_y = y;
        }

        let row = self.row(conf::KEY_NO_PRODUCTION, " ", "None", None);
        ui.set_sidebar_row(highest_y + 2, row);
    }
}

impl IMode for SetProductionMode {
    fn run<U:UI>(&self, game: &mut PlayerTurnControl, ui: &mut U, mode: &mut Mode, _prev_mode: &Option<Mode>) -> ModeStatus {
        ui.center_map(self.loc);

        ui.play_sound(Sounds::Silence);

        self.write_buf(game, ui);
        ui.draw_no_flush(game);


        let city = {
            let city = game.current_player_city_by_loc(self.loc).unwrap();
            ui.log_message(format!("Requesting production target for {}", city.short_desc() ));
            ui.draw_no_flush(game);

            city
        };
        // let city_viewport_loc = ui.map_scroller.scrollable.map_to_viewport_coords(city.loc).unwrap();
        let city_viewport_loc = ui.map_to_viewport_coords(city.loc).unwrap();
        ui.draw_map_tile_and_flush(game, city_viewport_loc, false, true, 
            Some(Some(city)), None, None, None);

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {
                    if let KeyCode::Char(c) = key.code {
                        if let Ok(unit_type) = UnitType::try_from_key(c) {
                            game.set_production_by_loc(self.loc, unit_type).unwrap();

                            let city = game.current_player_city_by_loc(self.loc).unwrap();
                            ui.log_message(Message {
                                text: format!("Set {}'s production to {}", city.short_desc(), unit_type),
                                mark: Some('·'),
                                bg_color: None,
                                fg_color: None,
                                source: Some(MessageSource::Mode)
                            });
                            ui.draw_log(game);


                            Self::clear_buf(ui);

                            *mode = Mode::TurnResume;
                            return ModeStatus::Continue;
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
                            return ModeStatus::Continue;
                        }
                    }
                },
                KeyStatus::Handled(state_disposition) => {
                    match state_disposition {
                        StateDisposition::Quit => return ModeStatus::Quit,
                        StateDisposition::Next => return ModeStatus::Continue,
                        StateDisposition::Stay => {}
                    }
                }
            }
        }
    }
}

impl IVisibleMode for SetProductionMode {
    fn clear_buf<U:UI>(ui: &mut U) {
        ui.clear_sidebar();
    }

    fn rect(&self) -> Rect {
        self.rect
    }
}