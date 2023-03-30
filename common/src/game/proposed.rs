use serde::{Deserialize, Serialize};

use super::action::PlayerAction;

/// A proposed player action
///
/// The `outcome` characterizes what happens when the action is taken.
///
/// If the outcome is acceptable, use `Game::take_action` to realize it.
///
#[derive(Debug, Deserialize, Serialize)]
pub struct Proposed2<T> {
    pub action: PlayerAction,
    pub outcome: T,
}
