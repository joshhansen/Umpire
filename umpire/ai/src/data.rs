use burn::{
    data::{dataloader::batcher::Batcher, dataset::Dataset},
    tensor::{backend::Backend, Int, Tensor},
};
use common::game::{
    action::AiPlayerAction,
    ai::{fX, TrainingOutcome},
    TurnNum,
};

#[derive(Clone, Debug)]
pub struct AgzDatum {
    pub features: Vec<fX>,
    pub action: AiPlayerAction,
    pub turns_until_outcome: TurnNum,
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

        let actions: Vec<i32> = items
            .iter()
            .map(|item| {
                let idx: usize = item.action.into();
                idx as i32
            })
            .collect();
        let actions: Tensor<B, 1, Int> = Tensor::from_ints(actions.as_slice(), &self.device);

        let targets: Vec<fX> = items
            .iter()
            .map(|item| item.outcome.to_training_target(item.turns_until_outcome))
            .collect();
        let targets: Tensor<B, 1> = Tensor::from_floats(targets.as_slice(), &self.device);

        AgzBatch {
            features,
            actions,
            targets,
        }
    }
}
