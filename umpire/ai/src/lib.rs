use std::{fmt, path::Path};

use async_trait::async_trait;

use burn::prelude::*;

use burn_autodiff::Autodiff;
use burn_wgpu::{Wgpu, WgpuDevice};
use futures::lock::Mutex as MutexAsync;

use common::game::{
    action::AiPlayerAction, ai::AISpec, player::PlayerTurn, turn::TurnOutcome,
    turn_async::TurnTaker as TurnTakerAsync,
};

pub type AiBackend = Wgpu;
pub type AiBackendTrain = Autodiff<AiBackend>;
pub const fn default_device() -> <AiBackend as Backend>::Device {
    WgpuDevice::BestAvailable
}

pub trait Loadable<B: Backend>: Sized {
    fn load<P: AsRef<Path>>(path: P, device: B::Device) -> Result<Self, String>;
}

pub trait Storable {
    fn store(self, path: &Path) -> Result<(), String>;
}

pub trait StorableAsBytes {
    fn store_as_bytes(self) -> Result<Vec<u8>, String>;
}

pub trait LoadableFromBytes: Sized {
    fn load_from_bytes<S: std::io::Read>(bytes: S) -> Result<Self, String>;
}

// Sub-modules
pub mod agz;
pub mod data;

mod random;

use agz::AgzActionModel;

pub enum AI<B: Backend> {
    Random(RandomAI),

    /// AlphaGo Zero style action model
    AGZ(MutexAsync<AgzActionModel<B>>),
}

impl<B: Backend> AI<B> {
    pub fn random(verbosity: usize, fix_output_loc: bool) -> Self {
        Self::Random(RandomAI::new(verbosity, fix_output_loc))
    }
}

impl<B: Backend> fmt::Debug for AI<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Random(_) => "random",
                Self::AGZ(_) => "agz",
            }
        )
    }
}

impl<B: Backend> From<AISpec> for AI<B> {
    fn from(ai_type: AISpec) -> Self {
        match ai_type {
            AISpec::Random => Self::Random(RandomAI::new(0, false)), //NOTE Assuming 0 verbosity
            AISpec::FromPath(path) => {
                let device: B::Device = Default::default();
                Self::load(Path::new(path.as_str()), device).unwrap()
            }
            AISpec::FromLevel(level) => {
                // let lfa: LFA_ = match level {
                //     1 => bincode::deserialize(include_bytes!(
                //         "../../../ai/lfa/10x10_e100_s100000_a__scorefix__turnpenalty.ai"
                //     ))
                //     .unwrap(),
                //     2 => bincode::deserialize(include_bytes!(
                //         "../../../ai/lfa/20x20_e100_s100000_a__scorefix__turnpenalty.ai"
                //     ))
                //     .unwrap(),
                //     3 => bincode::deserialize(include_bytes!(
                //         "../../../ai/lfa/10-30_e100_s100000_a__scorefix__turnpenalty.ai"
                //     ))
                //     .unwrap(),
                //     4 => bincode::deserialize(include_bytes!(
                //         "../../../ai/lfa/10-40+full_e100_s100000_a.ai"
                //     ))
                //     .unwrap(),
                //     level => unreachable!("Unsupported AI level: {}", level),
                // };
                // Self::LFA(lfa)
                panic!()
            }
        }
    }
}

impl<B: Backend> Loadable<B> for AI<B> {
    /// Loads the actual AI instance from a file.
    ///
    /// With feature "pytorch" enabled, files ending with .agz will be deserialized as AlphaGo Zero
    /// style action models (`AI::AGZ`).
    ///
    /// With feature "pytorch" enabled, files ending with .deep will be deserialized as an `rsrl`
    /// Q-learning model with DNN action model (`AI::DNN`).
    ///
    /// Everything else will be loaded as an `rsrl` Q-learning model with a linear action model (`AI::LFA`).
    fn load<P: AsRef<Path>>(path: P, device: B::Device) -> Result<Self, String> {
        if !path.as_ref().exists() {
            return Err(format!(
                "Could not load AI from path '{:?}' because it doesn't exist",
                path.as_ref()
            ));
        }

        if path.as_ref().to_string_lossy().contains(".agz") {
            return AgzActionModel::load(path, device).map(|agz| Self::AGZ(MutexAsync::new(agz)));
        }

        panic!("Could not load AI from path {}", path.as_ref().display());

        // #[cfg(feature = "pytorch")]
        // if path.as_ref().extension().map(|ext| ext.to_str()) == Some(Some("deep")) {
        //     return DNN::load(path).map(|dnn| Self::DNN(Mutex::new(dnn)));
        // }

        // let f = File::open(path).unwrap(); //NOTE unwrap on file open
        // let result: Result<LFA_, String> =
        //     bincode::deserialize_from(f).map_err(|err| format!("{}", err));
        // result.map(Self::LFA)
    }
}

impl<B: Backend> Storable for AI<B> {
    fn store(self, path: &Path) -> Result<(), String> {
        match self {
            Self::Random(_) => Err(String::from("Cannot store random AI; load explicitly using the appropriate specification (r/rand/random)")),
            Self::AGZ(agz) => agz.into_inner().store(path),
        }
    }
}

#[async_trait]
impl<B: Backend> TurnTakerAsync for AI<B> {
    async fn take_turn<R: RngCore + Send>(
        &mut self,
        rng: &mut R,
        turn: &mut PlayerTurn,
        datagen_prob: Option<f64>,
    ) -> TurnOutcome {
        match self {
            Self::Random(ai) => ai.take_turn(rng, turn, datagen_prob).await,
            Self::AGZ(agz) => agz.lock().await.take_turn(rng, turn, datagen_prob).await,
        }
    }
}

use rand::RngCore;
// Exports
pub use random::RandomAI;
