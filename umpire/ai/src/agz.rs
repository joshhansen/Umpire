//! AlphaGo Zero style action model
//!
//! Based on self-play game outcomes, learn P(victory|action; environment)
//!
//! Divided into two sub-models, one for city actions, one for unit actions
use std::{collections::HashSet, fs::OpenOptions};
use std::{fmt, fs::File, path::Path};

use async_trait::async_trait;

use burn::nn::loss::{MseLoss, Reduction};
use burn::record::{BinFileRecorder, FullPrecisionSettings};
use burn::tensor::backend::AutodiffBackend;
use burn::{
    module::Module,
    nn::{conv::Conv2dConfig, DropoutConfig, LinearConfig, Relu},
    optim::Optimizer,
    prelude::*,
    tensor::activation::softplus,
};
use burn_train::{RegressionOutput, TrainOutput, TrainStep, ValidStep};

use num_traits::ToPrimitive;

use rand::{thread_rng, Rng};

use serde::{
    de::{self, Visitor},
    Deserialize, Serialize,
};

use common::game::{
    action::{AiPlayerAction, NextCityAction, NextUnitAction},
    ai::{
        fX, TrainingFocus, TrainingOutcome, BASE_CONV_FEATS, BASE_CONV_FEATS_USIZE, DEEP_HEIGHT,
        DEEP_HEIGHT_USIZE, DEEP_LEN, DEEP_OUT_LEN, DEEP_OUT_LEN_USIZE, DEEP_WIDTH,
        DEEP_WIDTH_USIZE, FEATS_LEN, FEATS_LEN_USIZE, POSSIBLE_ACTIONS, POSSIBLE_ACTIONS_USIZE,
        WIDE_LEN, WIDE_LEN_USIZE,
    },
    player::PlayerTurn,
    turn_async::ActionwiseTurnTaker2,
    Game,
};
use common::util::weighted_sample_idx;

use crate::{data::AgzBatch, Loadable, LoadableFromBytes, Storable, StorableAsBytes};

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
    pub learning_rate: f64,
    pub possible_actions: usize,
}

impl AgzActionModelConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> AgzActionModel<B> {
        let convs = vec![
            Conv2dConfig::new([BASE_CONV_FEATS_USIZE, BASE_CONV_FEATS_USIZE * 2], [3, 3])
                .init(device), // -> 9x9
            Conv2dConfig::new(
                [BASE_CONV_FEATS_USIZE * 2, BASE_CONV_FEATS_USIZE * 2],
                [3, 3],
            )
            .init(device), // -> 7x7
            Conv2dConfig::new(
                [BASE_CONV_FEATS_USIZE * 2, BASE_CONV_FEATS_USIZE * 2],
                [3, 3],
            )
            .init(device), // -> 5x5
            Conv2dConfig::new([BASE_CONV_FEATS_USIZE * 2, BASE_CONV_FEATS_USIZE], [3, 3])
                .init(device), // -> 3x3
        ];

        let relu = Relu::new();

        let dropouts = vec![
            DropoutConfig::new(0.4).init(),
            DropoutConfig::new(0.4).init(),
            DropoutConfig::new(0.4).init(),
            DropoutConfig::new(0.4).init(),
        ];

        let dense0 = LinearConfig::new(WIDE_LEN_USIZE + DEEP_OUT_LEN_USIZE, 64).init(device);
        let dense1 = LinearConfig::new(64, 32).init(device);
        let dense2 = LinearConfig::new(32, self.possible_actions).init(device);

        // let optimizer = nn::Adam::default()
        //     .build(&vars, self.learning_rate)
        //     .map_err(|err| err.to_string())?;

        AgzActionModel {
            convs,
            dropouts,
            dense0,
            dense1,
            dense2,
            relu,
        }
        // Model {
        //     conv1: Conv2dConfig::new([1, 8], [3, 3]).init(device),
        //     conv2: Conv2dConfig::new([8, 16], [3, 3]).init(device),
        //     pool: AdaptiveAvgPool2dConfig::new([8, 8]).init(),
        //     activation: Relu::new(),
        //     linear1: LinearConfig::new(16 * 8 * 8, self.hidden_size).init(device),
        //     linear2: LinearConfig::new(self.hidden_size, self.num_classes).init(device),
        //     dropout: DropoutConfig::new(self.dropout).init(),
        // }
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
    relu: nn::Relu,
    convs: Vec<nn::conv::Conv2d<B>>,
    dropouts: Vec<nn::Dropout>,
    dense0: nn::Linear<B>,
    dense1: nn::Linear<B>,
    dense2: nn::Linear<B>,
}

impl<B: AutodiffBackend> AgzActionModel<B> {
    fn forward(&self, xs: &Tensor<B, 2>) -> Tensor<B, 1> {
        // Wide featuers that will pass through to the dense layers directly
        let wide = xs.slice([0..WIDE_LEN_USIZE]);

        // Input features to the 2d convolution
        let mut deep = xs.slice([WIDE_LEN_USIZE..FEATS_LEN_USIZE]).reshape([
            1,
            BASE_CONV_FEATS_USIZE,
            DEEP_WIDTH_USIZE,
            DEEP_HEIGHT_USIZE,
        ]);

        // let split: Vec<Tensor> = xs.split_with_sizes(&[WIDE_LEN, DEEP_LEN], 0);

        // // Wide featuers that will pass through to the dense layers directly
        // let wide = &split[0];

        // // Input features to the 2d convolution
        // let mut deep = split[1].view([1, BASE_CONV_FEATS, DEEP_WIDTH, DEEP_HEIGHT]);

        for (i, conv) in self.convs.iter().enumerate() {
            deep = conv.forward(deep);
            deep = self.relu.forward(deep);
            deep = self.dropouts[i].forward(deep);
        }

        // Reshape back to vector
        let deep_flat: Tensor<B, 1> = deep.reshape([-1]);

        let wide_and_deep = Tensor::cat(vec![wide, deep_flat], 0);

        // println!("Wide and deep shape: {:?}", wide_and_deep.size());

        // xs.view([-1, 1, 28, 28])
        //     .apply(&self.conv1)
        //     .max_pool2d_default(2)
        //     .apply(&self.conv2)
        //     .max_pool2d_default(2)
        //     .view([-1, 1024])
        //     .apply(&self.fc1)
        //     .relu()
        //     .dropout_(0.5, train)
        //     .apply(&self.fc2)

        let out0 = self.relu.forward(self.dense0.forward(wide_and_deep));
        let out1 = self.relu.forward(self.dense1.forward(out0));
        let out2 = softplus(self.dense2.forward(out1), 1f64);

        // wide_and_deep
        //     .apply(&self.dense0)
        //     .relu()
        //     // .dropout_(0.2, train)
        //     .apply(&self.dense1)
        //     .relu()
        //     // .dropout_(0.2, train)
        //     .apply(&self.dense2)
        //     .softplus()

        out2
    }

    pub fn evaluate_tensors(&self, features: &Tensor<B, 1>) -> Vec<fX> {
        let result_tensor = self.forward(features);

        // debug_assert!(result_tensor.device().is_cuda());

        // result_tensor.try_into().unwrap()
        result_tensor
            .into_data()
            .value
            .into_iter()
            .map(|x| x.to_f64().unwrap())
            .collect()
    }

    pub fn evaluate_tensor(&self, features: &Tensor<B, 1>, action: &usize) -> fX {
        let result_tensor = self.forward(features);

        // debug_assert!(result_tensor.device().is_cuda());

        let action_tensor = result_tensor.slice([*action..(*action + 1)]);

        action_tensor.into_scalar().to_f64().unwrap()
    }

    pub fn train_action<O: Optimizer<Self, B>>(
        &mut self,
        features: &Tensor<B, 2>,
        action: &usize,
        value: f64,
        optimizer: &O,
    ) {
        let estimates = self.forward(features);
        let action_estimate = estimates.slice([*action..(*action + 1)]);

        let device = Default::default();

        let value_tensor = Tensor::from_floats([value as f32], &device);

        // let value_tensor = Tensor::from(value as f32).to_device(self.vars.device());

        let f_loss: MseLoss<B> = MseLoss::new();

        let loss = f_loss.forward(action_estimate, value_tensor, Reduction::Sum);

        // let loss: Tensor = actual_estimate.mse_loss(&value_tensor, Reduction::Sum);

        // debug_assert!(loss.device().is_cuda());

        // self.optimizer.backward_step(&loss);

        optimizer.step(self.learning_rate, self)
    }

    pub fn forward_classification(
        &self,
        xs: Tensor<B, 2>,
        targets: Tensor<B, 1>,
    ) -> RegressionOutput<B> {
        let output = self.forward(&xs);
        let loss = MseLoss::new().forward(output.clone(), targets.clone(), Reduction::Mean);

        RegressionOutput::new(loss, output, targets)
    }

    pub fn forward_regression(
        &self,
        features: Tensor<B, 2>,
        action: usize,
        value: f64,
    ) -> RegressionOutput<B> {
        let estimates = self.forward(&features);
        let action_estimate = estimates.slice([action..(action + 1)]);

        let device = Default::default();

        let value_tensor = Tensor::from_floats([value as f32], &device);

        // let value_tensor = Tensor::from(value as f32).to_device(self.vars.device());

        let f_loss: MseLoss<B> = MseLoss::new();

        let loss = f_loss.forward(action_estimate, value_tensor, Reduction::Sum);

        RegressionOutput::new(loss, action_estimate, value_tensor)
    }

    // pub fn new(device: B::Device, learning_rate: f64) -> Result<Self, String> {
    //     let cfg = AgzActionModelConfig {
    //         learning_rate,
    //         possible_actions: TOTAL_ACTIONS,
    //     };
    //     let actions = cfg.init(&device);
    //     Ok(Self { device, actions })
    // }

    pub fn train_macro(&mut self, data: &Vec<AgzDatum<B>>, sample_prob: f64) {
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

                self.train(&datum.features, &city_action_idx, target);
            } else {
                let unit_action = NextUnitAction::try_from(datum.action).unwrap();

                // We use the city action count as an offset since city actions go first
                let raw_unit_action_idx: usize = unit_action.into();
                let unit_action_idx: usize = POSSIBLE_CITY_ACTIONS + raw_unit_action_idx;

                debug_assert!(POSSIBLE_CITY_ACTIONS <= unit_action_idx);
                debug_assert!(unit_action_idx < TOTAL_ACTIONS);

                self.train(&datum.features, &unit_action_idx, target);
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

                self.evaluate_tensor(features, &city_action_idx)
            } else {
                let unit_action =
                    <AiPlayerAction as TryInto<NextUnitAction>>::try_into(datum.action).unwrap();

                // We use the city action count as an offset since city actions go first
                let raw_unit_action_idx: usize = unit_action.into();
                let unit_action_idx: usize = POSSIBLE_CITY_ACTIONS + raw_unit_action_idx;

                debug_assert!(unit_action_idx < TOTAL_ACTIONS);

                self.evaluate_tensor(features, &unit_action_idx)
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

    // fn encode(self) -> AgzActionModelEncoding {
    //     AgzActionModelEncoding {
    //         actions: self.actions.store_as_bytes().unwrap(),
    //     }
    // }
}

impl<B: Backend> Loadable for AgzActionModel<B> {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!(
                "Can't load AgzActionModel from path '{:?}' because it doesn't exist",
                path
            ));
        }

        let recorder: BinFileRecorder<FullPrecisionSettings> = BinFileRecorder::new();

        let config = AgzActionModelConfig {
            learning_rate: 0.0,
            possible_actions: 0,
        };

        let device = Default::default();

        let model: AgzActionModel<B> = config.init(&device);

        model
            .load_file(path, &recorder, &device)
            .map_err(|e| e.to_string())
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

        let device = Default::default();

        let feats = Tensor::from_floats(feats.as_slice(), &device);

        let probs = self.evaluate_tensors(&feats);

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

        let device = Default::default();
        let feats = Tensor::from_floats(feats.as_slice(), &device);

        let unit_action_probs: Vec<(usize, fX)> = self
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

pub struct AgzDatum<B: Backend> {
    pub features: Tensor<B, 1>,
    pub action: AiPlayerAction,
    pub outcome: TrainingOutcome,
}

const POSSIBLE_CITY_ACTIONS: usize = NextCityAction::possible();
const POSSIBLE_UNIT_ACTIONS: usize = NextUnitAction::possible();
const TOTAL_ACTIONS: usize = POSSIBLE_CITY_ACTIONS + POSSIBLE_UNIT_ACTIONS;

// #[derive(Serialize, Deserialize)]
// pub struct AgzActionModelEncoding {
//     actions: Vec<u8>,
// }
// impl AgzActionModelEncoding {
//     pub fn decode<B: Backend>(self, device: B::Device) -> Result<AgzActionModel<B>, String> {
//         Ok(AgzActionModel {
//             device,

//             actions: DNN::load_from_bytes(&self.actions[..])?,
//         })
//     }
// }

// pub struct AgzActionModel<B: Backend> {
//     device: B::Device,

//     /// This models all actions at once, city actions first
//     actions: DNN<B>,
// }

// impl<B: Backend> AgzActionModel<B> {}

// #[async_trait]
// impl<B: Backend> ActionwiseTurnTaker2 for AgzActionModel<B> {
//     async fn next_city_action(&mut self, turn: &PlayerTurn) -> Option<NextCityAction> {
//         let legal_action_indices: HashSet<usize> = NextCityAction::legal(turn)
//             .await
//             .iter()
//             .copied()
//             .map(|a| a.into())
//             .collect();

//         if legal_action_indices.is_empty() {
//             return None;
//         }

//         let feats = Self::features(turn, TrainingFocus::City).await;

//         let feats = Tensor::from_floats(feats.as_slice(), &self.device);

//         let probs = self.actions.evaluate_tensors(&feats);

//         // No offset is subtracted because city actions go first
//         let city_action_probs: Vec<(usize, fX)> = probs
//             .into_iter()
//             .enumerate() // enumerating yields city action indices because city actions go first
//             .filter(|(i, _p_victory_ish)| legal_action_indices.contains(i))
//             .collect();

//         let mut rng = thread_rng();

//         let city_action_idx = weighted_sample_idx(&mut rng, &city_action_probs);

//         debug_assert!(
//             city_action_idx < POSSIBLE_CITY_ACTIONS,
//             "city_action_idx {} not less than POSSIBLE_CITY_ACTIONS {}",
//             city_action_idx,
//             POSSIBLE_CITY_ACTIONS
//         );

//         Some(NextCityAction::try_from(city_action_idx).unwrap())
//     }

//     async fn next_unit_action(&mut self, turn: &PlayerTurn) -> Option<NextUnitAction> {
//         let legal_action_indices: HashSet<usize> = NextUnitAction::legal(turn)
//             .await
//             .iter()
//             .copied()
//             .map(|a| a.into())
//             .collect();

//         if legal_action_indices.is_empty() {
//             return None;
//         }

//         let feats = Self::features(turn, TrainingFocus::Unit).await;

//         let feats = Tensor::from_floats(feats.as_slice(), &self.device);

//         let unit_action_probs: Vec<(usize, fX)> = self
//             .actions
//             .evaluate_tensors(&feats)
//             .into_iter()
//             .skip(POSSIBLE_CITY_ACTIONS) // ignore the city prefix
//             .enumerate() // enumerate now so we get unit action indices
//             .filter(|(i, _p_victory_ish)| legal_action_indices.contains(&i))
//             .collect();

//         let mut rng = thread_rng();

//         let unit_action_idx = weighted_sample_idx(&mut rng, &unit_action_probs);

//         debug_assert!(
//             unit_action_idx < POSSIBLE_UNIT_ACTIONS,
//             "unit_action_idx {} not less than POSSIBLE_UNIT_ACTIONS {}",
//             unit_action_idx,
//             POSSIBLE_UNIT_ACTIONS
//         );

//         Some(NextUnitAction::try_from(unit_action_idx).unwrap())
//     }
// }

// impl<B: Backend> Storable for AgzActionModel<B> {
//     fn store(self, path: &std::path::Path) -> Result<(), String> {
//         let w = OpenOptions::new()
//             .create_new(true)
//             .write(true)
//             .append(false)
//             .open(path)
//             .map_err(|e| format!("Error opening {}: {}", path.display(), e))?;

//         let enc = self.encode();

//         bincode::serialize_into(w, &enc)
//             .map_err(|e| format!("Error serializing encoded agz action model: {}", e))
//     }
// }

// impl<B: Backend> Loadable for AgzActionModel<B> {
//     fn load<P: AsRef<std::path::Path>>(path: P) -> Result<Self, String> {
//         let r = OpenOptions::new()
//             .read(true)
//             .open(path.as_ref())
//             .map_err(|e| format!("Error opening {}: {}", path.as_ref().display(), e))?;

//         let enc: AgzActionModelEncoding = bincode::deserialize_from(r)
//             .map_err(|e| format!("Error deserializing encoded agz action model: {}", e))?;

//         let device = Default::default();
//         // let device = Device::cuda_if_available();
//         enc.decode(device)
//     }
// }

impl<B: AutodiffBackend> TrainStep<AgzBatch<B>, RegressionOutput<B>> for AgzActionModel<B> {
    fn step(&self, batch: AgzBatch<B>) -> TrainOutput<RegressionOutput<B>> {
        let item = self.forward_classification(batch.data, batch.targets);

        TrainOutput::new(self, item.loss.backward(), item)
    }
}

impl<B: Backend> ValidStep<AgzBatch<B>, RegressionOutput<B>> for AgzActionModel<B> {
    fn step(&self, batch: AgzBatch<B>) -> RegressionOutput<B> {
        self.forward_classification(batch.data, batch.targets)
    }
}
