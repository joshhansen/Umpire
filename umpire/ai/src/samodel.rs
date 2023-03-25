use tch::{
    Device,
    nn::{
        self,
        Optimizer,
        OptimizerConfig,
    },
    Tensor,
};

use crate::{
    game::{
        Game,
    }, util::to_vec_f32,
};
use super::{rl::POSSIBLE_ACTIONS, dnn::{DEEP_WIDTH, BASE_CONV_FEATS, DEEP_HEIGHT, WIDE_LEN, DEEP_LEN}, TrainingOutcome, UmpireAction};

#[derive(Debug)]
pub struct StateActionValueModel {
    vars: nn::VarStore,
    convs: Vec<nn::Conv2D>,
    dense0: nn::Linear,
    dense1: nn::Linear,
    dense2: nn::Linear,
    optimizer: Optimizer<nn::Adam>,
}

impl StateActionValueModel {
    pub fn train(&mut self, features: Vec<f64>, action: UmpireAction, victorious: bool) {
        let features = to_vec_f32(features);
        let t = Tensor::try_from(features).unwrap().to_device(Device::cuda_if_available());
        
    }

    
    pub fn victory_probs(&self, state: &Game) -> Vec<f64> {
        let features = self.tensor_for(state);
        let result_tensor = <Self as nn::ModuleT>::forward_t(self, &features, true);
        let mut result: Vec<f64> = Vec::with_capacity(POSSIBLE_ACTIONS);
        result_tensor.copy_data(&mut result[..], POSSIBLE_ACTIONS);
        result
    }

    fn tensor_for(&self, state: &Game) -> Tensor {
        //NOTE We could avoid this extra allocation if we could figure out how to use 64-bit weights in PyTorch
        //     or 32-bit weights in `rsrl`
        let features_f64 = state.features();
        let features = to_vec_f32(features_f64);
        Tensor::try_from(features).unwrap().to_device(Device::cuda_if_available())
    }

    pub fn new(learning_rate: f32) -> Result<Self,String> {
        let device = Device::cuda_if_available();
        println!("Device: {:?}", device);
        let vars = nn::VarStore::new(device);


        let mut lr = vars.root().zeros_no_train("learning_rate", &[1]);
        lr.copy_(
            &Tensor::try_from(vec![learning_rate])
            .map_err(|err| format!("Cant' create DNN because the learning rate could not be encoded as a tensor: {}", err))?
        );
        
        Self::with_varstore(vars)
    }

    fn with_varstore(vars: nn::VarStore) -> Result<Self,String> {
        let path = vars.root();

        let learning_rate = 10e-3_f64;

        // let learning_rate: f64 = path
        //     .get("learning_rate")
        //     .ok_or(format!("Learning rate not set in VarStore"))?
        //     .double_value(&[0]);
        

        let conv0 = nn::conv2d(&path, 1, BASE_CONV_FEATS, 3, Default::default());// -> 9x9
        let conv1 = nn::conv2d(&path, BASE_CONV_FEATS, BASE_CONV_FEATS*2, 3, Default::default());// -> 7x7
        let conv2 = nn::conv2d(&path, BASE_CONV_FEATS*2, BASE_CONV_FEATS*4, 3, Default::default());// -> 5x5
        let conv3 = nn::conv2d(&path, BASE_CONV_FEATS*4, BASE_CONV_FEATS*8, 3, Default::default());// -> 3x3

        let convs = vec![conv0, conv1, conv2, conv3];

        let dense0 = nn::linear(&path, 2329, 256, Default::default());
        let dense1 = nn::linear(&path, 256, 128, Default::default());
        let dense2 = nn::linear(&path, 128, POSSIBLE_ACTIONS as i64, Default::default());

        let optimizer = nn::Adam::default().build(&vars, learning_rate)
            .map_err(|err| err.to_string())
        ?;

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

impl nn::ModuleT for StateActionValueModel {
    fn forward_t(&self, xs: &Tensor, _train: bool) -> Tensor {

        let split: Vec<Tensor> = xs.split_with_sizes(&[WIDE_LEN, DEEP_LEN, DEEP_LEN, DEEP_LEN, DEEP_LEN], 0);

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

        let wide_and_deep = Tensor::cat(&[
            wide,
            &enemy_feats,
            &observed_feats,
            &neutral_feats,
            &city_feats
        ], 0);

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
            .sigmoid()
            // .dropout_(0.1, train)
    }
}