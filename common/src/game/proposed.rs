use serde::{Deserialize, Serialize};

use super::{action::PlayerAction, player::PlayerGameView, Game};

/// A proposed change to the game state.
///
/// `delta` encapsulates the change, and `new_state` is the state as it will be after the change is applied.
#[derive(Debug)]
pub struct Proposed<T> {
    new_state: Game,
    pub delta: T,
}
impl<T> Proposed<T> {
    pub fn new(new_state: Game, delta: T) -> Self {
        Self { new_state, delta }
    }

    /// Apply the proposed change to the given game instance. This overwrites the game instance with `new_state`.
    pub fn apply(self, state: &mut Game) -> T {
        *state = self.new_state;
        self.delta
    }
}

/// A proposed player action
///
/// The `result` characterizes what happens when the action is taken
///
/// If the result is acceptable, use `Game::
///
#[derive(Deserialize, Serialize)]
pub struct Proposed2<T> {
    pub action: PlayerAction,
    pub result: T,
    pub game_view: PlayerGameView,
}
