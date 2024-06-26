use async_trait::async_trait;

use common::game::{
    action::{NextCityAction, NextUnitAction},
    ai::AiDevice,
    player::PlayerTurn,
    turn_async::ActionwiseTurnTaker2,
    unit::UnitType,
};

pub struct SkipAI;

#[async_trait]
impl ActionwiseTurnTaker2 for SkipAI {
    async fn next_city_action(
        &mut self,
        turn: &PlayerTurn,
        _device: AiDevice,
    ) -> Option<NextCityAction> {
        if let Some(city_loc) = turn.player_production_set_requests().await.first() {
            let valid_productions: Vec<UnitType> =
                turn.valid_productions_conservative(*city_loc).await;

            let unit_type = valid_productions[0];

            // if self.verbosity > 2 {
            //     println!("{:?} -> {:?}", city_loc, unit_type);
            // }

            return Some(NextCityAction::SetProduction { unit_type });
        }

        None
    }

    async fn next_unit_action(
        &mut self,
        turn: &PlayerTurn,
        _device: AiDevice,
    ) -> Option<NextUnitAction> {
        if !turn.player_unit_orders_requests().await.is_empty() {
            return Some(NextUnitAction::Skip);
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
            ai::AiDevice,
            alignment::Alignment,
            map::{terrain::Terrain, MapData},
            player::PlayerControl,
            turn_async::TurnTaker,
            Game,
        },
        util::{Dims, Location, Wrap2d},
    };

    use super::SkipAI;

    #[tokio::test]
    pub async fn test_skip_ai() {
        let device: AiDevice = Default::default();
        {
            let mut ai = SkipAI {};

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
    }
}
