use std::{
    cmp::{
        Ordering,
        PartialEq,
        PartialOrd,
    },
    convert::TryFrom,
    fmt,
    ops::{
        Mul,
        Sub,
    },
    path::Path
};


use rsrl::{
    fa::{
        EnumerableStateActionFunction,
        StateActionFunction,
    },
};

use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
    de::{self,Visitor},
};

use tch::{
    Device,
    nn::{
        self,
        ModuleT,
        Optimizer,
        OptimizerConfig,
    },
    Tensor,
};

use crate::{
    game::{
        Game,
        ai::UmpireAction,
        fX,
        unit::UnitType,
    },
};

use super::{Storable, Loadable, rl::POSSIBLE_ACTIONS};

const LEARNING_RATE: f64 = 1e-4;

const ADDED_WIDE_FEATURES: i64 = 4;
const UNIT_TYPE_WRIT_LARGE_LEN: i64 = UnitType::values().len() as i64 + 1;// what sort of unit is being considered, including
                                                                    // "city" as a unit type (thus the +1)

const WIDE_LEN: i64 = UNIT_TYPE_WRIT_LARGE_LEN + UnitType::values().len() as i64 + ADDED_WIDE_FEATURES;
const DEEP_WIDTH: i64 = 11;
const DEEP_HEIGHT: i64 = 11;
const DEEP_LEN: i64 = DEEP_WIDTH * DEEP_HEIGHT;
const DEEP_FEATS: i64 = 4;
pub(crate) const FEATS_LEN: i64 = WIDE_LEN + DEEP_FEATS * DEEP_LEN;

struct BytesVisitor;
impl<'de> Visitor<'de> for BytesVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "an array of bytes")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> where E: de::Error {
        Ok(Vec::from(v))
    }
}

#[derive(Debug)]
pub struct DNN {
    // path: nn::Path<'a>,
    vars: nn::VarStore,
    dense0: nn::Linear,
    dense1: nn::Linear,
    // dense2: nn::Linear,
    optimizer: Optimizer<nn::Adam>,
}

impl DNN {
    fn tensor_for(&self, state: &Game) -> Tensor {
        let x: Vec<f32> = state.features().iter().map(|x| *x as f32).collect();
        Tensor::try_from(x).unwrap().to_device(Device::cuda_if_available())
    }

    pub fn new() -> Result<Self,String> {
        let device = Device::cuda_if_available();
        println!("Device: {:?}", device);
        let vars = nn::VarStore::new(device);
        Self::with_varstore(vars)
    }

    fn with_varstore(vars: nn::VarStore) -> Result<Self,String> {
        let path = vars.root();
        let dense0 = nn::linear(&path, FEATS_LEN, 128, Default::default());
        // let dense1 = nn::linear(&path, 512, 256, Default::default());
        let dense1 = nn::linear(&path, 128, POSSIBLE_ACTIONS as i64, Default::default());
        // let dense2 = nn::linear(&path, 256, POSSIBLE_ACTIONS as i64, Default::default());

        let optimizer = nn::Adam::default().build(&vars, LEARNING_RATE)
            .map_err(|err| err.to_string())
        ?;

        Ok(Self {
            vars,
            dense0,
            dense1,
            // dense2,
            optimizer,
        })
    }
}


impl StateActionFunction<Game, usize> for DNN {
    type Output = f64;

    fn evaluate(&self, state: &Game, action: &usize) -> Self::Output {
        
        let features = self.tensor_for(state);

        let result_tensor = <Self as nn::ModuleT>::forward_t(self, &features, true);

        result_tensor.double_value(&[*action as i64])

    }

    fn update_with_error(&mut self, state: &Game, action: &usize, value: Self::Output, estimate: Self::Output,
            error: Self::Output, raw_error: Self::Output, learning_rate: Self::Output) {
    
        let features = self.tensor_for(state);

        let actual_estimate: Tensor = self.forward_t(&features, true).slice(0, *action as i64, *action as i64+1, 1);

        let loss: Tensor = (value - actual_estimate).pow(2.0f64);// we're doing mean squared error

        // println!("Requires grad: {}", loss.requires_grad());
        // println!("Dims: {}", loss.dim());
        // println!("Shape: {:?}", loss.size());

        self.optimizer.backward_step(&loss);

    }
}

struct TensorAndScalar(pub Tensor, pub f64);

impl Mul for TensorAndScalar {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(
            self.0 * rhs.0,
            self.1 * rhs.1
        )
    }
}

impl Sub for TensorAndScalar {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(
            self.0 - rhs.0,
            self.1 - rhs.1
        )
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
        UmpireAction::possible_actions().len()
    }

    fn evaluate_all(&self, state: &Game) -> Vec<f64> {
        (0..self.n_actions()).map(|action_idx| {
            self.evaluate(state, &action_idx)
        }).collect()
    }

    fn update_all_with_errors(&mut self, state: &Game, values: Vec<f64>, estimates: Vec<f64>, errors: Vec<f64>,
        raw_errors: Vec<f64>, learning_rate: f64) {

        for (i, value) in values.iter().enumerate() {
            self.update_with_error(state, &i, *value, estimates[i], errors[i], raw_errors[i], learning_rate);
        }
    }
}

impl Loadable for DNN {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self,String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!("Can't load DNN from path '{:?}' because it doesn't exist", path));
        }

        let device = Device::cuda_if_available();

        let mut vars = nn::VarStore::new(device);

        vars.load(path)
            .map_err(|err| err.to_string())?;

        // let path = vars.root();

        Self::with_varstore(vars)
    }
}

impl nn::ModuleT for DNN {
    fn forward_t(&self, xs: &Tensor, train: bool) -> Tensor {
        // let split: Vec<Tensor> = xs.split_with_sizes(&[14, 121, 121, 121], 1);

        // // Wide features
        // let wide = split[0];

        // // Deep features
        // let is_enemy_belligerent = split[1].view([-1, 11, 11, 1]);
        // let is_observed = split[2].view([-1, 11, 11, 1]);
        // let is_neutral = split[3].view([-1, 11, 11, 1]);

        

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

        xs
            .apply(&self.dense0)
            .relu()
            .dropout_(0.1, train)
            .apply(&self.dense1)
            .relu()
            .dropout_(0.1, train)
            // .apply(&self.dense2)
            // .relu()
            // .dropout_(0.1, train)
    }
}

impl Storable for DNN {
    fn store(self, path: &Path) -> Result<(),String> {
        self.vars.save(path)
            .map_err(|err| err.to_string())
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