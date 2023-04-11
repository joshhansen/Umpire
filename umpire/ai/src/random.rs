use std::io::stdout;

use async_trait::async_trait;

use crossterm::{cursor::MoveTo, execute};

use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};

use common::{
    game::{player::PlayerTurn, turn_async::ActionwiseTurnTaker, unit::UnitType},
    util::Direction,
};

use super::AiPlayerAction;

const P_DISBAND: f64 = 0.01;
const P_SKIP: f64 = 0.1;
const P_MOVE: f64 = 1f64 - P_DISBAND - P_SKIP;

pub struct RandomAI {
    rng: StdRng,
    verbosity: usize,
    fix_output_loc: bool,
}
impl RandomAI {
    pub fn new(verbosity: usize, fix_output_loc: bool) -> Self {
        Self {
            rng: StdRng::from_rng(rand::thread_rng()).unwrap(),
            verbosity,
            fix_output_loc,
        }
    }
}

#[async_trait]
impl ActionwiseTurnTaker for RandomAI {
    async fn next_action(&mut self, ctrl: &PlayerTurn) -> Option<AiPlayerAction> {
        let mut stdout = stdout();

        if let Some(city_loc) = ctrl.player_production_set_requests().await.iter().next() {
            let valid_productions: Vec<UnitType> =
                ctrl.valid_productions_conservative(*city_loc).await;

            let unit_type = valid_productions.choose(&mut self.rng).unwrap();

            if self.verbosity > 2 {
                println!("{:?} -> {:?}", city_loc, unit_type);
            }

            return Some(AiPlayerAction::SetNextCityProduction {
                unit_type: *unit_type,
            });
        }

        if let Some(unit_id) = ctrl
            .player_unit_orders_requests()
            .await
            .iter()
            .cloned()
            .next()
        {
            let unit = ctrl.player_unit_by_id(unit_id).await.unwrap();
            // let unit_id = unit.id;

            // let possible: Vec<Location> = match ctrl.current_player_unit_legal_one_step_destinations(unit_id) {
            //     Ok(it) => it,
            //     Err(e) => {
            //         let tile = ctrl.current_player_tile(unit.loc);
            //         panic!("Error getting destinations for unit with orders request: {}\nunit: {:?}\ntile: {:?}\ntile unit: {:?}\ntile city: {:?}",
            //                e, unit, tile, tile.as_ref().map(|t| t.unit.as_ref()), tile.as_ref().map(|t| t.city.as_ref()))
            //     }
            // }.drain().collect();

            let possible: Vec<Direction> = match ctrl.player_unit_legal_directions(unit_id).await {
                Ok(it) => it,
                Err(e) => {
                    let tile = ctrl.tile(unit.loc).await;
                    panic!("Error getting destinations for unit with orders request: {}\nunit: {:?}\ntile: {:?}\ntile unit: {:?}\ntile city: {:?}",
                           e, unit, tile, tile.as_ref().map(|t| t.unit.as_ref()), tile.as_ref().map(|t| t.city.as_ref()))
                }
            };

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

            let x: f64 = self.rng.gen();

            if self.fix_output_loc {
                execute!(stdout, MoveTo(60, 3)).unwrap();
            }

            if x <= move_prob {
                let direction = possible.choose(&mut self.rng).unwrap();

                if self.verbosity > 1 {
                    println!("{:?} {} -> {:?}", unit_id, unit.loc, direction);
                }

                return Some(AiPlayerAction::MoveNextUnit {
                    direction: *direction,
                });
            } else if x <= move_prob + skip_prob {
                if self.verbosity > 1 {
                    println!("Random skipped unit: {:?}", unit_id);
                }
                // ctrl.order_unit_skip(unit_id).unwrap();
                return Some(AiPlayerAction::SkipNextUnit);
            } else {
                if self.verbosity > 1 {
                    let loc = ctrl.player_unit_loc(unit_id).await.unwrap();
                    println!("Random disbanded unit: {:?} at location {}", unit_id, loc);
                }
                return Some(AiPlayerAction::DisbandNextUnit);
            }
        }

        None
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::sync::RwLock as RwLockTokio;

    use common::{
        game::{
            alignment::Alignment,
            map::{gen::generate_map, terrain::Terrain, MapData},
            player::PlayerControl,
            turn_async::TurnTaker,
            unit::UnitID,
            Game, IGame,
        },
        name::IntNamer,
        util::{Dims, Location, Wrap2d},
    };

    use super::RandomAI;

    #[tokio::test]
    pub async fn test_random_ai() {
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

            let (game, secrets) = Game::new_with_map(map, 1, true, None, Wrap2d::BOTH);

            let game = Arc::new(RwLockTokio::new(game));

            let mut ctrl = PlayerControl::new(game, 0, secrets[0]).await;

            for _ in 0..1000 {
                let mut turn = ctrl.turn_ctrl();
                ai.take_turn(&mut turn, false).await;
            }
        }

        let mut ai = RandomAI::new(2, false);

        for r in 0..1000 {
            let players = 2;
            let mut city_namer = IntNamer::new("city");
            let map = generate_map(&mut city_namer, Dims::new(5, 5), players);
            let (game, secrets) = Game::new_with_map(map, players, true, None, Wrap2d::BOTH);

            let game = Arc::new(RwLockTokio::new(game));

            let mut ctrls: Vec<PlayerControl> = Vec::with_capacity(players);
            for player in 0..players {
                ctrls.push(
                    PlayerControl::new(
                        Arc::clone(&game) as Arc<RwLockTokio<dyn IGame>>,
                        player,
                        secrets[player],
                    )
                    .await,
                );
            }

            for i in 0..300 {
                for player in 0..=1 {
                    let ctrl = &mut ctrls[player];

                    {
                        let mut turn = ctrl.turn_ctrl();

                        ai.take_turn(&mut turn, false).await;
                    }

                    let orders_requests: Vec<UnitID> = ctrl.player_unit_orders_requests().await;

                    for rqst_unit_id in orders_requests.iter().cloned() {
                        // Assert that all orders requests correspond to units still present and that the IDs still
                        // match
                        let unit = ctrl.player_unit_by_id(rqst_unit_id).await.expect(
                            format!("Unit not found in iteration {}, round {}", i, r).as_str(),
                        );

                        assert_eq!(unit.id, rqst_unit_id);
                    }
                }

                if game.read().await.victor().is_some() {
                    break;
                }
            }
        }
    }

    #[tokio::test]
    async fn test_random_ai_carried_unit_destruction() {
        // Load an infantry unit into a transport, then try to get the transport destroyed by the random AI. This was
        // causing issues because RandomAI cached the list of unit orders requests, but it could go stale when a
        // carried unit was destroyed

        let mut map = MapData::try_from("Kti").unwrap();

        let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
        let infantry_id = map.toplevel_unit_id_by_loc(Location::new(2, 0)).unwrap();

        map.carry_unit_by_id(transport_id, infantry_id).unwrap();

        let players = 2;

        let (game, secrets) = Game::new_with_map(map, players, true, None, Wrap2d::BOTH);

        let game = Arc::new(RwLockTokio::new(game));

        let mut ctrls: Vec<PlayerControl> = Vec::with_capacity(players);
        for player in 0..players {
            ctrls.push(
                PlayerControl::new(
                    Arc::clone(&game) as Arc<RwLockTokio<dyn IGame>>,
                    player,
                    secrets[player],
                )
                .await,
            );
        }

        let mut ai = RandomAI::new(0, false);

        for _ in 0..1000 {
            let game = game.clone();

            if game.read().await.current_player() == 0 {
                let ctrl = &mut ctrls[0];
                let _turn = ctrl.turn_ctrl();
                // drop this to end first player's turn without moving the infantry or transport
            } else {
                let ctrl = &mut ctrls[1];
                let mut turn = ctrl.turn_ctrl();

                ai.take_turn(&mut turn, false).await;
            }
        }
    }
}
