use std::{fmt, path::Path};

use async_trait::async_trait;

use burn::prelude::*;

use burn_wgpu::{Wgpu, WgpuDevice};
use futures::lock::Mutex as MutexAsync;
use rand::rngs::StdRng;

use common::{
    game::{
        ai::{AISpec, AiDevice},
        player::PlayerTurn,
        turn::TurnOutcome,
        turn_async::TurnTaker as TurnTakerAsync,
    },
    util::init_rng,
};

pub trait Loadable<B: Backend>: Sized {
    fn load<P: AsRef<Path>>(path: P, device: B::Device) -> Result<Self, String>;
}

pub trait Storable {
    fn store(self, path: &Path) -> Result<(), String>;
}

pub trait StorableAsBytes {
    fn store_as_bytes(self) -> Result<Vec<u8>, String>;
}

pub trait LoadableFromBytes<B: Backend>: Sized {
    fn load_from_bytes<S: std::io::Read>(bytes: S, device: B::Device) -> Result<Self, String>;
}

// Sub-modules
pub mod agz;
pub mod data;

mod random;
mod skip;

use agz::AgzActionModel;

pub enum AI<B: Backend> {
    Random(RandomAI),

    Skip(SkipAI),

    /// AlphaGo Zero style action model
    AGZ(MutexAsync<AgzActionModel<B>>),
}

impl<B: Backend> AI<B> {
    pub fn random(rng: StdRng) -> Self {
        Self::Random(RandomAI::new(rng))
    }
}

impl<B: Backend> fmt::Debug for AI<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Random(_) => "random",
                Self::Skip(_) => "skip",
                Self::AGZ(_) => "agz",
            }
        )
    }
}

impl From<AISpec> for AI<Wgpu> {
    fn from(ai_type: AISpec) -> Self {
        match ai_type {
            AISpec::Random { seed } => Self::Random(RandomAI::new(init_rng(seed))),
            AISpec::Skip => AI::Skip(SkipAI {}),
            AISpec::FromPath { path, device } => {
                let device: WgpuDevice = device.into();
                Self::load(Path::new(path.as_str()), device).unwrap()
            }
            AISpec::FromLevel { level, device } => {
                let device: WgpuDevice = device.into();
                let agz = match level {
                    0 => {
                        let bytes = include_bytes!("../../../ai/agz/15x15/0.agz.bin");
                        AgzActionModel::<Wgpu>::load_from_bytes(bytes.as_slice(), device).unwrap()
                    }
                    level => unreachable!("Unsupported AI level: {}", level),
                };

                Self::AGZ(MutexAsync::new(agz))
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
            Self::Skip(_) => Err(String::from("Cannot store skip-only AI; load explicitly using the appropriate specification (s)")),
            Self::AGZ(agz) => agz.into_inner().store(path),
        }
    }
}

#[async_trait]
impl TurnTakerAsync for AI<Wgpu> {
    async fn take_turn(
        &mut self,
        turn: &mut PlayerTurn,
        datagen_prob: Option<f64>,
        device: AiDevice,
    ) -> TurnOutcome {
        match self {
            Self::Random(ai) => ai.take_turn(turn, datagen_prob, device).await,
            Self::Skip(ai) => ai.take_turn(turn, datagen_prob, device).await,
            Self::AGZ(agz) => agz.lock().await.take_turn(turn, datagen_prob, device).await,
        }
    }
}

// Exports
pub use random::RandomAI;
pub use skip::SkipAI;
