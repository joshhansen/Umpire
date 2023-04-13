//! AlphaGo Zero style action model
//!
//! Based on self-play game outcomes, learn P(victory|action; environment)
//!
//! Divided into two sub-models, one for city actions, one for unit actions
use async_trait::async_trait;

use tch::{
    nn::{self, Module, ModuleT, VarStore},
    Device, Tensor,
};

use common::game::{
    action::{NextCityAction, NextUnitAction},
    ai::{fX, TrainingInstance, DEEP_HEIGHT, DEEP_LEN, DEEP_WIDTH, WIDE_LEN},
    player::PlayerTurn,
    turn_async::ActionwiseTurnTaker2,
};

use crate::dnn::DNN;

/// The number of outcomes to model: victory, defeat, and draw
const OUTCOMES: i64 = 3;


pub struct AgzActionModel {
    vars: VarStore,
    city_actions: Vec<DNN>,
    unit_actions: Vec<DNN>,
}

impl AgzActionModel {
    pub fn new(device: Device, learning_rate: f32) -> Result<Self, String> {
        let vars = VarStore::new(device);

        let possible_city_actions = NextCityAction::possible();
        let possible_unit_actions = NextUnitAction::possible();


        Ok(Self {
            vars,
            city_actions: (0..possible_city_actions).map(|_| DNN::with_varstore(vars, 1).unwrap()).collect(),
            unit_actions: (0..possible_unit_actions).map(|_| DNN::with_varstore(vars, 1).unwrap()).collect(),
        })
    }

    pub fn train_city(&mut self, data: &Vec<TrainingInstance>) {
        //TODO
    }

    pub fn train_unit(&mut self, data: &Vec<TrainingInstance>) {
        //TODO
    }

    pub fn train(&mut self, data: &Vec<TrainingInstance>) {
        for datum in data {
            if let Ok(city_action) = NextCityAction::try_from(datum.action) {

            } else {
                let unit_action = NextUnitAction::from(datum.action);
            }
        }
    }

    async fn features(turn: &PlayerTurn<'_>) -> Vec<f32> {
        turn
            .player_features()
            .await
            .iter()
            .map(|x| *x as f32)
            .collect()
    }
}

#[async_trait]
impl ActionwiseTurnTaker2 for AgzActionModel {
    async fn next_city_action(&mut self, turn: &PlayerTurn) -> Option<NextCityAction> {
        let feats = self.features(turn);

        let best_action_idx = self.city_actions.iter().max_by_key(|dnn| );
    }

    async fn next_unit_action(&mut self, turn: &PlayerTurn) -> Option<NextUnitAction> {
        unimplemented!()
    }
}
