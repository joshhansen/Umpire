use std::{borrow::Cow, sync::Arc};

use delegate::delegate;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as RwLockTokio;

use super::{
    action::{AiPlayerAction, PlayerAction, PlayerActionOutcome},
    ai::{fX, AISpec},
    error::GameError,
    map::dijkstra::Source,
    move_::Move,
    obs::{LocatedObsLite, ObsTracker},
    IGame, PlayerSecret, ProposedOrdersResult, ProposedUmpireResult, TurnPhase, TurnStart,
    UmpireResult,
};
use crate::{
    cli::Specified,
    game::{
        city::City,
        map::tile::Tile,
        obs::Obs,
        unit::{orders::OrdersResult, Unit, UnitID, UnitType},
        Game, TurnNum,
    },
    util::{Dims, Direction, Location, Wrap2d},
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
    /// Initialize a new PlayerControl, using the observations in the game instance as a starting point.
    pub async fn new(
        game: Arc<RwLockTokio<dyn IGame>>,
        player: PlayerNum,
        secret: PlayerSecret,
    ) -> Self {
        let observations = game.read().await.player_observations(secret).await.unwrap();
        Self::from_observations(game, player, secret, observations)
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

    pub async fn activate_unit_by_loc(&mut self, loc: Location) -> UmpireResult<LocatedObsLite> {
        let result = self
            .game
            .write()
            .await
            .activate_unit_by_loc(self.secret, loc)
            .await?;

        self.observations.track_lite(result.clone());

        Ok(result)
    }

    pub async fn begin_turn(&mut self) -> UmpireResult<TurnStart> {
        let result = self.game.write().await.begin_turn(self.secret).await;

        if let Ok(ref turn_start) = result {
            self.observations.track_many(turn_start.observations.iter());
        }

        result
    }

    delegate! {
        to self.game.write().await {
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

    pub async fn turn_ctrl(&mut self) -> PlayerTurn {
        PlayerTurn::new(self).await
    }
}

/// Guard that begins a turn, then ends it on drop
pub struct PlayerTurn<'a> {
    ctrl: &'a mut PlayerControl,

    turn_start: TurnStart,

    ended: bool,
}

impl<'a> PlayerTurn<'a> {
    pub async fn new(ctrl: &'a mut PlayerControl) -> PlayerTurn<'a> {
        let turn_start = ctrl.begin_turn().await.unwrap();
        Self {
            ctrl,
            turn_start,
            ended: false,
        }
    }

    pub fn start(&self) -> &TurnStart {
        &self.turn_start
    }

    pub async fn end_turn(&mut self) -> UmpireResult<()> {
        if self.ended {
            return Err(GameError::NotPlayersTurn {
                player: self.ctrl.player,
            });
        }

        let result = self.ctrl.end_turn().await;

        if result.is_ok() {
            self.ended = true;
        }

        result
    }

    pub async fn force_end_turn(&mut self) -> UmpireResult<()> {
        if self.ended {
            return Err(GameError::NotPlayersTurn {
                player: self.ctrl.player,
            });
        }

        let result = self.ctrl.force_end_turn().await;

        if result.is_ok() {
            self.ended = true;
        }

        result
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    delegate! {
        to self.ctrl {
            // Mutable
            pub async fn activate_unit_by_loc(&mut self, loc: Location) -> UmpireResult<LocatedObsLite>;

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

/// On drop, we check that the turn was already ended.
///
/// Turn ending has to happen asynchronously, so we can't do it on drop.
/// Instead, we make sure it was done, and panic otherwise.
impl<'a> Drop for PlayerTurn<'a> {
    fn drop(&mut self) {
        if !self.ended {
            panic!("PlayerTurn wasn't ended at time of drop");
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::game::{player::PlayerControl, test_support::game1, IGame};

    use tokio::sync::RwLock as RwLockTokio;

    #[tokio::test]
    pub async fn test_player_turn() {
        let (game, secrets) = game1();

        assert_eq!(game.current_player(), 0);
        assert_eq!(game.turn(), 0);

        let game = Arc::new(RwLockTokio::new(game));

        let mut ctrls: Vec<PlayerControl> = Vec::with_capacity(2);
        for player in 0..2 {
            ctrls.push(
                PlayerControl::new(
                    Arc::clone(&game) as Arc<RwLockTokio<dyn IGame>>,
                    player,
                    secrets[player],
                )
                .await,
            );
        }

        {
            let ctrl = &mut ctrls[0];

            let mut turn = ctrl.turn_ctrl().await;

            assert_eq!(turn.turn().await, 0);
            assert_eq!(turn.current_player().await, 0);

            assert_eq!(turn.start().orders_results.len(), 0);
            assert_eq!(turn.start().production_outcomes.len(), 0);

            turn.force_end_turn().await.unwrap();
        }

        assert_eq!(game.read().await.turn(), 0);
        assert_eq!(game.read().await.current_player(), 1);

        {
            let ctrl = &mut ctrls[1];

            let mut turn = ctrl.turn_ctrl().await;

            assert_eq!(turn.turn().await, 0);
            assert_eq!(turn.current_player().await, 1);

            assert_eq!(turn.start().orders_results.len(), 0);
            assert_eq!(turn.start().production_outcomes.len(), 0);

            turn.force_end_turn().await.unwrap();
        }

        assert_eq!(game.read().await.turn(), 1);
        assert_eq!(game.read().await.current_player(), 0);
    }
}
