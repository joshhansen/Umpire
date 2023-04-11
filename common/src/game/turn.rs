use std::sync::Arc;

use async_trait::async_trait;

use tokio::sync::RwLock as RwLockTokio;

use super::{
    action::AiPlayerAction,
    player::{PlayerControl, PlayerTurn},
    IGame, PlayerNum, PlayerSecret,
};
use crate::{
    game::{ai::TrainingInstance, Game},
    util::sparsify,
};

/// What's the meta-outcome of a TurnTaker taking a turn?
pub struct TurnOutcome {
    /// Training data generated during the turn, for ML purposes
    pub training_instances: Option<Vec<TrainingInstance>>,

    /// Indicate if the player quit the app
    pub quit: bool,
}
