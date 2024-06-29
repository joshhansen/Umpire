use async_trait::async_trait;

use rand::{rngs::StdRng, seq::SliceRandom, Rng};

use common::game::{
    action::{AiPlayerAction, NextCityAction, NextUnitAction},
    ai::AiDevice,
    player::PlayerTurn,
    turn_async::ActionwiseTurnTaker,
    turn_async::ActionwiseTurnTaker2,
};
use common::{game::unit::UnitType, util::Direction};

pub struct RandomAI {
    rng: StdRng,
}
impl RandomAI {
    pub fn new(rng: StdRng) -> Self {
        Self { rng }
    }
}

#[async_trait]
impl ActionwiseTurnTaker2 for RandomAI {
    async fn next_city_action(
        &mut self,
        turn: &PlayerTurn,
        _device: AiDevice,
    ) -> Option<NextCityAction> {
        let legal: Vec<NextCityAction> = turn
            .player_next_city_legal_actions()
            .await
            .into_iter()
            .collect();
        legal.choose(&mut self.rng).copied()
    }

    async fn next_unit_action(
        &mut self,
        turn: &PlayerTurn,
        _device: AiDevice,
    ) -> Option<NextUnitAction> {
        let legal: Vec<NextUnitAction> = turn
            .player_next_unit_legal_actions()
            .await
            .into_iter()
            .collect();
        legal.choose(&mut self.rng).copied()
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::sync::RwLock as RwLockTokio;

    use common::{
        game::{
            ai::AiDevice,
            alignment::Alignment,
            map::{gen::MapType, terrain::Terrain, MapData},
            player::PlayerControl,
            turn_async::TurnTaker,
            unit::UnitID,
            Game,
        },
        name::IntNamer,
        util::{init_rng, Dims, Location, Wrap2d},
    };

    use super::RandomAI;

    #[tokio::test]
    pub async fn test_random_ai() {
        let device: AiDevice = Default::default();
        {
            let mut ai = RandomAI::new(init_rng(None));

            let mut map = MapData::new(Dims::new(100, 100), |_loc| Terrain::Land);
            // let unit_id = map.new_unit(Location::new(0,0), UnitType::Armor, Alignment::Belligerent{player:0}, "Forest Gump").unwrap();
            map.new_city(
                Location::new(0, 0),
                Alignment::Belligerent { player: 0 },
                "Hebevund",
            )
            .unwrap();

            let (game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::BOTH);

            let game = Arc::new(RwLockTokio::new(game));

            let mut ctrl = PlayerControl::new(game, 0, secrets[0]).await;

            for _ in 0..1000 {
                let mut turn = ctrl.turn_ctrl(true).await;
                ai.take_turn(&mut turn, None, device).await;
                turn.force_end_turn().await.unwrap();
            }
        }

        let mut rng = init_rng(None);
        let mut ai = RandomAI::new(init_rng(None));

        for r in 0..1000 {
            let players = 2;
            let mut city_namer = IntNamer::new("city");
            let map =
                MapType::Continents.generate(&mut rng, Dims::new(5, 5), players, &mut city_namer);
            let (game, mut ctrls) =
                Game::setup_with_map(None, false, map, players, true, None, Wrap2d::BOTH).await;

            for i in 0..300 {
                for ctrl in ctrls.iter_mut() {
                    {
                        let mut turn = ctrl.turn_ctrl(true).await;

                        ai.take_turn(&mut turn, None, device).await;

                        turn.force_end_turn().await.unwrap();
                    }

                    let orders_requests: Vec<UnitID> = ctrl.player_unit_orders_requests().await;

                    for rqst_unit_id in orders_requests.iter().cloned() {
                        // Assert that all orders requests correspond to units still present and that the IDs still
                        // match
                        let unit =
                            ctrl.player_unit_by_id(rqst_unit_id)
                                .await
                                .unwrap_or_else(|| {
                                    panic!("Unit not found in iteration {}, round {}", i, r)
                                });

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

        let (_game, mut ctrls) =
            Game::setup_with_map(None, false, map, players, true, None, Wrap2d::BOTH).await;

        let rng = init_rng(None);

        let mut ai = RandomAI::new(rng);

        let device: AiDevice = Default::default();

        for _turn in 0..1000 {
            {
                let ctrl = &mut ctrls[0];
                let mut turn = ctrl.turn_ctrl(true).await;

                turn.force_end_turn().await.unwrap();
                // drop this to end first player's turn without moving the infantry or transport
            }

            {
                let ctrl = &mut ctrls[1];
                let mut turn = ctrl.turn_ctrl(true).await;

                ai.take_turn(&mut turn, None, device).await;

                turn.force_end_turn().await.unwrap();
            }
        }
    }
}

const P_DISBAND: f64 = 0.01;
const P_SKIP: f64 = 0.1;
const P_MOVE: f64 = 1f64 - P_DISBAND - P_SKIP;

pub struct RandomPlusAI {
    rng: StdRng,
}
impl RandomPlusAI {
    pub fn new(rng: StdRng) -> Self {
        Self { rng }
    }
}

#[async_trait]
impl ActionwiseTurnTaker for RandomPlusAI {
    async fn next_action(
        &mut self,
        ctrl: &PlayerTurn,
        _device: AiDevice,
    ) -> Option<AiPlayerAction> {
        if let Some(city_loc) = ctrl.player_production_set_requests().await.first() {
            let valid_productions: Vec<UnitType> =
                ctrl.valid_productions_conservative(*city_loc).await;

            let unit_type = valid_productions.choose(&mut self.rng).unwrap();

            return Some(AiPlayerAction::City(NextCityAction::SetProduction {
                unit_type: *unit_type,
            }));
        }

        if let Some(unit_id) = ctrl.player_unit_orders_requests().await.first().copied() {
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
                    let tile = ctrl.tile(unit.loc);
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

            if x <= move_prob {
                let direction = possible.choose(&mut self.rng).unwrap();

                return Some(AiPlayerAction::Unit(NextUnitAction::Move {
                    direction: *direction,
                }));
            } else if x <= move_prob + skip_prob {
                // ctrl.order_unit_skip(unit_id).unwrap();
                return Some(AiPlayerAction::Unit(NextUnitAction::Skip));
            } else {
                return Some(AiPlayerAction::Unit(NextUnitAction::Disband));
            }
        }

        None
    }
}

#[cfg(test)]
mod test2 {
    use std::sync::Arc;

    use tokio::sync::RwLock as RwLockTokio;

    use common::{
        game::{
            ai::AiDevice,
            alignment::Alignment,
            map::{gen::MapType, terrain::Terrain, MapData},
            player::PlayerControl,
            turn_async::TurnTaker,
            unit::UnitID,
            Game,
        },
        name::IntNamer,
        util::{init_rng, Dims, Location, Wrap2d},
    };

    use super::RandomPlusAI;

    #[tokio::test]
    pub async fn test_random_ai() {
        let device: AiDevice = Default::default();
        {
            let mut ai = RandomPlusAI::new(init_rng(None));

            let mut map = MapData::new(Dims::new(100, 100), |_loc| Terrain::Land);
            // let unit_id = map.new_unit(Location::new(0,0), UnitType::Armor, Alignment::Belligerent{player:0}, "Forest Gump").unwrap();
            map.new_city(
                Location::new(0, 0),
                Alignment::Belligerent { player: 0 },
                "Hebevund",
            )
            .unwrap();

            let (game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::BOTH);

            let game = Arc::new(RwLockTokio::new(game));

            let mut ctrl = PlayerControl::new(game, 0, secrets[0]).await;

            for _ in 0..1000 {
                let mut turn = ctrl.turn_ctrl(true).await;
                ai.take_turn(&mut turn, None, device).await;
                turn.force_end_turn().await.unwrap();
            }
        }

        let mut rng = init_rng(None);
        let mut ai = RandomPlusAI::new(init_rng(None));

        for r in 0..1000 {
            let players = 2;
            let mut city_namer = IntNamer::new("city");
            let map =
                MapType::Continents.generate(&mut rng, Dims::new(5, 5), players, &mut city_namer);
            let (game, mut ctrls) =
                Game::setup_with_map(None, false, map, players, true, None, Wrap2d::BOTH).await;

            for i in 0..300 {
                for ctrl in ctrls.iter_mut() {
                    {
                        let mut turn = ctrl.turn_ctrl(true).await;

                        ai.take_turn(&mut turn, None, device).await;

                        turn.force_end_turn().await.unwrap();
                    }

                    let orders_requests: Vec<UnitID> = ctrl.player_unit_orders_requests().await;

                    for rqst_unit_id in orders_requests.iter().cloned() {
                        // Assert that all orders requests correspond to units still present and that the IDs still
                        // match
                        let unit =
                            ctrl.player_unit_by_id(rqst_unit_id)
                                .await
                                .unwrap_or_else(|| {
                                    panic!("Unit not found in iteration {}, round {}", i, r)
                                });

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

        let (_game, mut ctrls) =
            Game::setup_with_map(None, false, map, players, true, None, Wrap2d::BOTH).await;

        let rng = init_rng(None);

        let mut ai = RandomPlusAI::new(rng);

        let device: AiDevice = Default::default();

        for _turn in 0..1000 {
            {
                let ctrl = &mut ctrls[0];
                let mut turn = ctrl.turn_ctrl(true).await;

                turn.force_end_turn().await.unwrap();
                // drop this to end first player's turn without moving the infantry or transport
            }

            {
                let ctrl = &mut ctrls[1];
                let mut turn = ctrl.turn_ctrl(true).await;

                ai.take_turn(&mut turn, None, device).await;

                turn.force_end_turn().await.unwrap();
            }
        }
    }
}
