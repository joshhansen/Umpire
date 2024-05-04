use burn::{
    data::{dataloader::batcher::Batcher, dataset::Dataset},
    prelude::*,
};
use common::game::ai::{fX, TrainingOutcome};

#[derive(Clone, Debug)]
pub struct AgzDatum {
    pub features_including_action: Vec<fX>,
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
 * The features here include at the end an "action feauture" - a way of encoding the action taken.
 *
 * This should be used in training to target the correct output of the model and update only based on that gradient.
*/
#[derive(Clone, Debug)]
pub struct AgzBatch<B: Backend> {
    /// [batch_size, feature_idx]
    pub data: Tensor<B, 2>,

    /// [batch_size]
    pub targets: Tensor<B, 1>,
}

impl<B: Backend> Batcher<AgzDatum, AgzBatch<B>> for AgzBatcher<B> {
    fn batch(&self, items: Vec<AgzDatum>) -> AgzBatch<B> {
        let data = items
            .iter()
            .map(|item| {
                let feats =
                    Tensor::from_floats(item.features_including_action.as_slice(), &self.device);
                feats.reshape([1, -1])
            })
            .collect();

        let data = Tensor::cat(data, 0).to_device(&self.device);

        let targets = items
            .into_iter()
            .map(|item| {
                Tensor::from_floats([item.outcome.to_training_target() as f32], &self.device)
            })
            .collect();

        let targets = Tensor::cat(targets, 0).to_device(&self.device);

        // let images = items
        //     .iter()
        //     .map(|item| Data::<f32, 2>::from(item.image))
        //     .map(|data| Tensor::<B, 2>::from_data(data.convert(), &self.device))
        //     .map(|tensor| tensor.reshape([1, 28, 28]))
        //     // Normalize: make between [0,1] and make the mean=0 and std=1
        //     // values mean=0.1307,std=0.3081 are from the PyTorch MNIST example
        //     // https://github.com/pytorch/examples/blob/54f4572509891883a947411fd7239237dd2a39c3/mnist/main.py#L122
        //     .map(|tensor| ((tensor / 255) - 0.1307) / 0.3081)
        //     .collect();

        // let targets = items
        //     .iter()
        //     .map(|item| {
        //         Tensor::<B, 1, Int>::from_data(
        //             Data::from([(item.label as i64).elem()]),
        //             &self.device,
        //         )
        //     })
        //     .collect();

        // let images = Tensor::cat(images, 0).to_device(&self.device);
        // let targets = Tensor::cat(targets, 0).to_device(&self.device);

        AgzBatch { data, targets }
    }
}
