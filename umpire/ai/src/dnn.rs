use std::{
    cmp::{Ordering, PartialEq, PartialOrd},
    fmt,
    ops::{Mul, Sub},
    path::Path,
};

use futures;

use rsrl::{
    fa::{EnumerableStateActionFunction, StateActionFunction},
    DerefVec,
};

use serde::de::{self, Visitor};

use tch::{
    nn::{self, ModuleT, Optimizer, OptimizerConfig},
    Device, Tensor,
};

use common::game::{
    action::AiPlayerAction,
    ai::{DEEP_HEIGHT, DEEP_LEN, DEEP_WIDTH, POSSIBLE_ACTIONS, WIDE_LEN},
    turn_async::ActionwiseTurnTaker,
    Game,
};

use super::{Loadable, Storable};

// const LEARNING_RATE: f64 = 1e-4;

const BASE_CONV_FEATS: i64 = 8;

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

#[derive(Debug)]
pub struct DNN {
    // path: nn::Path<'a>,
    vars: nn::VarStore,
    convs: Vec<nn::Conv2D>,
    dense0: nn::Linear,
    dense1: nn::Linear,
    dense2: nn::Linear,
    optimizer: Optimizer,
}

impl DNN {
    async fn tensor_for(&self, state: &Game) -> Tensor {
        //NOTE We could avoid this extra allocation if we could figure out how to use 64-bit weights in PyTorch
        //     or 32-bit weights in `rsrl`
        let features_f64 = state.deref_vec();
        let mut features: Vec<f32> = Vec::with_capacity(features_f64.len());
        for feat in features_f64 {
            features.push(feat as f32);
        }
        Tensor::try_from(features)
            .unwrap()
            .to_device(Device::cuda_if_available())
    }

    pub fn new(learning_rate: f32) -> Result<Self, String> {
        let device = Device::cuda_if_available();
        println!("Device: {:?}", device);
        let vars = nn::VarStore::new(device);

        let mut lr = vars.root().zeros_no_train("learning_rate", &[1]);
        lr.copy_(&Tensor::try_from(vec![learning_rate]).map_err(|err| {
            format!(
                "Cant' create DNN because the learning rate could not be encoded as a tensor: {}",
                err
            )
        })?);

        Self::with_varstore(vars)
    }

    fn with_varstore(vars: nn::VarStore) -> Result<Self, String> {
        let path = vars.root();

        let learning_rate = 10e-3_f64;

        // let learning_rate: f64 = path
        //     .get("learning_rate")
        //     .ok_or(format!("Learning rate not set in VarStore"))?
        //     .double_value(&[0]);

        let conv0 = nn::conv2d(&path, 1, BASE_CONV_FEATS, 3, Default::default()); // -> 9x9
        let conv1 = nn::conv2d(
            &path,
            BASE_CONV_FEATS,
            BASE_CONV_FEATS * 2,
            3,
            Default::default(),
        ); // -> 7x7
        let conv2 = nn::conv2d(
            &path,
            BASE_CONV_FEATS * 2,
            BASE_CONV_FEATS * 4,
            3,
            Default::default(),
        ); // -> 5x5
        let conv3 = nn::conv2d(
            &path,
            BASE_CONV_FEATS * 4,
            BASE_CONV_FEATS * 8,
            3,
            Default::default(),
        ); // -> 3x3

        let convs = vec![conv0, conv1, conv2, conv3];

        let dense0 = nn::linear(&path, 2329, 256, Default::default());
        let dense1 = nn::linear(&path, 256, 128, Default::default());
        let dense2 = nn::linear(&path, 128, POSSIBLE_ACTIONS as i64, Default::default());

        let optimizer = nn::Adam::default()
            .build(&vars, learning_rate)
            .map_err(|err| err.to_string())?;

        Ok(Self {
            vars,
            convs,
            dense0,
            dense1,
            dense2,
            optimizer,
        })
    }
}

impl StateActionFunction<Game, usize> for DNN {
    type Output = f64;

    fn evaluate(&self, state: &Game, action: &usize) -> Self::Output {
        let features = futures::executor::block_on(async { self.tensor_for(state).await });

        let result_tensor = <Self as nn::ModuleT>::forward_t(self, &features, true);

        result_tensor.double_value(&[*action as i64])
    }

    fn update_with_error(
        &mut self,
        state: &Game,
        action: &usize,
        value: Self::Output,
        _estimate: Self::Output,
        _error: Self::Output,
        _raw_error: Self::Output,
        _learning_rate: Self::Output,
    ) {
        let features = futures::executor::block_on(async { self.tensor_for(state).await });

        let exponent = Tensor::from(2.0f64);

        let actual_estimate: Tensor =
            self.forward_t(&features, true)
                .slice(0, *action as i64, *action as i64 + 1, 1);

        let loss: Tensor = (value - actual_estimate).pow(&exponent); // we're doing mean squared error

        self.optimizer.backward_step(&loss);
    }
}

struct TensorAndScalar(pub Tensor, pub f64);

impl Mul for TensorAndScalar {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0, self.1 * rhs.1)
    }
}

impl Sub for TensorAndScalar {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0, self.1 - rhs.1)
    }
}

impl PartialEq for TensorAndScalar {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl PartialOrd for TensorAndScalar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.1.partial_cmp(&other.1)
    }
}

impl EnumerableStateActionFunction<Game> for DNN {
    fn n_actions(&self) -> usize {
        AiPlayerAction::possible_actions().len()
    }

    fn evaluate_all(&self, state: &Game) -> Vec<f64> {
        (0..self.n_actions())
            .map(|action_idx| self.evaluate(state, &action_idx))
            .collect()
    }

    fn update_all_with_errors(
        &mut self,
        state: &Game,
        values: Vec<f64>,
        estimates: Vec<f64>,
        errors: Vec<f64>,
        raw_errors: Vec<f64>,
        learning_rate: f64,
    ) {
        for (i, value) in values.iter().enumerate() {
            self.update_with_error(
                state,
                &i,
                *value,
                estimates[i],
                errors[i],
                raw_errors[i],
                learning_rate,
            );
        }
    }
}

impl Loadable for DNN {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!(
                "Can't load DNN from path '{:?}' because it doesn't exist",
                path
            ));
        }

        let device = Device::cuda_if_available();

        let mut vars = nn::VarStore::new(device);

        vars.load(path).map_err(|err| err.to_string())?;

        Self::with_varstore(vars)
    }
}

impl nn::ModuleT for DNN {
    fn forward_t(&self, xs: &Tensor, _train: bool) -> Tensor {
        let split: Vec<Tensor> =
            xs.split_with_sizes(&[WIDE_LEN, DEEP_LEN, DEEP_LEN, DEEP_LEN, DEEP_LEN], 0);

        // Wide features
        let wide = &split[0];

        // Deep features
        let mut is_enemy_belligerent = split[1].view([1, 1, DEEP_WIDTH, DEEP_HEIGHT]);
        let mut is_observed = split[2].view([1, 1, DEEP_WIDTH, DEEP_HEIGHT]);
        let mut is_neutral = split[3].view([1, 1, DEEP_WIDTH, DEEP_HEIGHT]);
        let mut is_city = split[4].view([1, 1, DEEP_WIDTH, DEEP_HEIGHT]);

        for conv in &self.convs {
            is_enemy_belligerent = is_enemy_belligerent.apply(conv).relu();
            is_observed = is_observed.apply(conv).relu();
            is_neutral = is_neutral.apply(conv).relu();
            is_city = is_city.apply(conv).relu();
        }

        let enemy_feats = is_enemy_belligerent.view([-1]);
        let observed_feats = is_observed.view([-1]);
        let neutral_feats = is_neutral.view([-1]);
        let city_feats = is_city.view([-1]);

        // println!("Deep feats shapes: {:?} {:?} {:?}", enemy_feats.size(), observed_feats.size(), neutral_feats.size());

        let wide_and_deep = Tensor::cat(
            &[
                wide,
                &enemy_feats,
                &observed_feats,
                &neutral_feats,
                &city_feats,
            ],
            0,
        );

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

        wide_and_deep
            .apply(&self.dense0)
            .relu()
            // .dropout_(0.1, train)
            .apply(&self.dense1)
            .relu()
            // .dropout_(0.1, train)
            .apply(&self.dense2)
            .relu()
        // .dropout_(0.1, train)
    }
}

impl Storable for DNN {
    fn store(self, path: &Path) -> Result<(), String> {
        self.vars.save(path).map_err(|err| err.to_string())
        // let mut builder = SavedModelBuilder::new();
        // builder.add_tag(TAG);

        // let saver = builder.inject(&mut self.scope)
        //        .map_err(|status| {
        //            format!("Error injecting scope into saved model builder, status {}", status)
        //        })?;

        // let graph = self.scope.graph();

        // saver.save(&self.session, &(*graph), path)
        //      .map_err(|err| format!("Error saving DNN: {}", err))
    }
}

// impl ActionwiseTurnTaker for DNN {
//     async fn next_action(&mut self, turn: &PlayerTurn) -> Option<AiPlayerAction> {

//     }
// }
