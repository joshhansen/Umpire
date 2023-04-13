//! AlphaGo Zero style action model
//!
//! Based on self-play game outcomes, learn P(victory|action; environment)
//!
//! Divided into two sub-models, one for city actions, one for unit actions
use async_trait::async_trait;

use tch::{nn::VarStore, Device, Tensor};

use common::{
    game::{
        action::{NextCityAction, NextUnitAction},
        ai::TrainingInstance,
        player::PlayerTurn,
        turn_async::ActionwiseTurnTaker2,
    },
    util::densify,
};

use crate::dnn::DNN;

/// The number of outcomes to model: victory, defeat, and draw
const OUTCOMES: i64 = 3;

pub struct AgzActionModel {
    city_actions: Vec<DNN>,
    unit_actions: Vec<DNN>,
}

impl AgzActionModel {
    pub fn new(device: Device, learning_rate: f64) -> Result<Self, String> {
        let possible_city_actions = NextCityAction::possible();
        let possible_unit_actions = NextUnitAction::possible();

        Ok(Self {
            city_actions: (0..possible_city_actions)
                .map(|_| DNN::with_varstore(VarStore::new(device), learning_rate, 1).unwrap())
                .collect(),
            unit_actions: (0..possible_unit_actions)
                .map(|_| DNN::with_varstore(VarStore::new(device), learning_rate, 1).unwrap())
                .collect(),
        })
    }

    pub fn train(&mut self, data: &Vec<TrainingInstance>) {
        for datum in data {
            let target = datum.outcome.unwrap().to_training_target();

            let features = densify(datum.num_features, &datum.features);

            let features: Vec<f32> = features.iter().map(|x| *x as f32).collect();

            let features = Self::_tensor(features);

            if let Ok(city_action) = NextCityAction::try_from(datum.action) {
                let city_action_idx: usize = city_action.into();

                self.city_actions[city_action_idx].train(features, &0, target);
            } else {
                let unit_action = NextUnitAction::try_from(datum.action).unwrap();

                let unit_action_idx: usize = unit_action.into();

                self.unit_actions[unit_action_idx].train(features, &0, target);
            }
        }
    }

    async fn features(turn: &PlayerTurn<'_>) -> Vec<f32> {
        turn.player_features()
            .await
            .iter()
            .map(|x| *x as f32)
            .collect()
    }

    async fn features_tensor(turn: &PlayerTurn<'_>) -> Tensor {
        Self::_tensor(Self::features(turn).await)
    }

    fn _tensor(vec: Vec<f32>) -> Tensor {
        Tensor::try_from(vec)
            .unwrap()
            .to_device(Device::cuda_if_available())
    }
}

#[async_trait]
impl ActionwiseTurnTaker2 for AgzActionModel {
    async fn next_city_action(&mut self, turn: &PlayerTurn) -> Option<NextCityAction> {
        let feats = Self::features_tensor(turn).await;

        let city_action_idx = self
            .city_actions
            .iter()
            .map(|dnn| dnn.evaluate_tensor(&feats, &0))
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
            .0;

        Some(NextCityAction::try_from(city_action_idx).unwrap())
    }

    async fn next_unit_action(&mut self, turn: &PlayerTurn) -> Option<NextUnitAction> {
        let feats = Self::features_tensor(turn).await;

        let unit_action_idx = self
            .unit_actions
            .iter()
            .map(|dnn| dnn.evaluate_tensor(&feats, &0))
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
            .0;

        Some(NextUnitAction::try_from(unit_action_idx).unwrap())
    }
}
