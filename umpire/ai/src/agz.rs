//! AlphaGo Zero style action model
//!
//! Based on self-play game outcomes, learn P(victory|action; environment)
//!
//! Divided into two sub-models, one for city actions, one for unit actions
use std::collections::BTreeSet;
use std::{fmt, path::Path};

use async_trait::async_trait;

use burn::nn::loss::{MseLoss, Reduction};
use burn::record::{
    BinBytesRecorder, BinFileRecorder, FullPrecisionSettings, NamedMpkFileRecorder, Recorder,
};
use burn::tensor::activation::{relu, sigmoid};
use burn::tensor::backend::AutodiffBackend;
use burn::{
    module::Module,
    nn::{conv::Conv2dConfig, LinearConfig},
    prelude::*,
};
use burn_train::{RegressionOutput, TrainOutput, TrainStep, ValidStep};

use common::game::ai::{POSSIBLE_ACTIONS, POSSIBLE_CITY_ACTIONS, POSSIBLE_UNIT_ACTIONS};
use num_traits::ToPrimitive;

use serde::de::{self, Visitor};

use common::game::{
    action::{NextCityAction, NextUnitAction},
    ai::{
        fX, TrainingFocus, BASE_CONV_FEATS, DEEP_HEIGHT, DEEP_OUT_LEN, DEEP_WIDTH, FEATS_LEN,
        WIDE_LEN,
    },
    player::PlayerTurn,
    turn_async::ActionwiseTurnTaker2,
};
use common::util::max_sample_idx;

use crate::LoadableFromBytes;
use crate::{data::AgzBatch, Loadable, Storable};

struct BytesVisitor;
impl<'de> Visitor<'de> for BytesVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "an array of bytes")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Vec::from(v))
    }
}

#[derive(Config, Debug)]
pub struct AgzActionModelConfig {
    #[config(default = 0.1)]
    pub dropout_prob: f64,

    pub possible_actions: usize,
}

impl AgzActionModelConfig {
    pub fn init<B: Backend>(&self, device: B::Device) -> AgzActionModel<B> {
        let convs = vec![
            Conv2dConfig::new([BASE_CONV_FEATS, BASE_CONV_FEATS], [3, 3]).init(&device), // -> 15x15
            Conv2dConfig::new([BASE_CONV_FEATS, BASE_CONV_FEATS], [3, 3]).init(&device), // -> 13x13
            Conv2dConfig::new([BASE_CONV_FEATS, BASE_CONV_FEATS], [3, 3]).init(&device), // -> 11x11
            Conv2dConfig::new([BASE_CONV_FEATS, BASE_CONV_FEATS], [3, 3]).init(&device), // -> 9x9
            Conv2dConfig::new([BASE_CONV_FEATS, BASE_CONV_FEATS], [3, 3]).init(&device), // -> 7x7
            Conv2dConfig::new([BASE_CONV_FEATS, BASE_CONV_FEATS], [3, 3]).init(&device), // -> 5x5
            Conv2dConfig::new([BASE_CONV_FEATS, BASE_CONV_FEATS], [3, 3]).init(&device), // -> 3x3
        ];
        let dense = vec![
            LinearConfig::new(WIDE_LEN + DEEP_OUT_LEN, 128).init(&device),
            LinearConfig::new(128, 64).init(&device),
            LinearConfig::new(64, 32).init(&device),
            LinearConfig::new(32, self.possible_actions).init(&device),
        ];

        AgzActionModel { convs, dense }
    }
}

/// Approach: give all the info as raw as possible.
///
/// Use the neural network to extract features rather than proclaiming them a priori
///
/// Reduces 11x11 with 16 channels down to 3x3 with 16 channels
///
/// See `Obs::features` and `Game::player_features` for more information
#[derive(Debug, Module)]
pub struct AgzActionModel<B: Backend> {
    convs: Vec<nn::conv::Conv2d<B>>,
    dense: Vec<nn::Linear<B>>,
}
impl<B: Backend> AgzActionModel<B> {
    async fn features(turn: &PlayerTurn<'_>, focus: TrainingFocus) -> Vec<fX> {
        turn.player_features(focus).await
    }

    /// features: [batch,feat]
    /// actions: [batch]
    ///
    /// -> [batch,action_idx] (victory prob)
    fn forward(&self, features: Tensor<B, 2>) -> Tensor<B, 2> {
        // Wide features that will pass through to the dense layers directly
        // [batch,wide_feat]
        let batches = features.dims()[0];
        let wide = features.clone().slice([0..batches, 0..WIDE_LEN]);

        // Input features to the 2d convolution
        // [batch,conv_feat,x,y]
        let mut deep = features.slice([0..batches, WIDE_LEN..FEATS_LEN]).reshape([
            batches as i32,
            BASE_CONV_FEATS as i32,
            DEEP_WIDTH as i32,
            DEEP_HEIGHT as i32,
        ]);

        for conv in &self.convs {
            deep = relu(conv.forward(deep));
        }

        // Reshape back to vector
        // [batch,deep_feat]
        let deep_flat: Tensor<B, 2> = deep.reshape([batches as i32, DEEP_OUT_LEN as i32]);

        // [batch,feat]
        let wide_and_deep = Tensor::cat(vec![wide, deep_flat], 1);
        let mut out = wide_and_deep;

        for dense in self.dense.iter().take(self.dense.len() - 1) {
            out = relu(dense.forward(out));
        }

        sigmoid(self.dense.last().unwrap().forward(out))
    }

    fn forward_by_action(
        &self,
        features: Tensor<B, 2>,
        actions: Tensor<B, 1, Int>,
    ) -> Tensor<B, 2> {
        let batches = features.dims()[0];
        let action_victory_probs = self.forward(features);

        let actions_by_batch = actions.reshape([batches, 1]);
        action_victory_probs.gather(1, actions_by_batch)
    }

    /// [batch,feat]
    fn evaluate_tensors(&self, features: Tensor<B, 2>) -> Vec<fX> {
        let result_tensor = self.forward(features);

        // debug_assert!(result_tensor.device().is_cuda());

        // result_tensor.try_into().unwrap()
        result_tensor
            .into_data()
            .value
            .into_iter()
            .map(|x| x.to_f32().unwrap())
            .collect()
    }

    /**
     xs: [batch,feat]
     targets: [batch,target] - we're forced into 2d by RegressionOutput, target will always be 0
    */
    fn forward_regression_bulk(
        &self,
        features: Tensor<B, 2>,
        actions: Tensor<B, 1, Int>,
        targets: Tensor<B, 1>,
    ) -> RegressionOutput<B> {
        let output = self.forward_by_action(features, actions);
        let targets_batched = targets.reshape([-1, 1]);
        let loss = MseLoss::new().forward(output.clone(), targets_batched.clone(), Reduction::Mean);

        RegressionOutput::new(loss, output, targets_batched)
    }
}

impl<B: Backend> Loadable<B> for AgzActionModel<B> {
    fn load<P: AsRef<Path>>(path: P, device: B::Device) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!(
                "Can't load AgzActionModel from path '{:?}' because it doesn't exist",
                path
            ));
        }

        let recorder: NamedMpkFileRecorder<FullPrecisionSettings> = NamedMpkFileRecorder::new();

        let config = AgzActionModelConfig::new(POSSIBLE_ACTIONS);

        let model: AgzActionModel<B> = config.init(device.clone());

        model
            .load_file(path, &recorder, &device)
            .map_err(|e| e.to_string())
    }
}

impl<B: Backend> LoadableFromBytes<B> for AgzActionModel<B> {
    fn load_from_bytes<S: std::io::Read>(mut bytes: S, device: B::Device) -> Result<Self, String> {
        let config = AgzActionModelConfig::new(POSSIBLE_ACTIONS);

        let model: AgzActionModel<B> = config.init(device.clone());

        let mut buf = Vec::new();
        bytes.read_to_end(&mut buf).unwrap();

        let record = BinBytesRecorder::<FullPrecisionSettings>::default()
            .load(buf, &device)
            .unwrap();

        Ok(model.load_record(record))
    }
}

impl<B: Backend> Storable for AgzActionModel<B> {
    fn store(self, path: &Path) -> Result<(), String> {
        let recorder: BinFileRecorder<FullPrecisionSettings> = BinFileRecorder::new();

        self.save_file(path, &recorder).map_err(|e| e.to_string())
    }
}

#[async_trait]
impl<B: Backend> ActionwiseTurnTaker2 for AgzActionModel<B> {
    async fn next_city_action(&mut self, turn: &PlayerTurn) -> Option<NextCityAction> {
        let legal_action_indices: BTreeSet<usize> = NextCityAction::legal(turn)
            .await
            .into_iter()
            .map(|a| a.into())
            .collect();

        if legal_action_indices.is_empty() {
            return None;
        }

        let feats = Self::features(turn, TrainingFocus::City).await;

        let device: B::Device = Default::default();

        // [batch,feat] (a batch of one)
        let feats: Tensor<B, 2> = Tensor::from_floats(feats.as_slice(), &device).reshape([1, -1]);

        let probs = self.evaluate_tensors(feats);

        // No offset is subtracted because city actions go first
        let city_action_probs: Vec<(usize, fX)> = probs
            .into_iter()
            .enumerate() // enumerating yields city action indices because city actions go first
            .filter(|(i, _p_victory_ish)| legal_action_indices.contains(i))
            .collect();

        let city_action_idx = max_sample_idx(&city_action_probs);

        debug_assert!(
            city_action_idx < POSSIBLE_CITY_ACTIONS,
            "city_action_idx {} not less than POSSIBLE_CITY_ACTIONS {}",
            city_action_idx,
            POSSIBLE_CITY_ACTIONS
        );

        Some(NextCityAction::from(city_action_idx))
    }

    async fn next_unit_action(&mut self, turn: &PlayerTurn) -> Option<NextUnitAction> {
        let legal_action_indices: BTreeSet<usize> = NextUnitAction::legal(turn)
            .await
            .iter()
            .copied()
            .map(|a| a.into())
            .collect();

        if legal_action_indices.is_empty() {
            return None;
        }

        let feats = Self::features(turn, TrainingFocus::Unit).await;

        let device: B::Device = Default::default();
        let feats = Tensor::from_floats(feats.as_slice(), &device).reshape([1, -1]);

        let unit_action_probs: Vec<(usize, fX)> = self
            .evaluate_tensors(feats)
            .into_iter()
            .skip(POSSIBLE_CITY_ACTIONS) // ignore the city prefix
            .enumerate() // enumerate now so we get unit action indices
            .filter(|(i, _p_victory_ish)| legal_action_indices.contains(i))
            .collect();

        let unit_action_idx = max_sample_idx(&unit_action_probs);

        debug_assert!(
            unit_action_idx < POSSIBLE_UNIT_ACTIONS,
            "unit_action_idx {} not less than POSSIBLE_UNIT_ACTIONS {}",
            unit_action_idx,
            POSSIBLE_UNIT_ACTIONS
        );

        Some(NextUnitAction::from(unit_action_idx))
    }
}

impl<B: AutodiffBackend> TrainStep<AgzBatch<B>, RegressionOutput<B>> for AgzActionModel<B> {
    fn step(&self, batch: AgzBatch<B>) -> TrainOutput<RegressionOutput<B>> {
        let item = self.forward_regression_bulk(batch.features, batch.actions, batch.targets);

        TrainOutput::new(self, item.loss.backward(), item)
    }
}

impl<B: Backend> ValidStep<AgzBatch<B>, RegressionOutput<B>> for AgzActionModel<B> {
    fn step(&self, batch: AgzBatch<B>) -> RegressionOutput<B> {
        self.forward_regression_bulk(batch.features, batch.actions, batch.targets)
    }
}
