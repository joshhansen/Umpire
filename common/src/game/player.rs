use std::{borrow::Cow, cell::RefCell, fmt, rc::Rc};

use async_trait::async_trait;
use delegate::delegate;
use futures;
use serde::{Deserialize, Serialize};

use super::{
    action::{AiPlayerAction, PlayerAction, PlayerActionOutcome},
    ai::{fX, AISpec},
    map::dijkstra::Source,
    move_::Move,
    obs::ObsTracker,
    IGame, PlayerSecret, ProposedOrdersResult, ProposedUmpireResult, TurnStart, UmpireResult,
};
use crate::{
    cli::Specified,
    game::{
        ai::TrainingInstance,
        city::City,
        map::tile::Tile,
        obs::Obs,
        unit::{orders::OrdersResult, Unit, UnitID, UnitType},
        Game, GameError, TurnNum,
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

pub struct PlayerTurnControl<'a> {
    game: &'a mut dyn IGame,

    secret: PlayerSecret,

    observations: ObsTracker,

    end_turn_on_drop: bool,
}
impl<'a> PlayerTurnControl<'a> {
    pub async fn new(
        game: &'a mut dyn IGame,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        Self::_new(game, secret, TurnStartType::Regular, true)
            .await
            .map(|(ctrl, start)| (ctrl, start.unwrap()))
    }

    pub async fn new_clearing(
        game: &'a mut dyn IGame,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        Self::_new(game, secret, TurnStartType::Clearing, true)
            .await
            .map(|(ctrl, start)| (ctrl, start.unwrap()))
    }

    pub async fn new_nonending(
        game: &'a mut dyn IGame,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        Self::_new(game, secret, TurnStartType::Clearing, false)
            .await
            .map(|(ctrl, start)| (ctrl, start.unwrap()))
    }

    /// Does not start the turn and does not end it on drop
    pub async fn new_bare(
        game: &'a mut dyn IGame,
        secret: PlayerSecret,
    ) -> UmpireResult<PlayerTurnControl<'a>> {
        Self::_new(game, secret, TurnStartType::None, false)
            .await
            .map(|x| x.0)
    }

    pub async fn _new(
        game: &'a mut dyn IGame,
        secret: PlayerSecret,
        turn_start_type: TurnStartType,
        end_turn_on_drop: bool,
    ) -> UmpireResult<(PlayerTurnControl<'a>, Option<TurnStart>)> {
        let turn_start = match turn_start_type {
            TurnStartType::Regular => Some(game.begin_turn(secret).await?),
            TurnStartType::Clearing => Some(game.begin_turn_clearing(secret).await?),
            TurnStartType::None => None,
        };

        let observations = game.player_observations(secret).await?;
        Ok((
            Self {
                game,
                secret,
                observations,
                end_turn_on_drop,
            },
            turn_start,
        ))
    }

    pub fn new_sync(
        game: &'a mut Game,
        secret: PlayerSecret,
        initial_observations: Option<ObsTracker>,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        Self::_new_sync(
            game,
            secret,
            TurnStartType::Regular,
            true,
            initial_observations,
        )
        .map(|(ctrl, start)| (ctrl, start.unwrap()))
    }

    pub fn new_sync_clearing(
        game: &'a mut Game,
        secret: PlayerSecret,
        initial_observations: Option<ObsTracker>,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        Self::_new_sync(
            game,
            secret,
            TurnStartType::Clearing,
            true,
            initial_observations,
        )
        .map(|(ctrl, start)| (ctrl, start.unwrap()))
    }

    pub fn new_sync_nonending(
        game: &'a mut Game,
        secret: PlayerSecret,
        initial_observations: Option<ObsTracker>,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        Self::_new_sync(
            game,
            secret,
            TurnStartType::Clearing,
            false,
            initial_observations,
        )
        .map(|(ctrl, start)| (ctrl, start.unwrap()))
    }

    /// Does not start the turn and does not end it on drop
    pub fn new_sync_bare(
        game: &'a mut Game,
        secret: PlayerSecret,
        initial_observations: Option<ObsTracker>,
    ) -> UmpireResult<PlayerTurnControl<'a>> {
        Self::_new_sync(
            game,
            secret,
            TurnStartType::None,
            false,
            initial_observations,
        )
        .map(|x| x.0)
    }

    fn _new_sync(
        game: &'a mut Game,
        secret: PlayerSecret,
        turn_start_type: TurnStartType,
        end_turn_on_drop: bool,
        initial_observations: Option<ObsTracker>,
    ) -> UmpireResult<(PlayerTurnControl<'a>, Option<TurnStart>)> {
        let turn_start = match turn_start_type {
            TurnStartType::Regular => Some(game.begin_turn(secret)?),
            TurnStartType::Clearing => Some(game.begin_turn_clearing(secret)?),
            TurnStartType::None => None,
        };

        let dims = game.dims();

        let observations = initial_observations.unwrap_or_else(|| ObsTracker::new(dims));
        Ok((
            Self {
                game,
                secret,
                observations,
                end_turn_on_drop,
            },
            turn_start,
        ))
    }

    pub async fn turn_is_done(&self, player: PlayerNum, turn: TurnNum) -> UmpireResult<bool> {
        self.game.turn_is_done(player, turn).await
    }

    pub async fn current_turn_is_done(&self) -> bool {
        self.game.current_turn_is_done().await
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    pub async fn victor(&self) -> Option<PlayerNum> {
        self.game.victor().await
    }

    pub async fn player_unit_legal_directions(
        &self,
        unit_id: UnitID,
    ) -> UmpireResult<Vec<Direction>> {
        self.game
            .player_unit_legal_directions(self.secret, unit_id)
            .await
    }

    /// The tile at the given location, as present in the player's observations (or not)
    pub async fn tile(&self, loc: Location) -> Option<Cow<Tile>> {
        match self.observations.get(loc) {
            Obs::Observed { tile, .. } => Some(Cow::Borrowed(tile)),
            Obs::Unobserved => None,
        }
    }

    pub async fn obs(&self, loc: Location) -> Obs {
        self.observations.get(loc).clone()
    }

    pub async fn player_cities_producing_or_not_ignored(&self) -> usize {
        self.game
            .player_cities_producing_or_not_ignored(self.secret)
            .await
            .unwrap()
    }

    /// The city at `loc` if controlled by this player
    pub async fn player_city_by_loc(&self, loc: Location) -> Option<City> {
        self.game
            .player_city_by_loc(self.secret, loc)
            .await
            .unwrap()
    }

    pub async fn player_unit_by_id(&self, id: UnitID) -> Option<Unit> {
        self.game.player_unit_by_id(self.secret, id).await.unwrap()
    }

    pub async fn player_unit_loc(&self, id: UnitID) -> Option<Location> {
        self.game.player_unit_loc(self.secret, id).await.unwrap()
    }

    pub async fn player_toplevel_unit_by_loc(&self, loc: Location) -> Option<Unit> {
        self.game
            .player_toplevel_unit_by_loc(self.secret, loc)
            .await
            .unwrap()
    }

    pub async fn production_set_requests(&self) -> Vec<Location> {
        self.game
            .player_production_set_requests(self.secret)
            .await
            .unwrap()
    }

    pub async fn player_unit_orders_requests(&self) -> Vec<UnitID> {
        self.game
            .player_unit_orders_requests(self.secret)
            .await
            .unwrap()
    }

    // Movement-related methods

    pub async fn propose_move_unit_by_id(
        &self,
        id: UnitID,
        dest: Location,
    ) -> ProposedUmpireResult<Move> {
        self.game
            .propose_move_unit_by_id(self.secret, id, dest)
            .await
    }

    pub async fn move_unit_by_id_in_direction(
        &mut self,
        id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move> {
        self.game
            .move_unit_by_id_in_direction(self.secret, id, direction)
            .await
            .map(|move_| {
                self.observations.track_many(move_.observations());
                move_
            })
    }

    /// TODO Update observations
    pub async fn disband_unit_by_id(&mut self, id: UnitID) -> UmpireResult<Unit> {
        self.game.disband_unit_by_id(self.secret, id).await
    }

    /// Sets the production of the current player's city at location `loc` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    ///
    /// TODO Update observations
    pub async fn set_production_by_loc(
        &mut self,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        self.game
            .set_production_by_loc(self.secret, loc, production)
            .await
    }

    /// TODO Update observations
    pub async fn clear_production(
        &mut self,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<Option<UnitType>> {
        self.game
            .clear_production(self.secret, loc, ignore_cleared_production)
            .await
    }

    pub async fn turn(&self) -> TurnNum {
        self.game.turn().await
    }

    pub async fn current_player(&self) -> PlayerNum {
        self.game.current_player().await
    }

    /// The logical dimensions of the game map
    pub async fn dims(&self) -> Dims {
        self.game.dims().await
    }

    pub async fn wrapping(&self) -> Wrap2d {
        self.game.wrapping().await
    }

    /// Units that could be produced by a city located at the given location
    pub async fn valid_productions(&self, loc: Location) -> Vec<UnitType> {
        self.game.valid_productions(self.secret, loc).await.unwrap()
    }

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    pub async fn valid_productions_conservative(&self, loc: Location) -> Vec<UnitType> {
        self.game
            .valid_productions_conservative(self.secret, loc)
            .await
            .unwrap()
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    ///
    /// TODO Update observations
    pub async fn order_unit_sentry(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_sentry(self.secret, unit_id).await
    }

    /// TODO Update observations
    pub async fn order_unit_skip(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_skip(self.secret, unit_id).await
    }

    /// Simulate ordering the specified unit to go to the given location
    pub async fn propose_order_unit_go_to(
        &self,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult {
        self.game
            .propose_order_unit_go_to(self.secret, unit_id, dest)
            .await
    }

    /// Simulate ordering the specified unit to explore.
    pub async fn propose_order_unit_explore(&self, unit_id: UnitID) -> ProposedOrdersResult {
        self.game
            .propose_order_unit_explore(self.secret, unit_id)
            .await
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    ///
    /// TODO Update observations
    pub async fn activate_unit_by_loc(&mut self, loc: Location) -> UmpireResult<()> {
        self.game.activate_unit_by_loc(self.secret, loc).await
    }

    /// TODO Update observations
    pub async fn begin_turn(&mut self) -> UmpireResult<TurnStart> {
        self.game.begin_turn(self.secret).await
    }

    /// TODO Update observations
    pub async fn end_turn(&mut self) -> UmpireResult<()> {
        self.game.end_turn(self.secret).await
    }

    pub async fn player_score(&self) -> UmpireResult<f64> {
        self.game.player_score(self.secret).await
    }

    /// TODO Update observations
    pub async fn take_simple_action(
        &mut self,
        action: AiPlayerAction,
    ) -> Result<PlayerActionOutcome, GameError> {
        self.game.take_simple_action(self.secret, action).await
    }

    /// FIXME Implement ML feature vector generation
    /// Possibly split unit-relevant from city-relevant features
    /// FIXME Maintain this vector in the client, incrementally
    pub async fn player_features(&self) -> Vec<fX> {
        // player_features(self.game, self.secret).unwrap()
        Vec::new()
    }

    /// TODO Update observations
    pub async fn take_action(
        &mut self,
        action: PlayerAction,
    ) -> Result<PlayerActionOutcome, GameError> {
        self.game.take_action(self.secret, action).await
    }
}

impl<'a> fmt::Debug for PlayerTurnControl<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlayerTurnControl")
    }
}

/// If for whatever reason a careless user fails to end the turn, we do it for them so the game continues to advance.
///
/// This forces the turn to end regardless of the state of production and orders requests.
impl<'a> Drop for PlayerTurnControl<'a> {
    fn drop(&mut self) {
        if self.end_turn_on_drop {
            // let rt = Handle::current();

            // Because `Drop` is synchronous, we grab a reference to the Tokio runtime and block on
            // the async calls we need to end the user's turn
            futures::executor::block_on(async {
                if self.game.is_player_turn(self.secret).await.unwrap() {
                    self.game.force_end_turn(self.secret).await.unwrap();
                }
            });
        }
    }
}

/// What's the meta-outcome of a TurnTaker taking a turn?
pub struct TurnOutcome {
    /// Training data generated during the turn, for ML purposes
    pub training_instances: Option<Vec<TrainingInstance>>,

    /// Indicate if the player quit the app
    pub quit: bool,
}

/// Take a turn with only the knowledge of game state an individual player should have
/// This is the main thing to use
///
/// # Arguments
/// * generate_data: whether or not training data for a state-action-value model should be returned
#[async_trait]
pub trait LimitedTurnTaker {
    async fn take_turn(&mut self, ctrl: &mut PlayerTurnControl, generate_data: bool)
        -> TurnOutcome;
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
        game: &mut dyn IGame,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> TurnOutcome;

    async fn take_turn_clearing(
        &mut self,
        game: &mut dyn IGame,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> TurnOutcome;

    async fn take_turn(
        &mut self,
        game: &mut dyn IGame,
        player: PlayerNum,
        secret: PlayerSecret,
        clear_at_end_of_turn: bool,
        generate_data: bool,
    ) -> TurnOutcome {
        if clear_at_end_of_turn {
            self.take_turn_clearing(game, player, secret, generate_data)
                .await
        } else {
            self.take_turn_not_clearing(game, player, secret, generate_data)
                .await
        }
    }
}

#[async_trait]
impl<T: LimitedTurnTaker + Send> TurnTaker for T {
    async fn take_turn_not_clearing(
        &mut self,
        game: &mut dyn IGame,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> TurnOutcome {
        let turn = game.turn().await;

        let (mut ctrl, _turn_start) = PlayerTurnControl::new(game, secret).await.unwrap();

        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let mut quit = false;

        loop {
            let result =
                <Self as LimitedTurnTaker>::take_turn(self, &mut ctrl, generate_data).await;
            if let Some(mut instances) = result.training_instances {
                training_instances
                    .as_mut()
                    .map(|v| v.append(&mut instances));
            }

            if result.quit {
                quit = true;
                break;
            }

            if ctrl.turn_is_done(player, turn).await.unwrap() {
                break;
            }
        }

        TurnOutcome {
            training_instances,
            quit,
        }
    }

    async fn take_turn_clearing(
        &mut self,
        game: &mut dyn IGame,
        player: PlayerNum,
        secret: PlayerSecret,
        generate_data: bool,
    ) -> TurnOutcome {
        let turn = game.turn().await;
        let (mut ctrl, _turn_start) = PlayerTurnControl::new_clearing(game, secret).await.unwrap();

        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let mut quit = false;

        loop {
            let result =
                <Self as LimitedTurnTaker>::take_turn(self, &mut ctrl, generate_data).await;
            if let Some(mut instances) = result.training_instances {
                training_instances
                    .as_mut()
                    .map(|v| v.append(&mut instances));
            }

            if result.quit {
                quit = true;
                break;
            }

            if ctrl.turn_is_done(player, turn).await.unwrap() {
                break;
            }
        }

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
    async fn next_action(&mut self, ctrl: &PlayerTurnControl<'_>) -> Option<AiPlayerAction>;
}

#[async_trait]
impl<T: ActionwiseLimitedTurnTaker + Send + Sync> LimitedTurnTaker for T {
    async fn take_turn(
        &mut self,
        ctrl: &mut PlayerTurnControl,
        generate_data: bool,
    ) -> TurnOutcome {
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

            if ctrl.turn_is_done(player, turn).await.unwrap() {
                break;
            }
        }

        TurnOutcome {
            training_instances,
            quit: false, // Only robots are using this trait and they never quit the game
        }
    }
}

trait ActionwiseTurnTaker {
    fn next_action(&self, game: &Game, generate_data: bool) -> Option<TrainingInstance>;
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
    game: Rc<RefCell<dyn IGame>>,
    player: PlayerNum,
    secret: PlayerSecret,
    observations: ObsTracker,
}

impl PlayerControl {
    pub async fn new(
        game: Rc<RefCell<dyn IGame>>,
        player: PlayerNum,
        secret: PlayerSecret,
    ) -> Self {
        let dims = game.borrow().dims().await;
        Self::from_observations(game, player, secret, ObsTracker::new(dims))
    }

    pub fn from_observations(
        game: Rc<RefCell<dyn IGame>>,
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
        to self.game.borrow_mut() {
            /// TODO Update observations
            pub async fn activate_unit_by_loc(&mut self, [self.secret], loc: Location) -> UmpireResult<()>;

            /// TODO Update observations
            pub async fn begin_turn(&mut self, [self.secret]) -> UmpireResult<TurnStart>;

            /// TODO Update observations
            pub async fn clear_production(&mut self, [self.secret], loc: Location, ignore_cleared_production: bool) -> UmpireResult<Option<UnitType>>;

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

        to self.game.borrow() {
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
            pub async fn player_cities_producing_or_not_ignored(&self, [self.secret]) -> usize;

            #[unwrap]
            pub async fn player_city_by_loc(&self, [self.secret], loc: Location) -> Option<City>;

            #[unwrap]
            pub async fn player_production_set_requests(&self, [self.secret]) -> Vec<Location>;

            pub async fn player_score(&self, [self.secret]) -> UmpireResult<f64>;

            #[unwrap]
            pub async fn player_toplevel_unit_by_loc(&self, [self.secret], loc: Location) -> Option<Unit>;

            #[unwrap]
            pub async fn player_unit_by_id(&self, [self.secret], id: UnitID) -> Option<Unit>;

            pub async fn player_unit_legal_directions(&self, [self.secret], unit_id: UnitID) -> UmpireResult<Vec<Direction>>;

            #[unwrap]
            pub async fn player_unit_orders_requests(&self, [self.secret]) -> Vec<UnitID>;

            #[unwrap]
            pub async fn player_unit_loc(&self, [self.secret], id: UnitID) -> Option<Location>;

            pub async fn turn(&self) -> TurnNum;

            pub async fn turn_is_done(&self, [self.player], turn: TurnNum) -> UmpireResult<bool>;
            pub async fn current_turn_is_done(&self) -> bool;

            #[unwrap]
            pub async fn valid_productions(&self, [self.secret], loc: Location) -> Vec<UnitType>;

            #[unwrap]
            pub async fn valid_productions_conservative(&self, [self.secret], loc: Location) -> Vec<UnitType>;

            pub async fn victor(&self) -> Option<PlayerNum>;

            pub async fn wrapping(&self) -> Wrap2d;
        }
    }

    pub async fn move_unit_by_id_in_direction(
        &mut self,
        id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move> {
        self.game
            .borrow_mut()
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

            pub async fn begin_turn(&mut self) -> UmpireResult<TurnStart>;

            pub async fn clear_production(&mut self, loc: Location, ignore_cleared_production: bool) -> UmpireResult<Option<UnitType>>;

            pub async fn disband_unit_by_id(&mut self, id: UnitID) -> UmpireResult<Unit>;

            pub async fn end_turn(&mut self) -> UmpireResult<()>;

            pub async fn force_end_turn(&mut self) -> UmpireResult<()>;

            pub async fn move_unit_by_id_in_direction(&mut self, id: UnitID, direction: Direction) -> UmpireResult<Move>;

            pub async fn order_unit_sentry(&mut self, unit_id: UnitID) -> OrdersResult;

            pub async fn order_unit_skip(&mut self,  unit_id: UnitID) -> OrdersResult;

            pub async fn set_production_by_loc(&mut self, loc: Location, production: UnitType) -> UmpireResult<Option<UnitType>>;

            pub async fn take_action(&mut self, action: PlayerAction) -> UmpireResult<PlayerActionOutcome>;

            pub async fn take_simple_action(&mut self, action: AiPlayerAction) -> UmpireResult<PlayerActionOutcome>;

            // Immutable
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
