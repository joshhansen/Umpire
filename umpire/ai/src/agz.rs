//! AlphaGo Zero style action model
//!
//! Based on self-play game outcomes, learn P(victory|action; environment)
//!
//! Divided into two sub-models, one for city actions, one for unit actions
use std::{collections::HashSet, fs::OpenOptions};

use async_trait::async_trait;

use burn::prelude::*;

use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use common::{
    game::{
        action::{AiPlayerAction, NextCityAction, NextUnitAction},
        ai::{fX, TrainingFocus, TrainingOutcome, FEATS_LEN_USIZE},
        player::PlayerTurn,
        turn_async::ActionwiseTurnTaker2,
    },
    util::weighted_sample_idx,
};

use crate::{
    dnn::{DNNConfig, DNN},
    Loadable, LoadableFromBytes, Storable, StorableAsBytes,
};

pub struct AgzDatum<B: Backend> {
    pub features: Tensor<B, FEATS_LEN_USIZE>,
    pub action: AiPlayerAction,
    pub outcome: TrainingOutcome,
}

const POSSIBLE_CITY_ACTIONS: usize = NextCityAction::possible();
const POSSIBLE_UNIT_ACTIONS: usize = NextUnitAction::possible();
const TOTAL_ACTIONS: usize = POSSIBLE_CITY_ACTIONS + POSSIBLE_UNIT_ACTIONS;

#[derive(Serialize, Deserialize)]
pub struct AgzActionModelEncoding {
    actions: Vec<u8>,
}
impl AgzActionModelEncoding {
    pub fn decode<B: Backend>(self, device: B::Device) -> Result<AgzActionModel<B>, String> {
        Ok(AgzActionModel {
            device,

            actions: DNN::load_from_bytes(&self.actions[..])?,
        })
    }
}

pub struct AgzActionModel<B: Backend> {
    device: B::Device,

    /// This models all actions at once, city actions first
    actions: DNN<B>,
}

impl<B: Backend> AgzActionModel<B> {
    pub fn new(device: B::Device, learning_rate: f64) -> Result<Self, String> {
        let cfg = DNNConfig {
            learning_rate,
            possible_actions: TOTAL_ACTIONS,
        };
        let actions = cfg.init(&device);
        Ok(Self { device, actions })
    }

    pub fn train(&mut self, data: &Vec<AgzDatum<B>>, sample_prob: f64) {
        let mut rand = thread_rng();
        for datum in data {
            if rand.gen::<f64>() > sample_prob {
                continue;
            }

            let target = datum.outcome.to_training_target();

            if let Ok(city_action) = NextCityAction::try_from(datum.action) {
                // City actions go first, so no offset is added
                let city_action_idx: usize = city_action.into();

                debug_assert!(city_action_idx < POSSIBLE_CITY_ACTIONS);
                debug_assert!(city_action_idx < TOTAL_ACTIONS);

                self.actions
                    .train(&datum.features, &city_action_idx, target);
            } else {
                let unit_action = NextUnitAction::try_from(datum.action).unwrap();

                // We use the city action count as an offset since city actions go first
                let raw_unit_action_idx: usize = unit_action.into();
                let unit_action_idx: usize = POSSIBLE_CITY_ACTIONS + raw_unit_action_idx;

                debug_assert!(POSSIBLE_CITY_ACTIONS <= unit_action_idx);
                debug_assert!(unit_action_idx < TOTAL_ACTIONS);

                self.actions
                    .train(&datum.features, &unit_action_idx, target);
            }
        }
    }

    pub fn error(&self, data: &Vec<AgzDatum<B>>) -> f64 {
        let mut sse = 0.0f64;
        for datum in data {
            let features = &datum.features;

            let predicted_outcome = if let Ok(city_action) =
                <AiPlayerAction as TryInto<NextCityAction>>::try_into(datum.action)
            {
                // City actions go first, so no offset is added
                let city_action_idx: usize = city_action.into();

                self.actions.evaluate_tensor(features, &city_action_idx)
            } else {
                let unit_action =
                    <AiPlayerAction as TryInto<NextUnitAction>>::try_into(datum.action).unwrap();

                // We use the city action count as an offset since city actions go first
                let raw_unit_action_idx: usize = unit_action.into();
                let unit_action_idx: usize = POSSIBLE_CITY_ACTIONS + raw_unit_action_idx;

                debug_assert!(unit_action_idx < TOTAL_ACTIONS);

                self.actions.evaluate_tensor(features, &unit_action_idx)
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
            actions: self.actions.store_as_bytes().unwrap(),
        }
    }
}

#[async_trait]
impl<B: Backend> ActionwiseTurnTaker2 for AgzActionModel<B> {
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

        let feats = Tensor::try_from(feats).unwrap().to_device(&self.device);

        let probs = self.actions.evaluate_tensors(&feats);

        // No offset is subtracted because city actions go first
        let city_action_probs: Vec<(usize, fX)> = probs
            .into_iter()
            .enumerate() // enumerating yields city action indices because city actions go first
            .filter(|(i, _p_victory_ish)| legal_action_indices.contains(i))
            .collect();

        let mut rng = thread_rng();

        let city_action_idx = weighted_sample_idx(&mut rng, &city_action_probs);

        debug_assert!(
            city_action_idx < POSSIBLE_CITY_ACTIONS,
            "city_action_idx {} not less than POSSIBLE_CITY_ACTIONS {}",
            city_action_idx,
            POSSIBLE_CITY_ACTIONS
        );

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

        let feats = Tensor::try_from(feats).unwrap().to_device(&self.device);

        let unit_action_probs: Vec<(usize, fX)> = self
            .actions
            .evaluate_tensors(&feats)
            .into_iter()
            .skip(POSSIBLE_CITY_ACTIONS) // ignore the city prefix
            .enumerate() // enumerate now so we get unit action indices
            .filter(|(i, _p_victory_ish)| legal_action_indices.contains(&i))
            .collect();

        let mut rng = thread_rng();

        let unit_action_idx = weighted_sample_idx(&mut rng, &unit_action_probs);

        debug_assert!(
            unit_action_idx < POSSIBLE_UNIT_ACTIONS,
            "unit_action_idx {} not less than POSSIBLE_UNIT_ACTIONS {}",
            unit_action_idx,
            POSSIBLE_UNIT_ACTIONS
        );

        Some(NextUnitAction::try_from(unit_action_idx).unwrap())
    }
}

impl<B: Backend> Storable for AgzActionModel<B> {
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

impl<B: Backend> Loadable for AgzActionModel<B> {
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
