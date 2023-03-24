use std::io::stdout;

use crossterm::{cursor::MoveTo, execute};

use rand::{seq::SliceRandom, Rng};

use common::{
    game::{
        player::{ActionwiseLimitedTurnTaker, PlayerTurnControl},
        unit::UnitType,
    },
    util::Direction,
};

use super::AiPlayerAction;

const P_DISBAND: f64 = 0.01;
const P_SKIP: f64 = 0.1;
const P_MOVE: f64 = 1f64 - P_DISBAND - P_SKIP;

pub struct RandomAI {
    verbosity: usize,
    fix_output_loc: bool,
}
impl RandomAI {
    pub fn new(verbosity: usize, fix_output_loc: bool) -> Self {
        Self {
            verbosity,
            fix_output_loc,
        }
    }
}

impl ActionwiseLimitedTurnTaker for RandomAI {
    fn next_action(&self, ctrl: &PlayerTurnControl) -> Option<AiPlayerAction> {
        let mut rng = rand::thread_rng();

        let mut stdout = stdout();

        if let Some(city_loc) = ctrl.production_set_requests().next() {
            let valid_productions: Vec<UnitType> =
                ctrl.valid_productions_conservative(city_loc).collect();

            let unit_type = valid_productions.choose(&mut rng).unwrap();

            if self.verbosity > 2 {
                println!("{:?} -> {:?}", city_loc, unit_type);
            }

            return Some(AiPlayerAction::SetNextCityProduction {
                unit_type: *unit_type,
            });
        }

        if let Some(unit_id) = ctrl.player_unit_orders_requests().next() {
            let unit = ctrl.player_unit_by_id(unit_id).unwrap();
            // let unit_id = unit.id;

            // let possible: Vec<Location> = match ctrl.current_player_unit_legal_one_step_destinations(unit_id) {
            //     Ok(it) => it,
            //     Err(e) => {
            //         let tile = ctrl.current_player_tile(unit.loc);
            //         panic!("Error getting destinations for unit with orders request: {}\nunit: {:?}\ntile: {:?}\ntile unit: {:?}\ntile city: {:?}",
            //                e, unit, tile, tile.as_ref().map(|t| t.unit.as_ref()), tile.as_ref().map(|t| t.city.as_ref()))
            //     }
            // }.drain().collect();

            let possible: Vec<Direction> = match ctrl.current_player_unit_legal_directions(unit_id) {
                Ok(it) => it,
                Err(e) => {
                    let tile = ctrl.tile(unit.loc);
                    panic!("Error getting destinations for unit with orders request: {}\nunit: {:?}\ntile: {:?}\ntile unit: {:?}\ntile city: {:?}",
                           e, unit, tile, tile.as_ref().map(|t| t.unit.as_ref()), tile.as_ref().map(|t| t.city.as_ref()))
                }
            }.collect();

            // // Check to be sure the source location isn't appearing in the list of destinations
            // debug_assert!(!possible.contains(
            //         ctrl.current_player_unit_loc(unit_id).as_ref().unwrap()
            //     ),
            //     "The current location {} of unit with ID {:?} appeared in list of one step destinations {:?}",
            //     ctrl.current_player_unit_loc(unit_id).as_ref().unwrap(),
            //     unit_id,
            //     possible
            // );

            // Normalization factor
            let z = if possible.is_empty() {
                P_SKIP + P_DISBAND
            } else {
                1f64
            };

            let move_prob = if possible.is_empty() { 0f64 } else { P_MOVE } / z;
            let skip_prob = P_SKIP / z;

            let x: f64 = rng.gen();

            if self.fix_output_loc {
                execute!(stdout, MoveTo(60, 3)).unwrap();
            }

            if x <= move_prob {
                let direction = possible.choose(&mut rng).unwrap();

                if self.verbosity > 1 {
                    println!("{:?} {} -> {:?}", unit_id, unit.loc, direction);
                }

                return Some(AiPlayerAction::MoveNextUnit {
                    direction: *direction,
                });

                // // println!("dest: {:?}", dest);
                // if self.verbosity > 1 {
                //     let src = ctrl.current_player_unit_loc(unit_id).unwrap();
                //     println!("{:?} {} -> {}", unit_id, src, dest);
                // }
                // let result = ctrl.move_unit_by_id(unit_id, *dest).unwrap();
                // if self.verbosity > 1 && !result.moved_successfully() {

                //     println!("Random's unit destroyed: {:?}", unit_id);
                // }

                // if self.verbosity > 1 {
                //     println!("{:?}", ctrl.current_player_observations());
                // }
            } else if x <= move_prob + skip_prob {
                if self.verbosity > 1 {
                    println!("Random skipped unit: {:?}", unit_id);
                }
                // ctrl.order_unit_skip(unit_id).unwrap();
                return Some(AiPlayerAction::SkipNextUnit);
            } else {
                if self.verbosity > 1 {
                    let loc = ctrl.player_unit_loc(unit_id).unwrap();
                    println!("Random disbanded unit: {:?} at location {}", unit_id, loc);
                }
                return Some(AiPlayerAction::DisbandNextUnit);
            }
        }

        None
    }
}

// impl LimitedTurnTaker for RandomAI {
//     fn take_turn(&mut self, game: &mut PlayerTurnControl, generate_data: bool) -> Option<Vec<TrainingInstance>> {
//         let mut rng = rand::thread_rng();

//         let mut stdout = stdout();

//         if self.verbosity > 1 {
//             if self.fix_output_loc {
//                 execute!(stdout, MoveTo(60,0)).unwrap();
//             }
//             println!("Random:\n{:?}", game.current_player_observations());

//             if self.fix_output_loc {
//                 execute!(stdout, MoveTo(60,1)).unwrap();
//             }

//             println!("Random cities: {}", game.current_player_cities().count());

//             if self.fix_output_loc {
//                 execute!(stdout, MoveTo(60,2)).unwrap();
//             }

//             println!("Random units: {}", game.current_player_units().count());
//         }

//         let training_instances = if generate_data {
//             Some(Vec::new())
//         } else {
//             None
//         };

//         let production_set_requests: Vec<Location> = game.production_set_requests().collect();
//         for city_loc in production_set_requests {
//             let valid_productions: Vec<UnitType> = game.valid_productions_conservative(city_loc).collect();

//             let unit_type = valid_productions.choose(&mut rng).unwrap();

//             if self.verbosity > 2 {
//                 println!("{:?} -> {:?}", city_loc, unit_type);
//             }

//             game.set_production_by_loc(city_loc, *unit_type).unwrap();
//         }

//         // let unit_orders_requests: Vec<UnitID> = game.unit_orders_requests().collect();
//         // let units_with_orders_requests: Vec<Unit> = game.units_with_orders_requests().cloned().collect();
//         // for unit_id in unit_orders_requests {
//         // for unit in units_with_orders_requests {

//         while game.unit_orders_requests().next().is_some() {
//             let unit_id = game.unit_orders_requests().next().unwrap();
//             let unit = game.current_player_unit_by_id(unit_id).unwrap();
//             // let unit_id = unit.id;

//             let possible: Vec<Location> = match game.current_player_unit_legal_one_step_destinations(unit_id) {
//                 Ok(it) => it,
//                 Err(e) => {
//                     let tile = game.current_player_tile(unit.loc);
//                     panic!("Error getting destinations for unit with orders request: {}\nunit: {:?}\ntile: {:?}\ntile unit: {:?}\ntile city: {:?}",
//                            e, unit, tile, tile.as_ref().map(|t| t.unit.as_ref()), tile.as_ref().map(|t| t.city.as_ref()))
//                 }
//             }.drain().collect();

//             // Check to be sure the source location isn't appearing in the list of destinations
//             debug_assert!(!possible.contains(
//                     game.current_player_unit_loc(unit_id).as_ref().unwrap()
//                 ),
//                 "The current location {} of unit with ID {:?} appeared in list of one step destinations {:?}",
//                 game.current_player_unit_loc(unit_id).as_ref().unwrap(),
//                 unit_id,
//                 possible
//             );

//             let non_disband_options = possible.len() + 1;
//             let move_prob = possible.len() as f64 / non_disband_options as f64;
//             let skip_prob = (1.0f64 / non_disband_options as f64) - P_DISBAND;

//             let x: f64 = rng.gen();

//             if self.fix_output_loc {
//                 execute!(stdout, MoveTo(60,3)).unwrap();
//             }

//             if x <= move_prob {
//                 let dest = possible.choose(&mut rng).unwrap();

//                 // println!("dest: {:?}", dest);
//                 if self.verbosity > 1 {
//                     let src = game.current_player_unit_loc(unit_id).unwrap();
//                     println!("{:?} {} -> {}", unit_id, src, dest);
//                 }
//                 let result = game.move_unit_by_id(unit_id, *dest).unwrap();
//                 if self.verbosity > 1 && !result.moved_successfully() {

//                     println!("Random's unit destroyed: {:?}", unit_id);
//                 }

//                 if self.verbosity > 1 {
//                     println!("{:?}", game.current_player_observations());
//                 }
//             } else if x <= move_prob + skip_prob {
//                 if self.verbosity > 1 {
//                     println!("Random skipped unit: {:?}", unit_id);
//                 }
//                 game.order_unit_skip(unit_id).unwrap();
//             } else {
//                 if self.verbosity > 1 {
//                     let loc = game.current_player_unit_loc(unit_id).unwrap();
//                     println!("Random disbanded unit: {:?} at location {}", unit_id, loc);
//                 }
//                 game.disband_unit_by_id(unit_id).unwrap();
//             }
//         }

//         training_instances
//     }
// }

#[cfg(test)]
mod test {
    use common::{
        game::{
            alignment::Alignment,
            map::{gen::generate_map, terrain::Terrain, MapData},
            player::LimitedTurnTaker,
            unit::UnitID,
            Game,
        },
        name::IntNamer,
        util::{Dims, Location, Wrap2d},
    };

    use super::RandomAI;

    #[test]
    pub fn test_random_ai() {
        {
            let mut ai = RandomAI::new(0, false);

            let mut map = MapData::new(Dims::new(100, 100), |_loc| Terrain::Land);
            // let unit_id = map.new_unit(Location::new(0,0), UnitType::Armor, Alignment::Belligerent{player:0}, "Forest Gump").unwrap();
            map.new_city(
                Location::new(0, 0),
                Alignment::Belligerent { player: 0 },
                "Hebevund",
            )
            .unwrap();

            let (mut game, secrets) = Game::new_with_map(map, 1, true, None, Wrap2d::BOTH);
            let (mut ctrl, _turn_start) = game.player_turn_control(secrets[0]).unwrap();

            for _ in 0..1000 {
                ai.take_turn(&mut ctrl, false);
            }
        }

        let mut ai = RandomAI::new(2, false);

        for r in 0..1000 {
            let players = 2;
            let mut city_namer = IntNamer::new("city");
            let map = generate_map(&mut city_namer, Dims::new(5, 5), players);
            let (mut game, secrets) = Game::new_with_map(map, players, true, None, Wrap2d::BOTH);

            for i in 0..300 {
                for player in 0..=1 {
                    let (mut ctrl, _turn_start) =
                        game.player_turn_control(secrets[player]).unwrap();
                    ai.take_turn(&mut ctrl, false);

                    let orders_requests: Vec<UnitID> = ctrl.player_unit_orders_requests().collect();

                    for rqst_unit_id in orders_requests.iter().cloned() {
                        // Assert that all orders requests correspond to units still present and that the IDs still
                        // match
                        let unit = ctrl.player_unit_by_id(rqst_unit_id).expect(
                            format!("Unit not found in iteration {}, round {}", i, r).as_str(),
                        );

                        assert_eq!(unit.id, rqst_unit_id);
                    }
                }

                if game.victor().is_some() {
                    break;
                }
            }
        }
    }

    #[test]
    fn test_random_ai_carried_unit_destruction() {
        // Load an infantry unit into a transport, then try to get the transport destroyed by the random AI. This was
        // causing issues because RandomAI cached the list of unit orders requests, but it could go stale when a
        // carried unit was destroyed

        let mut map = MapData::try_from("Kti").unwrap();

        let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
        let infantry_id = map.toplevel_unit_id_by_loc(Location::new(2, 0)).unwrap();

        map.carry_unit_by_id(transport_id, infantry_id).unwrap();

        let (game, secrets) = Game::new_with_map(map, 2, true, None, Wrap2d::BOTH);

        let mut ai = RandomAI::new(0, false);

        for _ in 0..1000 {
            let mut game = game.clone();

            if game.current_player() == 0 {
                let (_ctrl, _turn_start) = game.player_turn_control(secrets[0]).unwrap();
                // drop this to end first player's turn without moving the infantry or transport
            } else {
                let (mut ctrl, _turn_start) = game.player_turn_control(secrets[1]).unwrap();

                ai.take_turn(&mut ctrl, false);
            }
        }
    }
}
