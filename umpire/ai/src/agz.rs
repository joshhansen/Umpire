//! AlphaGo Zero style action model
//!
//! Based on self-play game outcomes, learn P(victory|action; environment)
//!
//! Divided into two sub-models, one for city actions, one for unit actions
use std::{collections::HashSet, fs::OpenOptions};

use async_trait::async_trait;

use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use tch::{Device, Tensor};

use common::game::{
    action::{AiPlayerAction, NextCityAction, NextUnitAction},
    ai::{TrainingFocus, TrainingOutcome},
    player::PlayerTurn,
    turn_async::ActionwiseTurnTaker2,
};

use crate::{dnn::DNN, Loadable, LoadableFromBytes, Storable, StorableAsBytes};

pub struct AgzDatum {
    pub features: Tensor,
    pub action: AiPlayerAction,
    pub outcome: TrainingOutcome,
}

#[derive(Serialize, Deserialize)]
pub struct AgzActionModelEncoding {
    city_actions: Vec<u8>,
    unit_actions: Vec<u8>,
}
impl AgzActionModelEncoding {
    pub fn decode(self, device: Device) -> Result<AgzActionModel, String> {
        Ok(AgzActionModel {
            device,
            city_actions: DNN::load_from_bytes(&self.city_actions[..])?,
            unit_actions: DNN::load_from_bytes(&self.unit_actions[..])?,
        })
    }
}

pub struct AgzActionModel {
    device: Device,
    city_actions: DNN,
    unit_actions: DNN,
}

impl AgzActionModel {
    pub fn new(device: Device, learning_rate: f64) -> Result<Self, String> {
        let possible_city_actions = NextCityAction::possible();
        let possible_unit_actions = NextUnitAction::possible();

        Ok(Self {
            device,
            city_actions: DNN::new(device, learning_rate, possible_city_actions as i64)?,
            unit_actions: DNN::new(device, learning_rate, possible_unit_actions as i64)?,
        })
    }

    pub fn train(&mut self, data: &Vec<AgzDatum>, sample_prob: f64) {
        let mut rand = thread_rng();
        for datum in data {
            if rand.gen::<f64>() > sample_prob {
                continue;
            }

            let target = datum.outcome.to_training_target();

            if let Ok(city_action) = NextCityAction::try_from(datum.action) {
                let city_action_idx: usize = city_action.into();

                self.city_actions
                    .train(&datum.features, &city_action_idx, target);
            } else {
                let unit_action = NextUnitAction::try_from(datum.action).unwrap();

                let unit_action_idx: usize = unit_action.into();

                self.unit_actions
                    .train(&datum.features, &unit_action_idx, target);
            }
        }
    }

    pub fn error(&self, data: &Vec<AgzDatum>) -> f64 {
        let mut sse = 0.0f64;
        for datum in data {
            let features = &datum.features;

            let predicted_outcome = if let Ok(city_action) =
                <AiPlayerAction as TryInto<NextCityAction>>::try_into(datum.action)
            {
                let city_action_idx: usize = city_action.into();

                self.city_actions
                    .evaluate_tensor(features, &city_action_idx)
            } else {
                let unit_action =
                    <AiPlayerAction as TryInto<NextUnitAction>>::try_into(datum.action).unwrap();

                let unit_action_idx: usize = unit_action.into();

                self.unit_actions
                    .evaluate_tensor(features, &unit_action_idx)
            };

            let actual_outcome = datum.outcome.to_training_target();

            sse += (predicted_outcome - actual_outcome).powf(2.0);
        }

        sse
    }

    async fn features(turn: &PlayerTurn<'_>, focus: TrainingFocus) -> Vec<f32> {
        turn.player_features(focus)
            .await
            .iter()
            .map(|x| *x as f32)
            .collect()
    }

    fn encode(self) -> AgzActionModelEncoding {
        AgzActionModelEncoding {
            city_actions: self.city_actions.store_as_bytes().unwrap(),
            unit_actions: self.unit_actions.store_as_bytes().unwrap(),
        }
    }
}

#[async_trait]
impl ActionwiseTurnTaker2 for AgzActionModel {
    async fn next_city_action(&mut self, turn: &PlayerTurn) -> Option<NextCityAction> {
        let legal_action_indices: HashSet<usize> = NextCityAction::legal(turn)
            .await
            .iter()
            .copied()
            .map(|a| a.into())
            .collect();

        if legal_action_indices.is_empty() {
            return None;
        }

        let feats = Self::features(turn, TrainingFocus::City).await;

        let feats = Tensor::try_from(feats).unwrap().to_device(self.device);

        let city_action_idx = self
            .city_actions
            .evaluate_tensors(&feats)
            .iter()
            .enumerate()
            .filter(|(i, _p_victory_ish)| legal_action_indices.contains(i))
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
            .0;

        Some(NextCityAction::try_from(city_action_idx).unwrap())
    }

    async fn next_unit_action(&mut self, turn: &PlayerTurn) -> Option<NextUnitAction> {
        let legal_action_indices: HashSet<usize> = NextUnitAction::legal(turn)
            .await
            .iter()
            .copied()
            .map(|a| a.into())
            .collect();

        if legal_action_indices.is_empty() {
            return None;
        }

        let feats = Self::features(turn, TrainingFocus::Unit).await;

        let feats = Tensor::try_from(feats).unwrap().to_device(self.device);

        let unit_action_idx = self
            .unit_actions
            .evaluate_tensors(&feats)
            .iter()
            .enumerate()
            .filter(|(i, _p_victory_ish)| legal_action_indices.contains(&i))
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
            .0;

        Some(NextUnitAction::try_from(unit_action_idx).unwrap())
    }
}

impl Storable for AgzActionModel {
    fn store(self, path: &std::path::Path) -> Result<(), String> {
        let w = OpenOptions::new()
            .create_new(true)
            .write(true)
            .append(false)
            .open(path)
            .map_err(|e| format!("Error opening {}: {}", path.display(), e))?;

        let enc = self.encode();

        bincode::serialize_into(w, &enc)
            .map_err(|e| format!("Error serializing encoded agz action model: {}", e))
    }
}

impl Loadable for AgzActionModel {
    fn load<P: AsRef<std::path::Path>>(path: P) -> Result<Self, String> {
        let r = OpenOptions::new()
            .read(true)
            .open(path.as_ref())
            .map_err(|e| format!("Error opening {}: {}", path.as_ref().display(), e))?;

        let enc: AgzActionModelEncoding = bincode::deserialize_from(r)
            .map_err(|e| format!("Error deserializing encoded agz action model: {}", e))?;

        let device = Device::cuda_if_available();
        enc.decode(device)
    }
}
