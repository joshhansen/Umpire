use burn::{
    data::{dataloader::batcher::Batcher, dataset::Dataset},
    prelude::*,
};
use common::game::ai::{fX, TrainingOutcome};

#[derive(Clone, Debug)]
pub struct AgzDatum {
    pub features: Vec<fX>,
    pub action: usize,
    pub outcome: TrainingOutcome,
}

pub struct AgzData {
    data: Vec<AgzDatum>,
}

impl AgzData {
    pub fn new(data: Vec<AgzDatum>) -> Self {
        Self { data }
    }
}

impl Dataset<AgzDatum> for AgzData {
    fn get(&self, index: usize) -> Option<AgzDatum> {
        self.data.get(index).cloned()
    }
    fn len(&self) -> usize {
        self.data.len()
    }
}

#[derive(Clone)]
pub struct AgzBatcher<B: Backend> {
    device: B::Device,
}

impl<B: Backend> AgzBatcher<B> {
    pub fn new(device: B::Device) -> Self {
        Self { device }
    }
}

/**
 * A batch of AlphaGo Zero style state-action victory probabilities.
 *
 * The features here include at the end an "action feature" - a way of encoding the action taken.
 *
 * This should be used in training to target the correct output of the model and update only based on that gradient.
*/
#[derive(Clone, Debug)]
pub struct AgzBatch<B: Backend> {
    /// [batch_size, feature_idx]
    pub features: Tensor<B, 2>,

    /// [batch_size]
    pub actions: Tensor<B, 1, Int>,

    /// [batch_size]
    pub targets: Tensor<B, 1>,
}

impl<B: Backend> Batcher<AgzDatum, AgzBatch<B>> for AgzBatcher<B> {
    fn batch(&self, items: Vec<AgzDatum>) -> AgzBatch<B> {
        let features = items
            .iter()
            .map(|item| {
                let feats = Tensor::from_floats(item.features.as_slice(), &self.device);
                feats.reshape([1, -1])
            })
            .collect();

        let features = Tensor::cat(features, 0).to_device(&self.device);

        let actions: Vec<i32> = items.iter().map(|item| item.action as i32).collect();
        let actions: Tensor<B, 1, Int> = Tensor::from_ints(actions.as_slice(), &self.device);

        let targets: Vec<fX> = items
            .iter()
            .map(|item| item.outcome.to_training_target())
            .collect();
        let targets: Tensor<B, 1> = Tensor::from_floats(targets.as_slice(), &self.device);

        AgzBatch {
            features,
            actions,
            targets,
        }
    }
}
