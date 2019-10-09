use crossterm::{
    KeyEvent,
};

use crate::{
    conf,
    game::{
        Game,
        unit::UnitType,
    },
    log::{LogTarget,Message,MessageSource},
    ui::{
        audio::Sounds,
        buf::RectBuffer,
        Draw,
        TermUI,
        sym::Sym,
    },
    util::{Location,Rect},
};

use super::{
    IMode,
    IVisibleMode,
    KeyStatus,
    Mode,
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

    fn write_buf(&self, game: &Game, ui: &mut TermUI) {
        let tile = &game.current_player_tile(self.loc).unwrap();
        let city = tile.city.as_ref().unwrap();

        let buf = ui.sidebar_buf_mut();
        buf.clear();
        buf.set_row(0, format!("Set Production for {}", city));

        let mut highest_y = 0;

        for (i,unit_type) in game.valid_productions(self.loc).iter().enumerate() {
            let y = i + 2;
            let row = self.row(unit_type.key(), unit_type.sym(self.unicode), unit_type.name(), Some(unit_type.cost()));
            buf.set_row(y, row);
            highest_y = y;
        }

        let row = self.row(conf::KEY_NO_PRODUCTION, " ", "None", None);
        buf.set_row(highest_y + 2, row);
    }
}

impl IMode for SetProductionMode {
    fn run(&self, game: &mut Game, ui: &mut TermUI, mode: &mut Mode, _prev_mode: &Option<Mode>) -> bool {
        ui.map_scroller.scrollable.center_viewport(self.loc);

        ui.play_sound(Sounds::Silence);

        self.write_buf(game, ui);
        ui.draw_no_flush(game);


        let city = {
            let city = game.current_player_city_by_loc(self.loc).unwrap();
            ui.log_message(format!("Requesting production target for {}", city.short_desc() ));
            ui.log.draw_no_flush(game, &mut ui.stdout, &ui.palette);

            city
        };
        let city_viewport_loc = ui.map_scroller.scrollable.map_to_viewport_coords(city.loc).unwrap();
        ui.map_scroller.scrollable.draw_tile_and_flush(game, &mut ui.stdout, city_viewport_loc, false, true, Some(Some(city)), None, None);

        loop {
            match self.get_key(game, ui, mode) {
                KeyStatus::Unhandled(key) => {
                    if let KeyEvent::Char(c) = key {
                        if let Some(unit_type) = UnitType::from_key(c) {
                            game.set_production(self.loc, unit_type).unwrap();

                            let city = &game.current_player_city_by_loc(self.loc).unwrap();
                            ui.log_message(Message {
                                text: format!("Set {}'s production to {}", city.short_desc(), unit_type),
                                mark: Some('·'),
                                bg_color: None,
                                fg_color: None,
                                source: Some(MessageSource::Mode)
                            });
                            ui.log.draw(game, &mut ui.stdout, &ui.palette);


                            Self::clear_buf(ui);

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

    fn buf_mut(ui: &mut TermUI) -> &mut RectBuffer {
        ui.sidebar_buf_mut()
    }
}