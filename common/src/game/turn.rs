use async_trait::async_trait;

use super::{
    action::AiPlayerAction,
    player::{PlayerControl, PlayerTurn},
    PlayerNum, PlayerSecret,
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

/// Take player turns. Acts directly on a Game instance, so it's for local usage only.
pub trait TurnTakerRaw {
    fn take_turn_not_clearing(
        &mut self,
        game: &mut Game,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> TurnOutcome {
        self.take_turn(game, player, secret, false, generate_data)
    }

    fn take_turn_clearing(
        &mut self,
        game: &mut Game,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> TurnOutcome {
        self.take_turn(game, player, secret, true, generate_data)
    }

    /// Take a complete turn for the specified player
    fn take_turn(
        &mut self,
        game: &mut Game,
        player: PlayerNum,
        secret: PlayerSecret,
        clear_productions_at_start_of_turn: bool,
        generate_data: bool,
    ) -> TurnOutcome;
}

/// Take a turn with only the knowledge of game state an individual player should have
/// This is the main thing to use
///
/// PlayerTurn will have already started the turn
///
/// # Arguments
/// * generate_data: whether or not training data for a state-action-value model should be returned
#[async_trait]
pub trait LimitedTurnTaker {
    async fn take_turn(&mut self, turn: &mut PlayerTurn, generate_data: bool) -> TurnOutcome;
}

/// Take a turn with full knowledge of the game state
///
/// This is a kludgey escape hatch for an issue in AI training where we need the whole state. It is crucial for
/// implementors to guarantee that the player's turn is ended (and only the player's turn---no further turns) by the
/// end of the `take_turn` function call.
#[async_trait]
pub trait TurnTaker {
    async fn take_turn_not_clearing(
        &mut self,
        ctrl: &mut PlayerControl,
        generate_data: bool,
    ) -> TurnOutcome {
        self.take_turn(ctrl, false, generate_data).await
    }

    async fn take_turn_clearing(
        &mut self,
        ctrl: &mut PlayerControl,
        generate_data: bool,
    ) -> TurnOutcome {
        self.take_turn(ctrl, true, generate_data).await
    }

    async fn take_turn(
        &mut self,
        ctrl: &mut PlayerControl,
        clear_productions_at_start_of_turn: bool,
        generate_data: bool,
    ) -> TurnOutcome;
}

#[async_trait]
impl<T: LimitedTurnTaker + Send> TurnTaker for T {
    async fn take_turn(
        &mut self,
        ctrl: &mut PlayerControl,
        clear_productions_at_start_of_turn: bool,
        generate_data: bool,
    ) -> TurnOutcome {
        let turn = ctrl.turn().await;

        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let mut quit = false;

        {
            let mut turn_ctrl = ctrl.turn_ctrl();

            if clear_productions_at_start_of_turn {
                turn_ctrl.clear_productions(false).await.unwrap();
            }

            loop {
                let result =
                    <Self as LimitedTurnTaker>::take_turn(self, &mut turn_ctrl, generate_data)
                        .await;

                if let Some(mut instances) = result.training_instances {
                    training_instances
                        .as_mut()
                        .map(|v| v.append(&mut instances));
                }

                if result.quit {
                    quit = true;
                    break;
                }

                if turn_ctrl.turn_is_done(turn).await.unwrap() {
                    break;
                }
            }
        }; // turn ends on drop

        TurnOutcome {
            training_instances,
            quit,
        }
    }
}

#[async_trait]
pub trait ActionwiseLimitedTurnTaker {
    /// The next action that should be taken
    ///
    /// Return None if there are no actions that should be taken
    async fn next_action(&mut self, ctrl: &PlayerTurn<'_>) -> Option<AiPlayerAction>;
}

#[async_trait]
impl<T: ActionwiseLimitedTurnTaker + Send + Sync> LimitedTurnTaker for T {
    async fn take_turn(&mut self, ctrl: &mut PlayerTurn, generate_data: bool) -> TurnOutcome {
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let player = ctrl.current_player().await;
        let turn = ctrl.turn().await;

        loop {
            let (num_features, features, pre_score) = if generate_data {
                let (num_features, features) = sparsify(ctrl.player_features().await);
                (
                    Some(num_features),
                    Some(features),
                    Some(ctrl.player_score().await.unwrap()),
                )
            } else {
                (None, None, None)
            };

            if let Some(action) = self.next_action(ctrl).await {
                // If an action was specified...

                ctrl.take_simple_action(action).await.unwrap();

                if generate_data {
                    let post_score = ctrl.player_score().await.unwrap();
                    training_instances.as_mut().map(|v| {
                        v.push(TrainingInstance::undetermined(
                            player,
                            num_features.unwrap(),
                            features.unwrap(),
                            pre_score.unwrap(),
                            action,
                            post_score,
                        ));
                    });
                }
            }

            if ctrl.turn_is_done(turn).await.unwrap() {
                break;
            }
        }

        TurnOutcome {
            training_instances,
            quit: false, // Only robots are using this trait and they never quit the game
        }
    }
}

pub trait ActionwiseTurnTaker {
    fn next_action(
        &self,
        game: &Game,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> Option<AiPlayerAction>;
}

impl<T: ActionwiseTurnTaker> TurnTakerRaw for T {
    fn take_turn(
        &mut self,
        ctrl: &mut Game,
        player: PlayerNum,
        secret: PlayerSecret,
        _clear_productions_at_start_of_turn: bool,
        generate_data: bool,
    ) -> TurnOutcome {
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let turn = ctrl.turn();

        loop {
            let (num_features, features, pre_score) = if generate_data {
                let (num_features, features) = sparsify(ctrl.player_features(secret).unwrap());
                (
                    Some(num_features),
                    Some(features),
                    Some(ctrl.player_score(secret).unwrap()),
                )
            } else {
                (None, None, None)
            };

            if let Some(action) = self.next_action(ctrl, player, secret, generate_data) {
                // If an action was specified...

                ctrl.take_simple_action(secret, action).unwrap();

                if generate_data {
                    let post_score = ctrl.player_score(secret).unwrap();
                    training_instances.as_mut().map(|v| {
                        v.push(TrainingInstance::undetermined(
                            player,
                            num_features.unwrap(),
                            features.unwrap(),
                            pre_score.unwrap(),
                            action,
                            post_score,
                        ));
                    });
                }
            }

            if ctrl.turn_is_done(player, turn).unwrap() {
                break;
            }
        }

        TurnOutcome {
            training_instances,
            quit: false, // Only robots are using this trait and they never quit the game
        }
    }
}
