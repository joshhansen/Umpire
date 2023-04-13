//! AlphaGo Zero style action model
//!
//! Based on self-play game outcomes, learn P(victory|action; environment)
//!
//! Divided into two sub-models, one for city actions, one for unit actions
use async_trait::async_trait;

use tch::{Device, Tensor};

use common::game::{
    action::{AiPlayerAction, NextCityAction, NextUnitAction},
    ai::TrainingOutcome,
    player::PlayerTurn,
    turn_async::ActionwiseTurnTaker2,
};

use crate::{dnn::DNN, Loadable, Storable};

pub struct AgzDatum {
    pub features: Tensor,
    pub action: AiPlayerAction,
    pub outcome: TrainingOutcome,
}

pub struct AgzActionModel {
    device: Device,
    city_actions: Vec<DNN>,
    unit_actions: Vec<DNN>,
}

impl AgzActionModel {
    pub fn new(device: Device, learning_rate: f64) -> Result<Self, String> {
        let possible_city_actions = NextCityAction::possible();
        let possible_unit_actions = NextUnitAction::possible();

        Ok(Self {
            device,
            city_actions: (0..possible_city_actions)
                .map(|_| DNN::new(device, learning_rate, 1).unwrap())
                .collect(),
            unit_actions: (0..possible_unit_actions)
                .map(|_| DNN::new(device, learning_rate, 1).unwrap())
                .collect(),
        })
    }

    pub fn train(&mut self, data: &Vec<AgzDatum>) {
        for datum in data {
            let target = datum.outcome.to_training_target();

            if let Ok(city_action) = NextCityAction::try_from(datum.action) {
                let city_action_idx: usize = city_action.into();

                self.city_actions[city_action_idx].train(&datum.features, &0, target);
            } else {
                let unit_action = NextUnitAction::try_from(datum.action).unwrap();

                let unit_action_idx: usize = unit_action.into();

                self.unit_actions[unit_action_idx].train(&datum.features, &0, target);
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
}

#[async_trait]
impl ActionwiseTurnTaker2 for AgzActionModel {
    async fn next_city_action(&mut self, turn: &PlayerTurn) -> Option<NextCityAction> {
        let feats = Self::features(turn).await;

        let feats = Tensor::try_from(feats).unwrap().to_device(self.device);

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
        let feats = Self::features(turn).await;

        let feats = Tensor::try_from(feats).unwrap().to_device(self.device);

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

impl Storable for AgzActionModel {
    fn store(self, path: &std::path::Path) -> Result<(), String> {
        todo!()
    }
}

impl Loadable for AgzActionModel {
    fn load<P: AsRef<std::path::Path>>(path: P) -> Result<Self, String> {
        todo!()
    }
}
