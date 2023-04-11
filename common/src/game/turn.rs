use crate::game::ai::TrainingInstance;

/// What's the meta-outcome of a TurnTaker taking a turn?
pub struct TurnOutcome {
    /// Training data generated during the turn, for ML purposes
    pub training_instances: Option<Vec<TrainingInstance>>,

    /// Indicate if the player quit the app
    pub quit: bool,
}
