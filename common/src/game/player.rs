use serde::{Deserialize, Serialize};

use super::{
    action::{AiPlayerAction, PlayerActionOutcome},
    ai::{fX, player_features, AISpec},
};
use crate::{
    cli::Specified,
    game::{
        ai::TrainingInstance,
        city::City,
        map::tile::Tile,
        move_::{Move, MoveError},
        obs::{Obs, ObsTracker},
        unit::{orders::OrdersResult, Unit, UnitID, UnitType},
        Game, GameError, Proposed, TurnNum, TurnStart,
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

    /// Which player is this control shim representing? A copy of `Game::current_player`'s result. Shouldn't get stale
    /// since we lock down anything that would change who the current player is. We do this for convenience.
    pub player: PlayerNum,

    clear_completed_productions: bool,

    end_turn_on_drop: bool,
}
impl<'a> PlayerTurnControl<'a> {
    pub fn new(
        game: &'a mut Game,
        end_turn_on_drop: bool,
        clear_completed_productions: bool,
    ) -> Self {
        let player = game.current_player();
        Self {
            game,
            player,
            clear_completed_productions,
            end_turn_on_drop,
        }
    }

    pub fn new_clearing(game: &'a mut Game) -> Self {
        let player = game.current_player();
        Self {
            game,
            player,
            clear_completed_productions: true,
            end_turn_on_drop: true,
        }
    }

    pub fn turn_is_done(&self) -> bool {
        self.game.turn_is_done()
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    pub fn victor(&self) -> Option<PlayerNum> {
        self.game.victor()
    }

    pub fn current_player_unit_legal_directions<'b>(
        &'b self,
        unit_id: UnitID,
    ) -> Result<impl Iterator<Item = Direction> + 'b, GameError> {
        self.game.current_player_unit_legal_directions(unit_id)
    }

    /// The current player's most recent observation of the tile at location `loc`, if any
    pub fn current_player_tile(&self, loc: Location) -> Option<&Tile> {
        self.game.current_player_tile(loc)
    }

    /// The current player's observation at location `loc`
    pub fn current_player_obs(&self, loc: Location) -> &Obs {
        self.game.current_player_obs(loc)
    }

    /// The number of cities controlled by the current player which either have a production target or are NOT set to be ignored when requesting productions to be set
    ///
    /// This basically lets us make sure a player doesn't set all their cities' productions to none since right now the UI has no way of getting out of that situation
    ///
    /// FIXME Get rid of this and just make the UI smarter
    #[deprecated]
    pub fn player_cities_producing_or_not_ignored(&self) -> usize {
        self.game.player_cities_producing_or_not_ignored()
    }

    /// If the current player controls a city at location `loc`, return it
    pub fn current_player_city_by_loc(&self, loc: Location) -> Option<&City> {
        self.game.current_player_city_by_loc(loc)
    }

    /// If the current player controls a unit with ID `id`, return it
    pub fn current_player_unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.game.current_player_unit_by_id(id)
    }

    /// If the current player controls a unit with ID `id`, return its location
    pub fn current_player_unit_loc(&self, id: UnitID) -> Option<Location> {
        self.game.current_player_unit_loc(id)
    }

    /// If the current player controls the top-level unit at location `loc`, return it
    pub fn current_player_toplevel_unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        self.game.current_player_toplevel_unit_by_loc(loc)
    }

    pub fn production_set_requests(&'a self) -> impl Iterator<Item = Location> + 'a {
        self.game.production_set_requests()
    }

    /// Which if the current player's units need orders?
    ///
    /// In other words, which of the current player's units have no orders and have moves remaining?
    pub fn unit_orders_requests(&'a self) -> impl Iterator<Item = UnitID> + 'a {
        self.game.unit_orders_requests()
    }

    // Movement-related methods

    pub fn propose_move_unit_by_id(
        &self,
        id: UnitID,
        dest: Location,
    ) -> Proposed<Result<Move, MoveError>> {
        self.game.propose_move_unit_by_id(id, dest)
    }

    pub fn disband_unit_by_id(&mut self, id: UnitID) -> Result<Unit, GameError> {
        self.game.disband_unit_by_id(id)
    }

    /// Sets the production of the current player's city at location `loc` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    pub fn set_production_by_loc(
        &mut self,
        loc: Location,
        production: UnitType,
    ) -> Result<Option<UnitType>, GameError> {
        self.game.set_production_by_loc(loc, production)
    }

    pub fn clear_production(
        &mut self,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> Result<Option<UnitType>, GameError> {
        self.game.clear_production(loc, ignore_cleared_production)
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
        self.game.valid_productions(loc)
    }

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    pub fn valid_productions_conservative<'b>(
        &'b self,
        loc: Location,
    ) -> impl Iterator<Item = UnitType> + 'b {
        self.game.valid_productions_conservative(loc)
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    pub fn order_unit_sentry(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_sentry(unit_id)
    }

    pub fn order_unit_skip(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_skip(unit_id)
    }

    /// Simulate ordering the specified unit to go to the given location
    pub fn propose_order_unit_go_to(
        &self,
        unit_id: UnitID,
        dest: Location,
    ) -> Proposed<OrdersResult> {
        self.game.propose_order_unit_go_to(unit_id, dest)
    }

    /// Simulate ordering the specified unit to explore.
    pub fn propose_order_unit_explore(&self, unit_id: UnitID) -> Proposed<OrdersResult> {
        self.game.propose_order_unit_explore(unit_id)
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    pub fn activate_unit_by_loc(&mut self, loc: Location) -> Result<(), GameError> {
        self.game.activate_unit_by_loc(loc)
    }

    pub fn propose_end_turn(&self) -> (Game, Result<TurnStart, PlayerNum>) {
        let mut game = self.game.clone();
        let result = game.end_turn();
        (game, result)
    }

    pub fn apply_proposal<T>(&mut self, proposal: Proposed<T>) -> T {
        proposal.apply(self.game)
    }

    fn player_score(&self, player: PlayerNum) -> Result<f64, GameError> {
        self.game.player_score(player)
    }

    fn take_simple_action(
        &mut self,
        action: AiPlayerAction,
    ) -> Result<PlayerActionOutcome, GameError> {
        self.game.take_simple_action(action)
    }

    pub fn player_features(&self) -> Vec<fX> {
        player_features(self.game, self.current_player())
    }
}

/// If for whatever reason a careless user fails to end the turn, we do it for them so the game continues to advance.
///
/// This forces the turn to end regardless of the state of production and orders requests.
impl<'a> Drop for PlayerTurnControl<'a> {
    fn drop(&mut self) {
        if self.end_turn_on_drop {
            if self.clear_completed_productions {
                self.game.force_end_turn_clearing();
            } else {
                self.game.force_end_turn();
            }
        }
    }
}

/// Take a turn with only the knowledge of game state an individual player should have
/// This is the main thing to use
///
/// # Arguments
/// * generate_data: whether or not training data for a state-action-value model should be returned
pub trait LimitedTurnTaker {
    fn take_turn(
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
pub trait TurnTaker {
    fn take_turn_not_clearing(
        &mut self,
        game: &mut Game,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>>;

    fn take_turn_clearing(
        &mut self,
        game: &mut Game,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>>;

    fn take_turn(
        &mut self,
        game: &mut Game,
        clear_at_end_of_turn: bool,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        if clear_at_end_of_turn {
            self.take_turn_clearing(game, generate_data)
        } else {
            self.take_turn_not_clearing(game, generate_data)
        }
    }
}

impl<T: LimitedTurnTaker> TurnTaker for T {
    fn take_turn_not_clearing(
        &mut self,
        game: &mut Game,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        let mut ctrl = game.player_turn_control(game.current_player());
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        loop {
            let result = <Self as LimitedTurnTaker>::take_turn(self, &mut ctrl, generate_data);
            if let Some(mut instances) = result {
                training_instances
                    .as_mut()
                    .map(|v| v.append(&mut instances));
            }

            if ctrl.turn_is_done() {
                break;
            }
        }

        training_instances
    }

    fn take_turn_clearing(
        &mut self,
        game: &mut Game,
        generate_data: bool,
    ) -> Option<Vec<TrainingInstance>> {
        let mut ctrl = game.player_turn_control_clearing(game.current_player());
        let mut training_instances = if generate_data {
            Some(Vec::new())
        } else {
            None
        };

        loop {
            let result = <Self as LimitedTurnTaker>::take_turn(self, &mut ctrl, generate_data);
            if let Some(mut instances) = result {
                training_instances
                    .as_mut()
                    .map(|v| v.append(&mut instances));
            }

            if ctrl.turn_is_done() {
                break;
            }
        }

        training_instances
    }
}

pub trait ActionwiseLimitedTurnTaker {
    /// The next action that should be taken
    ///
    /// Return None if there are no actions that should be taken
    fn next_action(&self, ctrl: &PlayerTurnControl) -> Option<AiPlayerAction>;
}

impl<T: ActionwiseLimitedTurnTaker> LimitedTurnTaker for T {
    fn take_turn(
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

        loop {
            let (num_features, features, pre_score) = if generate_data {
                let (num_features, features) = sparsify(ctrl.player_features());
                (
                    Some(num_features),
                    Some(features),
                    Some(ctrl.player_score(player).unwrap()),
                )
            } else {
                (None, None, None)
            };

            if let Some(action) = self.next_action(ctrl) {
                // If an action was specified...

                ctrl.take_simple_action(action).unwrap();

                if generate_data {
                    let post_score = ctrl.player_score(player).unwrap();
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

            if ctrl.turn_is_done() {
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
