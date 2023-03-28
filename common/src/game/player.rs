use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{
    action::{AiPlayerAction, PlayerAction, PlayerActionOutcome},
    ai::{fX, player_features, AISpec},
    move_::Move,
    obs::PlayerObsTracker,
    PlayerSecret, ProposedOrdersResult, ProposedUmpireResult, TurnStart, UmpireResult,
};
use crate::{
    cli::Specified,
    game::{
        ai::TrainingInstance,
        city::City,
        map::tile::Tile,
        obs::{Obs, ObsTracker},
        unit::{orders::OrdersResult, Unit, UnitID, UnitType},
        Game, GameError, TurnNum,
    },
    util::{sparsify, Dims, Direction, Location, Wrap2d},
};

pub type PlayerNum = usize;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

pub struct PlayerTurnControl<'a> {
    game: &'a mut Game,

    secret: PlayerSecret,

    observations: PlayerObsTracker,

    end_turn_on_drop: bool,
}
impl<'a> PlayerTurnControl<'a> {
    pub fn new(
        game: &'a mut Game,
        secret: PlayerSecret,
        end_turn_on_drop: bool,
        clearing: bool,
    ) -> UmpireResult<(Self, TurnStart)> {
        game.validate_is_player_turn(secret)?;

        let turn_start = if clearing {
            game.begin_turn_clearing(secret)
        } else {
            game.begin_turn(secret)
        }?;

        let observations = PlayerObsTracker::new(game.num_players(), game.dims());
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
        self.game.turn_is_done(player, turn)
    }

    pub async fn current_turn_is_done(&self) -> bool {
        self.game.current_turn_is_done()
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    pub async fn victor(&self) -> Option<PlayerNum> {
        self.game.victor()
    }

    pub async fn player_unit_legal_directions<'b>(
        &'b self,
        unit_id: UnitID,
    ) -> UmpireResult<impl Iterator<Item = Direction> + 'b> {
        self.game.player_unit_legal_directions(self.secret, unit_id)
    }

    /// The tile at the given location, as present in the player's observations (or not)
    pub async fn tile(&self, loc: Location) -> Option<&Tile> {
        self.game.player_tile(self.secret, loc).unwrap()
    }

    pub async fn obs(&self, loc: Location) -> &Obs {
        self.game.player_obs(self.secret, loc).unwrap()
    }

    pub async fn player_cities_producing_or_not_ignored(&self) -> usize {
        self.game
            .player_cities_producing_or_not_ignored(self.secret)
            .unwrap()
    }

    /// The city at `loc` if controlled by this player
    pub async fn player_city_by_loc(&self, loc: Location) -> Option<&City> {
        self.game.player_city_by_loc(self.secret, loc).unwrap()
    }

    pub async fn player_unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.game.player_unit_by_id(self.secret, id).unwrap()
    }

    pub async fn player_unit_loc(&self, id: UnitID) -> Option<Location> {
        self.game.player_unit_loc(self.secret, id).unwrap()
    }

    pub async fn player_toplevel_unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        self.game
            .player_toplevel_unit_by_loc(self.secret, loc)
            .unwrap()
    }

    pub async fn production_set_requests(&'a self) -> impl Iterator<Item = Location> + 'a {
        self.game
            .player_production_set_requests(self.secret)
            .unwrap()
    }

    pub async fn player_unit_orders_requests(&'a self) -> impl Iterator<Item = UnitID> + 'a {
        self.game.player_unit_orders_requests(self.secret).unwrap()
    }

    // Movement-related methods

    pub async fn propose_move_unit_by_id(
        &self,
        id: UnitID,
        dest: Location,
    ) -> ProposedUmpireResult<Move> {
        self.game.propose_move_unit_by_id(self.secret, id, dest)
    }

    pub fn disband_unit_by_id(&mut self, id: UnitID) -> UmpireResult<Unit> {
        self.game.disband_unit_by_id(self.secret, id)
    }

    /// Sets the production of the current player's city at location `loc` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    pub fn set_production_by_loc(
        &mut self,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        self.game
            .set_production_by_loc(self.secret, loc, production)
    }

    pub fn clear_production(
        &mut self,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<Option<UnitType>> {
        self.game
            .clear_production(self.secret, loc, ignore_cleared_production)
    }

    pub fn turn(&self) -> TurnNum {
        self.game.turn()
    }

    pub fn current_player(&self) -> PlayerNum {
        self.game.current_player()
    }

    /// The logical dimensions of the game map
    pub fn dims(&self) -> Dims {
        self.game.dims()
    }

    pub fn wrapping(&self) -> Wrap2d {
        self.game.wrapping()
    }

    /// Units that could be produced by a city located at the given location
    pub fn valid_productions(&'a self, loc: Location) -> impl Iterator<Item = UnitType> + 'a {
        self.game.valid_productions(self.secret, loc).unwrap()
    }

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    pub fn valid_productions_conservative<'b>(
        &'b self,
        loc: Location,
    ) -> impl Iterator<Item = UnitType> + 'b {
        self.game
            .valid_productions_conservative(self.secret, loc)
            .unwrap()
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    pub fn order_unit_sentry(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_sentry(self.secret, unit_id)
    }

    pub fn order_unit_skip(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_skip(self.secret, unit_id)
    }

    /// Simulate ordering the specified unit to go to the given location
    pub fn propose_order_unit_go_to(
        &self,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult {
        self.game
            .propose_order_unit_go_to(self.secret, unit_id, dest)
    }

    /// Simulate ordering the specified unit to explore.
    pub fn propose_order_unit_explore(&self, unit_id: UnitID) -> ProposedOrdersResult {
        self.game.propose_order_unit_explore(self.secret, unit_id)
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    pub fn activate_unit_by_loc(&mut self, loc: Location) -> UmpireResult<()> {
        self.game.activate_unit_by_loc(self.secret, loc)
    }

    pub fn begin_turn(&mut self) -> UmpireResult<TurnStart> {
        self.game.begin_turn(self.secret)
    }

    pub fn end_turn(&mut self) -> UmpireResult<()> {
        self.game.end_turn(self.secret)
    }

    fn player_score(&self) -> UmpireResult<f64> {
        self.game.player_score(self.secret)
    }

    fn take_simple_action(
        &mut self,
        action: AiPlayerAction,
    ) -> Result<PlayerActionOutcome, GameError> {
        self.game.take_simple_action(self.secret, action)
    }

    /// FIXME Maintain this vector in the client, incrementally
    pub fn player_features(&self) -> Vec<fX> {
        player_features(self.game, self.secret).unwrap()
    }

    pub fn take_action(&mut self, action: PlayerAction) -> Result<PlayerActionOutcome, GameError> {
        self.game.take_action(self.secret, action)
    }
}

/// If for whatever reason a careless user fails to end the turn, we do it for them so the game continues to advance.
///
/// This forces the turn to end regardless of the state of production and orders requests.
impl<'a> Drop for PlayerTurnControl<'a> {
    fn drop(&mut self) {
        if self.end_turn_on_drop && self.game.is_player_turn(self.secret).unwrap() {
            self.game.force_end_turn(self.secret).unwrap();
        }
    }
}

/// Take a turn with only the knowledge of game state an individual player should have
/// This is the main thing to use
///
/// # Arguments
/// * generate_data: whether or not training data for a state-action-value model should be returned
#[async_trait]
pub trait LimitedTurnTaker {
    async fn take_turn(
        &mut self,
        ctrl: &mut PlayerTurnControl,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>>;
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
        game: &mut Game,
        player_secrets: &Vec<PlayerSecret>,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>>;

    async fn take_turn_clearing(
        &mut self,
        game: &mut Game,
        player_secrets: &Vec<PlayerSecret>,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>>;

    async fn take_turn(
        &mut self,
        game: &mut Game,
        player_secrets: &Vec<PlayerSecret>,
        clear_at_end_of_turn: bool,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        if clear_at_end_of_turn {
            self.take_turn_clearing(game, player_secrets, generate_data)
                .await
        } else {
            self.take_turn_not_clearing(game, player_secrets, generate_data)
                .await
        }
    }
}

#[async_trait]
impl<T: LimitedTurnTaker + Send> TurnTaker for T {
    async fn take_turn_not_clearing(
        &mut self,
        game: &mut Game,
        player_secrets: &Vec<PlayerSecret>,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        let player = game.current_player();
        let player_secret = player_secrets[player];
        let turn = game.turn;
        let (mut ctrl, _turn_start) = game.player_turn_control(player_secret).unwrap();
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        loop {
            let result =
                <Self as LimitedTurnTaker>::take_turn(self, &mut ctrl, generate_data).await;
            if let Some(mut instances) = result {
                training_instances
                    .as_mut()
                    .map(|v| v.append(&mut instances));
            }

            if ctrl.turn_is_done(player, turn).await.unwrap() {
                break;
            }
        }

        training_instances
    }

    async fn take_turn_clearing(
        &mut self,
        game: &mut Game,
        player_secrets: &Vec<PlayerSecret>,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        let player = game.current_player();
        let player_secret = player_secrets[player];
        let turn = game.turn;
        let (mut ctrl, _turn_start) = game.player_turn_control_clearing(player_secret).unwrap();
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        loop {
            let result =
                <Self as LimitedTurnTaker>::take_turn(self, &mut ctrl, generate_data).await;
            if let Some(mut instances) = result {
                training_instances
                    .as_mut()
                    .map(|v| v.append(&mut instances));
            }

            if ctrl.turn_is_done(player, turn).await.unwrap() {
                break;
            }
        }

        training_instances
    }
}

#[async_trait]
pub trait ActionwiseLimitedTurnTaker {
    /// The next action that should be taken
    ///
    /// Return None if there are no actions that should be taken
    async fn next_action(&mut self, ctrl: &PlayerTurnControl) -> Option<AiPlayerAction>;
}

#[async_trait]
impl<T: ActionwiseLimitedTurnTaker + Send + Sync> LimitedTurnTaker for T {
    async fn take_turn(
        &mut self,
        ctrl: &mut PlayerTurnControl,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        let player = ctrl.current_player();
        let turn = ctrl.turn();

        loop {
            let (num_features, features, pre_score) = if generate_data {
                let (num_features, features) = sparsify(ctrl.player_features());
                (
                    Some(num_features),
                    Some(features),
                    Some(ctrl.player_score().unwrap()),
                )
            } else {
                (None, None, None)
            };

            if let Some(action) = self.next_action(ctrl).await {
                // If an action was specified...

                ctrl.take_simple_action(action).unwrap();

                if generate_data {
                    let post_score = ctrl.player_score().unwrap();
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

        training_instances
    }
}

trait ActionwiseTurnTaker {
    fn next_action(&self, game: &Game, generate_data: bool) -> Option<TrainingInstance>;
}

/**
 * The game information available to a particular player
 */
#[derive(Deserialize, Serialize)]
pub struct PlayerGameView {
    pub observations: ObsTracker,

    pub turn: TurnNum,

    pub num_players: PlayerNum,

    pub current_player: PlayerNum,

    pub wrapping: Wrap2d,

    pub fog_of_war: bool,

    pub score: f64,
}
