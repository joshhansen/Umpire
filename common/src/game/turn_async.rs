//! Turn-taking traits
//!
//! Implement the lowest-powered trait that suits your needs, and the higher-powered traits will be
//! implemented for you.
//!
//! From most to least powerful: TurnTakerSuperuser -> TurnTakerDIY -> TurnTaker -> ActionwiseTurnTaker

use std::sync::Arc;

use async_trait::async_trait;

use tokio::sync::RwLock as RwLockTokio;

use super::{
    action::AiPlayerAction,
    ai::TrainingInstance,
    player::{PlayerControl, PlayerTurn},
    turn::TurnOutcome,
    PlayerNum, PlayerSecret,
};
use crate::{game::Game, util::sparsify};

/// Take a turn, with all the superpowers, do whatever you want
#[async_trait]
pub trait TurnTakerSuperuser {
    async fn take_turn(
        &mut self,
        game: Arc<RwLockTokio<Game>>,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> TurnOutcome;
}

/// Take a turn, but you have to begin and end the turn yourself
#[async_trait]
pub trait TurnTakerDIY {
    async fn take_turn(&mut self, player: &mut PlayerControl, generate_data: bool) -> TurnOutcome;
}

/// Take a turn that has already been started for you, and will be ended
/// for you.
#[async_trait]
pub trait TurnTaker {
    async fn take_turn(&mut self, turn: &mut PlayerTurn, generate_data: bool) -> TurnOutcome;
}

/// Implements TurnTaker by indicating the next action the player should take, if any
#[async_trait]
pub trait ActionwiseTurnTaker {
    async fn next_action(&mut self, turn: &PlayerTurn) -> Option<AiPlayerAction>;
}

#[async_trait]
impl<T: ActionwiseTurnTaker + Send> TurnTaker for T {
    async fn take_turn(&mut self, turn: &mut PlayerTurn, generate_data: bool) -> TurnOutcome {
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let player = turn.current_player().await;
        let turn_num = turn.turn().await;

        println!("player: {}", player);
        println!("turn: {}", turn_num);

        loop {
            let (num_features, features, pre_score) = if generate_data {
                let (num_features, features) = sparsify(turn.player_features().await);
                (
                    Some(num_features),
                    Some(features),
                    Some(turn.player_score().await.unwrap()),
                )
            } else {
                (None, None, None)
            };

            if let Some(action) = self.next_action(turn).await {
                // If an action was specified...

                println!("Next action: {:?}", action);

                turn.take_simple_action(action).await.unwrap();

                if generate_data {
                    let post_score = turn.player_score().await.unwrap();
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

            if turn.turn_is_done(turn_num).await.unwrap() {
                break;
            }

            println!("looping");
        }

        TurnOutcome {
            training_instances,
            quit: false, // Only robots are using this trait and they never quit the game
        }
    }
}

#[async_trait]
impl<T: TurnTaker + Send> TurnTakerDIY for T {
    async fn take_turn(&mut self, player: &mut PlayerControl, generate_data: bool) -> TurnOutcome {
        let mut turn = player.turn_ctrl().await;

        let outcome = <Self as TurnTaker>::take_turn(self, &mut turn, generate_data).await;

        turn.force_end_turn().await.unwrap();

        outcome
    }
}

#[async_trait]
impl<T: TurnTakerDIY + Send> TurnTakerSuperuser for T {
    async fn take_turn(
        &mut self,
        game: Arc<RwLockTokio<Game>>,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> TurnOutcome {
        let mut ctrl = PlayerControl::new(game, player, secret).await;

        <Self as TurnTakerDIY>::take_turn(self, &mut ctrl, generate_data).await
    }
}
