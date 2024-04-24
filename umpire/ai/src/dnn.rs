use std::{fmt, fs::File, path::Path};

use burn::{
    module::Module,
    nn::{conv::Conv2dConfig, DropoutConfig, LinearConfig, Relu},
    optim::Optimizer,
    prelude::*,
    tensor::activation::softplus,
};

use serde::{
    de::{self, Visitor},
    Deserialize, Serialize,
};

use common::game::{
    action::AiPlayerAction,
    ai::{
        BASE_CONV_FEATS, BASE_CONV_FEATS_USIZE, DEEP_HEIGHT, DEEP_HEIGHT_USIZE, DEEP_LEN,
        DEEP_OUT_LEN, DEEP_OUT_LEN_USIZE, DEEP_WIDTH, DEEP_WIDTH_USIZE, FEATS_LEN, FEATS_LEN_USIZE,
        POSSIBLE_ACTIONS, POSSIBLE_ACTIONS_USIZE, WIDE_LEN, WIDE_LEN_USIZE,
    },
    Game,
};

use crate::{LoadableFromBytes, StorableAsBytes};

use super::{Loadable, Storable};

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
pub struct DNNConfig {
    learning_rate: f64,
    possible_actions: usize,
}

impl DNNConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> DNN<B> {
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

        DNN {
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
pub struct DNN<B: Backend> {
    relu: nn::Relu,
    convs: Vec<nn::conv::Conv2d<B>>,
    dropouts: Vec<nn::Dropout>,
    dense0: nn::Linear<B>,
    dense1: nn::Linear<B>,
    dense2: nn::Linear<B>,
}

impl<B: Backend> DNN<B> {
    fn forward(&self, xs: &Tensor<B, 1>, train: bool) -> Tensor<B, 1> {
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

    // /// Two variables must be set:
    // /// * `learning_rate`: the DNN learning rate, f64
    // /// * `possible_actions`: the number of values to predict among, i64
    // pub fn new(device: Device, learning_rate: f64, possible_actions: i64) -> Result<Self, String> {
    //     let path = vars.root();

    //     let convs = vec![
    //         nn::conv2d(
    //             &path,
    //             BASE_CONV_FEATS,
    //             BASE_CONV_FEATS * 2,
    //             3,
    //             Default::default(),
    //         ), // -> 9x9
    //         nn::conv2d(
    //             &path,
    //             BASE_CONV_FEATS * 2,
    //             BASE_CONV_FEATS * 2,
    //             3,
    //             Default::default(),
    //         ), // -> 7x7
    //         nn::conv2d(
    //             &path,
    //             BASE_CONV_FEATS * 2,
    //             BASE_CONV_FEATS * 2,
    //             3,
    //             Default::default(),
    //         ), // -> 5x5
    //         nn::conv2d(
    //             &path,
    //             BASE_CONV_FEATS * 2,
    //             BASE_CONV_FEATS,
    //             3,
    //             Default::default(),
    //         ), // -> 3x3
    //     ];

    //     let dense0 = nn::linear(&path, WIDE_LEN + DEEP_OUT_LEN, 64, Default::default());
    //     let dense1 = nn::linear(&path, 64, 32, Default::default());
    //     let dense2 = nn::linear(&path, 32, possible_actions, Default::default());

    //     let optimizer = nn::Adam::default()
    //         .build(&vars, learning_rate)
    //         .map_err(|err| err.to_string())?;

    //     Ok(Self {
    //         learning_rate,
    //         possible_actions,
    //         convs,
    //         dense0,
    //         dense1,
    //         dense2,
    //         optimizer,
    //     })
    // }

    // pub fn train(&mut self, features: &Tensor, action: &usize, value: f64) {
    //     let actual_estimate: Tensor =
    //         self.forward_t(features, true)
    //             .slice(0, *action as i64, *action as i64 + 1, 1);

    //     let value_tensor = Tensor::from(value as f32).to_device(self.vars.device());

    //     let loss: Tensor = actual_estimate.mse_loss(&value_tensor, Reduction::None);

    //     debug_assert!(loss.device().is_cuda());

    //     self.optimizer.backward_step(&loss);
    // }

    // pub fn evaluate_tensors(&self, features: &Tensor) -> Vec<f64> {
    //     let result_tensor = <Self as nn::ModuleT>::forward_t(self, &features, false);

    //     debug_assert!(result_tensor.device().is_cuda());

    //     result_tensor.try_into().unwrap()
    // }

    // pub fn evaluate_tensor(&self, features: &Tensor, action: &usize) -> f64 {
    //     let result_tensor = <Self as nn::ModuleT>::forward_t(self, &features, false);

    //     debug_assert!(result_tensor.device().is_cuda());

    //     result_tensor.double_value(&[*action as i64])
    // }
}

// impl StateActionFunction<Game, usize> for DNN {
//     type Output = f64;

//     fn evaluate(&self, state: &Game, action: &usize) -> Self::Output {
//         let features = self.tensor_for(state);

//         self.evaluate_tensor(&features, action)
//     }

//     fn update_with_error(
//         &mut self,
//         state: &Game,
//         action: &usize,
//         value: Self::Output,
//         _estimate: Self::Output,
//         _error: Self::Output,
//         _raw_error: Self::Output,
//         _learning_rate: Self::Output,
//     ) {
//         let features = self.tensor_for(state);

//         self.train(&features, action, value)
//     }
// }

// impl EnumerableStateActionFunction<Game> for DNN {
//     fn n_actions(&self) -> usize {
//         AiPlayerAction::possible_actions().len()
//     }

//     fn evaluate_all(&self, state: &Game) -> Vec<f64> {
//         (0..self.n_actions())
//             .map(|action_idx| self.evaluate(state, &action_idx))
//             .collect()
//     }

//     fn update_all_with_errors(
//         &mut self,
//         state: &Game,
//         values: Vec<f64>,
//         estimates: Vec<f64>,
//         errors: Vec<f64>,
//         raw_errors: Vec<f64>,
//         learning_rate: f64,
//     ) {
//         for (i, value) in values.iter().enumerate() {
//             self.update_with_error(
//                 state,
//                 &i,
//                 *value,
//                 estimates[i],
//                 errors[i],
//                 raw_errors[i],
//                 learning_rate,
//             );
//         }
//     }
// }

impl<B: Backend> Loadable for DNN<B> {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!(
                "Can't load DNN from path '{:?}' because it doesn't exist",
                path
            ));
        }

        let r = File::open(path).map_err(|err| err.to_string())?;

        Self::load_from_bytes(r)
    }
}

// impl nn::ModuleT for DNN {
//     fn forward_t(&self, xs: &Tensor, train: bool) -> Tensor {
//         let split: Vec<Tensor> = xs.split_with_sizes(&[WIDE_LEN, DEEP_LEN], 0);

//         // Wide featuers that will pass through to the dense layers directly
//         let wide = &split[0];

//         // Input features to the 2d convolution
//         let mut deep = split[1].view([1, BASE_CONV_FEATS, DEEP_WIDTH, DEEP_HEIGHT]);

//         for conv in &self.convs {
//             deep = deep.apply(conv).relu().dropout(0.4, train);
//         }

//         // Reshape back to vector
//         deep = deep.view([-1]);

//         let wide_and_deep = Tensor::cat(&[wide, &deep], 0);

//         debug_assert!(wide_and_deep.device().is_cuda());

//         // println!("Wide and deep shape: {:?}", wide_and_deep.size());

//         // xs.view([-1, 1, 28, 28])
//         //     .apply(&self.conv1)
//         //     .max_pool2d_default(2)
//         //     .apply(&self.conv2)
//         //     .max_pool2d_default(2)
//         //     .view([-1, 1024])
//         //     .apply(&self.fc1)
//         //     .relu()
//         //     .dropout_(0.5, train)
//         //     .apply(&self.fc2)

//         wide_and_deep
//             .apply(&self.dense0)
//             .relu()
//             // .dropout_(0.2, train)
//             .apply(&self.dense1)
//             .relu()
//             // .dropout_(0.2, train)
//             .apply(&self.dense2)
//             .softplus()
//     }
// }

impl<B: Backend> Storable for DNN<B> {
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
