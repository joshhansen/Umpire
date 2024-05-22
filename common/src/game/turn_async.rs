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
    action::{AiPlayerAction, NextCityAction, NextUnitAction},
    ai::{TrainingFocus, TrainingInstance},
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
        clear_after_unit_production: bool,
        datagen_prob: Option<f64>,
    ) -> TurnOutcome;
}

/// Take a turn, but you have to begin and end the turn yourself
#[async_trait]
pub trait TurnTakerDIY {
    async fn take_turn(
        &mut self,
        player: &mut PlayerControl,
        clear_after_unit_production: bool,
        datagen_prob: Option<f64>,
    ) -> TurnOutcome;
}

/// Take a turn that has already been started for you, and will be ended
/// for you.
#[async_trait]
pub trait TurnTaker {
    async fn take_turn(&mut self, turn: &mut PlayerTurn, datagen_prob: Option<f64>) -> TurnOutcome;
}

/// Implements TurnTaker by indicating the next action the player should take, if any
#[async_trait]
pub trait ActionwiseTurnTaker {
    async fn next_action(&mut self, turn: &PlayerTurn) -> Option<AiPlayerAction>;
}

#[async_trait]
impl<T: ActionwiseTurnTaker + Send> TurnTaker for T {
    async fn take_turn(&mut self, turn: &mut PlayerTurn, datagen_prob: Option<f64>) -> TurnOutcome {
        let mut training_instances = datagen_prob.map(|_| Vec::new());

        let player = turn.current_player().await;
        let turn_num = turn.turn().await;

        loop {
            let pre_score = if datagen_prob.is_some() {
                Some(turn.player_score().await.unwrap())
            } else {
                None
            };

            if let Some(action) = self.next_action(turn).await {
                let (num_features, features) = if datagen_prob.is_some() {
                    // Determine if the spatial features should focus on the next city or the next unit
                    let focus = if NextCityAction::try_from(action).is_ok() {
                        TrainingFocus::City
                    } else {
                        TrainingFocus::Unit
                    };

                    let (num_features, features) = sparsify(turn.player_features(focus).await);
                    (Some(num_features), Some(features))
                } else {
                    (None, None)
                };

                // If an action was specified...
                turn.take_simple_action(action).await.unwrap();

                if let Some(datagen_prob) = datagen_prob {
                    let post_score = turn.player_score().await.unwrap();

                    if rand::random::<f64>() <= datagen_prob {
                        if let Some(v) = training_instances.as_mut() {
                            v.push(TrainingInstance::undetermined(
                                player,
                                num_features.unwrap(),
                                features.unwrap(),
                                turn_num,
                                pre_score.unwrap(),
                                action,
                                post_score,
                            ));
                        }
                    }
                }
            }

            if turn.turn_is_done(turn_num).await.unwrap() {
                break;
            }
        }

        TurnOutcome {
            training_instances,
            quit: false, // Only robots are using this trait and they never quit the game
        }
    }
}

/// Like ActionwiseTurnTaker, but determines city and unit actions separately
#[async_trait]
pub trait ActionwiseTurnTaker2 {
    async fn next_city_action(&mut self, turn: &PlayerTurn) -> Option<NextCityAction>;

    async fn next_unit_action(&mut self, turn: &PlayerTurn) -> Option<NextUnitAction>;
}

#[async_trait]
impl<T: ActionwiseTurnTaker2 + Send> ActionwiseTurnTaker for T {
    async fn next_action(&mut self, turn: &PlayerTurn) -> Option<AiPlayerAction> {
        if let Some(city_action) = self.next_city_action(turn).await {
            Some(city_action.into())
        } else {
            self.next_unit_action(turn)
                .await
                .map(|unit_action| unit_action.into())
        }
    }
}

#[async_trait]
impl<T: TurnTaker + Send> TurnTakerDIY for T {
    async fn take_turn(
        &mut self,
        player: &mut PlayerControl,
        clear_after_unit_production: bool,
        datagen_prob: Option<f64>,
    ) -> TurnOutcome {
        let mut turn = player.turn_ctrl(clear_after_unit_production).await;

        let outcome = <Self as TurnTaker>::take_turn(self, &mut turn, datagen_prob).await;

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
        clear_after_unit_production: bool,
        datagen_prob: Option<f64>,
    ) -> TurnOutcome {
        let mut ctrl = PlayerControl::new(game, player, secret).await;

        <Self as TurnTakerDIY>::take_turn(
            self,
            &mut ctrl,
            clear_after_unit_production,
            datagen_prob,
        )
        .await
    }
}
