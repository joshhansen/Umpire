use std::{borrow::Cow, sync::Arc};

use async_trait::async_trait;
use delegate::delegate;
use futures;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as RwLockTokio;

use super::{
    action::{AiPlayerAction, PlayerAction, PlayerActionOutcome},
    ai::{fX, AISpec},
    map::dijkstra::Source,
    move_::Move,
    obs::ObsTracker,
    IGame, PlayerSecret, ProposedOrdersResult, ProposedUmpireResult, TurnPhase, TurnStart,
    UmpireResult,
};
use crate::{
    cli::Specified,
    game::{
        ai::TrainingInstance,
        city::City,
        map::tile::Tile,
        obs::Obs,
        unit::{orders::OrdersResult, Unit, UnitID, UnitType},
        Game, TurnNum,
    },
    util::{sparsify, Dims, Direction, Location, Wrap2d},
};

pub type PlayerNum = usize;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum PlayerType {
    Human,
    AI(AISpec),
}

impl PlayerType {
    pub fn values() -> [Self; 6] {
        [
            Self::Human,
            Self::AI(AISpec::Random),
            Self::AI(AISpec::FromLevel(1)),
            Self::AI(AISpec::FromLevel(2)),
            Self::AI(AISpec::FromLevel(3)),
            Self::AI(AISpec::FromLevel(4)),
        ]
    }
}

impl Specified for PlayerType {
    fn desc(&self) -> String {
        match self {
            Self::Human => String::from("human"),
            Self::AI(ai_type) => ai_type.desc(),
        }
    }

    fn spec(&self) -> String {
        match self {
            Self::Human => String::from("h"),
            Self::AI(ai_type) => ai_type.spec(),
        }
    }
}

impl TryFrom<String> for PlayerType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "h" | "human" => Ok(Self::Human),
            _ => AISpec::try_from(value).map(Self::AI),
        }
    }
}

impl Into<String> for PlayerType {
    fn into(self) -> String {
        match self {
            Self::Human => "h".to_string(),
            Self::AI(ai_type) => ai_type.into(),
        }
    }
}

pub enum TurnStartType {
    Regular,
    Clearing,
    None,
}

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

/// A player-specific layer around IGame that tracks the player's observations (view of the game world.)
///
/// Can only perform actions as the player whose secret is provided.
///
/// This is intended to be instantiated by clients; calls to `game` could cross network boundaries
/// and so should be minimized.
///
/// The `observations` cache is key to this. The server also tracks player observations (for now) and
/// the player's view should be eventually consistent with it.
pub struct PlayerControl {
    game: Arc<RwLockTokio<dyn IGame>>,
    pub player: PlayerNum,
    secret: PlayerSecret,
    observations: ObsTracker,
}

impl PlayerControl {
    pub async fn new(
        game: Arc<RwLockTokio<dyn IGame>>,
        player: PlayerNum,
        secret: PlayerSecret,
    ) -> Self {
        let dims = game.read().await.dims().await;
        Self::from_observations(game, player, secret, ObsTracker::new(dims))
    }

    pub fn from_observations(
        game: Arc<RwLockTokio<dyn IGame>>,
        player: PlayerNum,
        secret: PlayerSecret,
        observations: ObsTracker,
    ) -> Self {
        Self {
            game,
            player,
            secret,
            observations,
        }
    }

    delegate! {
        to self.game.write().await {
            /// TODO Update observations
            pub async fn activate_unit_by_loc(&mut self, [self.secret], loc: Location) -> UmpireResult<()>;

            /// TODO Update observations
            pub async fn begin_turn(&mut self, [self.secret]) -> UmpireResult<TurnStart>;

            /// TODO Update observations
            pub async fn clear_production(&mut self, [self.secret], loc: Location, ignore_cleared_production: bool) -> UmpireResult<Option<UnitType>>;

            /// TODO Update observations
            pub async fn clear_productions(&mut self, [self.secret], ignore_cleared_production: bool) -> UmpireResult<()>;

            /// TODO Update observations
            pub async fn disband_unit_by_id(&mut self, [self.secret], id: UnitID) -> UmpireResult<Unit>;

            /// TODO Update observations
            pub async fn end_turn(&mut self, [self.secret]) -> UmpireResult<()>;

            /// TODO Update observations
            pub async fn force_end_turn(&mut self, [self.secret]) -> UmpireResult<()>;

            /// TODO Update observations
            pub async fn order_unit_sentry(&mut self, [self.secret], unit_id: UnitID) -> OrdersResult;

            /// TODO Update observations
            pub async fn order_unit_skip(&mut self, [self.secret], unit_id: UnitID) -> OrdersResult;

            /// TODO Update observations
            pub async fn set_production_by_loc(&mut self, [self.secret], loc: Location, production: UnitType) -> UmpireResult<Option<UnitType>>;

            /// TODO Update observations
            pub async fn take_action(&mut self, [self.secret], action: PlayerAction) -> UmpireResult<PlayerActionOutcome>;

            /// TODO Update observations
            pub async fn take_simple_action(&mut self, [self.secret], action: AiPlayerAction) -> UmpireResult<PlayerActionOutcome>;
        }

        to self.game.read().await {
            pub async fn current_player(&self) -> PlayerNum;

            pub async fn dims(&self) -> Dims;

            #[unwrap]
            pub async fn is_player_turn(&self, [self.secret]) -> bool;

            pub async fn propose_move_unit_by_id(&self, [self.secret], id: UnitID, dest: Location) -> ProposedUmpireResult<Move>;

            pub async fn propose_order_unit_explore(&self, [self.secret], unit_id: UnitID) -> ProposedOrdersResult;

            pub async fn propose_order_unit_go_to(
                &self,
                [self.secret],
                unit_id: UnitID,
                dest: Location,
            ) -> ProposedOrdersResult;

            #[unwrap]
            pub async fn player_cities(&self, [self.secret]) -> Vec<City>;

            #[unwrap]
            pub async fn player_cities_producing_or_not_ignored(&self, [self.secret]) -> usize;

            #[unwrap]
            pub async fn player_city_by_loc(&self, [self.secret], loc: Location) -> Option<City>;

            #[unwrap]
            pub async fn player_production_set_requests(&self, [self.secret]) -> Vec<Location>;

            pub async fn player_score(&self, [self.secret]) -> UmpireResult<f64>;

            pub async fn current_player_score(&self) -> f64;

            #[unwrap]
            pub async fn player_toplevel_unit_by_loc(&self, [self.secret], loc: Location) -> Option<Unit>;

            #[unwrap]
            pub async fn player_unit_by_id(&self, [self.secret], id: UnitID) -> Option<Unit>;

            pub async fn player_unit_legal_directions(&self, [self.secret], unit_id: UnitID) -> UmpireResult<Vec<Direction>>;

            #[unwrap]
            pub async fn player_unit_orders_requests(&self, [self.secret]) -> Vec<UnitID>;

            #[unwrap]
            pub async fn player_unit_loc(&self, [self.secret], id: UnitID) -> Option<Location>;

            #[unwrap]
            pub async fn player_units(&self, [self.secret]) -> Vec<Unit>;

            pub async fn turn(&self) -> TurnNum;

            pub async fn turn_is_done(&self, [self.player], turn: TurnNum) -> UmpireResult<bool>;
            pub async fn current_turn_is_done(&self) -> bool;

            pub async fn turn_phase(&self) -> TurnPhase;

            #[unwrap]
            pub async fn valid_productions(&self, [self.secret], loc: Location) -> Vec<UnitType>;

            #[unwrap]
            pub async fn valid_productions_conservative(&self, [self.secret], loc: Location) -> Vec<UnitType>;

            pub async fn victor(&self) -> Option<PlayerNum>;

            pub async fn wrapping(&self) -> Wrap2d;
        }
    }

    pub async fn clone_underlying_game_state(&self) -> Result<Game, String> {
        self.game.read().await.clone_underlying_game_state()
    }

    pub async fn move_unit_by_id_in_direction(
        &mut self,
        id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .await
            .move_unit_by_id_in_direction(self.secret, id, direction)
            .await
            .map(|move_| {
                self.observations.track_many(move_.observations());
                move_
            })
    }

    /// The player's most recent observation at the given location
    pub async fn obs(&self, loc: Location) -> Obs {
        self.observations.get(loc).clone()
    }

    pub fn observations(&self) -> &ObsTracker {
        &self.observations
    }

    /// The tile at the given location, as present in the player's observations (or not)
    pub async fn tile(&self, loc: Location) -> Option<Cow<Tile>> {
        match self.observations.get(loc) {
            Obs::Observed { tile, .. } => Some(Cow::Borrowed(tile)),
            Obs::Unobserved => None,
        }
    }

    /// FIXME Implement ML feature vector generation
    /// Possibly split unit-relevant from city-relevant features
    /// FIXME Maintain this vector in the client, incrementally
    pub async fn player_features(&self) -> Vec<fX> {
        // player_features(self.game, self.secret).unwrap()
        // Vec::new()
        unimplemented!()
    }

    pub fn turn_ctrl(&mut self) -> PlayerTurn {
        PlayerTurn::new(self)
    }
}

/// Guard that begins a turn, then ends it on drop
pub struct PlayerTurn<'a> {
    ctrl: &'a mut PlayerControl,
}

impl<'a> PlayerTurn<'a> {
    pub fn new(ctrl: &'a mut PlayerControl) -> Self {
        let _turn_start = futures::executor::block_on(async { ctrl.begin_turn().await.unwrap() });
        Self { ctrl }
    }

    delegate! {
        to self.ctrl {
            // Mutable
            pub async fn activate_unit_by_loc(&mut self, loc: Location) -> UmpireResult<()>;

            // pub async fn begin_turn(&mut self) -> UmpireResult<TurnStart>;

            pub async fn clear_production(&mut self, loc: Location, ignore_cleared_production: bool) -> UmpireResult<Option<UnitType>>;

            pub async fn clear_productions(&mut self, ignore_cleared_production: bool) -> UmpireResult<()>;

            pub async fn disband_unit_by_id(&mut self, id: UnitID) -> UmpireResult<Unit>;

            // pub async fn end_turn(&mut self) -> UmpireResult<()>;

            // pub async fn force_end_turn(&mut self) -> UmpireResult<()>;

            pub async fn move_unit_by_id_in_direction(&mut self, id: UnitID, direction: Direction) -> UmpireResult<Move>;

            pub async fn order_unit_sentry(&mut self, unit_id: UnitID) -> OrdersResult;

            pub async fn order_unit_skip(&mut self,  unit_id: UnitID) -> OrdersResult;

            pub async fn set_production_by_loc(&mut self, loc: Location, production: UnitType) -> UmpireResult<Option<UnitType>>;

            pub async fn take_action(&mut self, action: PlayerAction) -> UmpireResult<PlayerActionOutcome>;

            pub async fn take_simple_action(&mut self, action: AiPlayerAction) -> UmpireResult<PlayerActionOutcome>;

            // Immutable
            pub async fn clone_underlying_game_state(&self) -> Result<Game, String>;

            pub async fn current_player(&self) -> PlayerNum;

            pub async fn dims(&self) -> Dims;

            pub async fn is_player_turn(&self) -> bool;

            pub async fn propose_move_unit_by_id(&self, id: UnitID, dest: Location) -> ProposedUmpireResult<Move>;

            pub async fn propose_order_unit_explore(&self, unit_id: UnitID) -> ProposedOrdersResult;

            pub async fn propose_order_unit_go_to(
                &self,
                unit_id: UnitID,
                dest: Location,
            ) -> ProposedOrdersResult;

            pub async fn obs(&self, loc: Location) -> Obs;

            pub async fn player_cities_producing_or_not_ignored(&self) -> usize;

            pub async fn player_city_by_loc(&self, loc: Location) -> Option<City>;

            pub async fn player_features(&self) -> Vec<fX>;

            pub async fn player_production_set_requests(&self) -> Vec<Location>;

            pub async fn player_score(&self) -> UmpireResult<f64>;

            pub async fn player_toplevel_unit_by_loc(&self, loc: Location) -> Option<Unit>;

            pub async fn player_unit_by_id(&self, id: UnitID) -> Option<Unit>;

            pub async fn player_unit_legal_directions(&self, unit_id: UnitID) -> UmpireResult<Vec<Direction>>;

            pub async fn player_unit_orders_requests(&self) -> Vec<UnitID>;

            pub async fn player_unit_loc(&self, id: UnitID) -> Option<Location>;

            pub async fn tile(&self, loc: Location) -> Option<Cow<Tile>>;

            pub async fn turn(&self) -> TurnNum;

            pub async fn turn_is_done(&self, turn: TurnNum) -> UmpireResult<bool>;
            pub async fn current_turn_is_done(&self) -> bool;

            pub async fn valid_productions(&self, loc: Location) -> Vec<UnitType>;

            pub async fn valid_productions_conservative(&self, loc: Location) -> Vec<UnitType>;

            pub async fn victor(&self) -> Option<PlayerNum>;

            pub async fn wrapping(&self) -> Wrap2d;
        }
    }
}

/// On drop, the turn ends
///
/// This forces the turn to end regardless of the state of production and orders requests.
impl<'a> Drop for PlayerTurn<'a> {
    fn drop(&mut self) {
        futures::executor::block_on(async {
            self.ctrl.force_end_turn().await.unwrap();
        });
    }
}

#[cfg(test)]
mod test {
    use crate::game::{test_support::game1, TurnPhase};

    use super::GameError;

    #[tokio::test]
    pub async fn test_player_turn_control() {
        let (mut game, secrets) = game1();

        assert_eq!(game.current_player(), 0);
        assert_eq!(game.turn(), 0);

        {
            let (ctrl, turn_start) = game.player_turn_control(secrets[0]).unwrap();

            assert_eq!(ctrl.turn().await, 0);
            assert_eq!(ctrl.current_player().await, 0);

            assert_eq!(turn_start.orders_results.len(), 0);
            assert_eq!(turn_start.production_outcomes.len(), 0);
        }

        assert_eq!(game.turn(), 0);
        assert_eq!(game.current_player(), 1);

        assert_eq!(
            game.player_turn_control(secrets[0]).unwrap_err(),
            GameError::NotPlayersTurn { player: 0 }
        );

        {
            let (ctrl, turn_start) = game.player_turn_control(secrets[1]).unwrap();

            assert_eq!(ctrl.turn().await, 0);
            assert_eq!(ctrl.current_player().await, 1);

            assert_eq!(turn_start.orders_results.len(), 0);
            assert_eq!(turn_start.production_outcomes.len(), 0);
        }

        assert_eq!(game.turn(), 1);
        assert_eq!(game.current_player(), 0);

        {
            let (ctrl, turn_start) = game.player_turn_control_nonending(secrets[0]).unwrap();

            assert_eq!(ctrl.turn().await, 1);
            assert_eq!(ctrl.current_player().await, 0);

            assert_eq!(turn_start.orders_results.len(), 0);
            assert_eq!(turn_start.production_outcomes.len(), 0);
        }

        assert_eq!(game.turn(), 1); // turn wasn't ended
        assert_eq!(game.current_player(), 0); // still player 0's turn

        {
            // Try starting the turn again when it's already been started
            let err = game.player_turn_control_nonending(secrets[0]).unwrap_err();
            assert_eq!(
                err,
                GameError::WrongPhase {
                    turn: 1,
                    player: 0,
                    phase: TurnPhase::Main
                }
            );
        }

        assert_eq!(game.turn(), 1); // turn wasn't ended
        assert_eq!(game.current_player(), 0); // still player 0's turn

        {
            let ctrl = game.player_turn_control_bare(secrets[0]).unwrap();

            assert_eq!(ctrl.turn().await, 1);
            assert_eq!(ctrl.current_player().await, 0);
        }

        assert_eq!(game.turn(), 1); // turn wasn't ended
        assert_eq!(game.current_player(), 0); // still player 0's turn
    }
}
