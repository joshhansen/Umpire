use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use async_trait::async_trait;

use crate::{
    game::{
        city::{City, CityID},
        error::GameError,
        map::Tile,
        obs::{Obs, ObsTracker},
        unit::{
            orders::{Orders, OrdersResult},
            Unit, UnitID, UnitType,
        },
    },
    util::{Dims, Direction, Location, Wrap2d},
};

use super::{
    action::{AiPlayerAction, NextCityAction, NextUnitAction, PlayerAction, PlayerActionOutcome},
    ai::fX,
    move_::Move,
    obs::LocatedObsLite,
    player::PlayerNum,
    Game, PlayerSecret, ProposedActionResult, ProposedOrdersResult, ProposedResult, TurnNum,
    TurnPhase, TurnStart, UmpireResult,
};

#[async_trait]
pub trait IGame: Send + Sync {
    async fn num_players(&self) -> PlayerNum;

    async fn is_player_turn(&self, secret: PlayerSecret) -> UmpireResult<bool>;

    async fn begin_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<TurnStart>;

    /// Begin the turn of the specified player, claring productions
    async fn begin_turn_clearing(&mut self, player_secret: PlayerSecret)
        -> UmpireResult<TurnStart>;

    /// Indicates whether the given player has completed the specified turn, or not
    ///
    /// This is public information.
    async fn turn_is_done(&self, player: PlayerNum, turn: TurnNum) -> UmpireResult<bool>;

    async fn turn_phase(&self) -> TurnPhase;

    async fn current_turn_is_done(&self) -> bool;

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    ///
    /// Defeat is defined as having no cities and having no units that can capture cities
    async fn victor(&self) -> Option<PlayerNum>;

    async fn end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<()>;

    async fn force_end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<()>;

    /// End the current human player's turn and begin the next human player's turn
    ///
    /// Returns the number of the now-current player.
    /// Ok if the turn ended properly
    /// Err if not
    ///
    /// If the requirements for ending the turn weren't met, it will remain the turn of the player that was playing
    /// when this method was called.
    ///
    /// If the requirements for ending the turn were met the next player's turn will begin
    ///
    /// At the beginning of a turn, new units will be created as necessary, production counts will be reset as
    /// necessary, and production and movement requests will be created as necessary.
    ///
    /// At the end of a turn, production counts will be incremented.
    async fn end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart>;

    async fn end_then_begin_turn_clearing(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart>;

    /// End the turn without checking that the player has filled all production and orders requests.
    async fn force_end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart>;

    /// End the turn without checking that the player has filled all production and orders requests.
    async fn force_end_then_begin_turn_clearing(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart>;

    /// The set of destinations that the specified unit could actually attempt a move onto in exactly one movement step.
    /// This excludes the unit's original location
    async fn player_unit_legal_one_step_destinations(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<HashSet<Location>>;

    async fn player_unit_legal_directions(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Vec<Direction>>;

    /// The current player's most recent observation of the tile at location `loc`, if any
    async fn player_tile(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Cow<Tile>>>;

    /// The current player's observation at location `loc`
    async fn player_obs(&self, player_secret: PlayerSecret, loc: Location) -> UmpireResult<Obs>;

    async fn player_observations(&self, player_secret: PlayerSecret) -> UmpireResult<ObsTracker>;

    /// Every city controlled by the player whose secret is provided
    async fn player_cities(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<City>>;

    async fn player_cities_with_production_target(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<City>>;

    /// How many cities does the specified player control?
    async fn player_city_count(&self, player_secret: PlayerSecret) -> UmpireResult<usize>;

    /// The number of cities controlled by the current player which either have a production target or
    /// are NOT set to be ignored when requesting productions to be set
    ///
    /// This basically lets us make sure a player doesn't set all their cities' productions to none since
    /// right now the UI has no way of getting out of that situation
    ///
    /// NOTE Maybe we could make the UI smarter and get rid of this?
    async fn player_cities_producing_or_not_ignored(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize>;

    async fn player_units(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<Unit>>;

    /// The counts of unit types controlled by the specified player
    async fn player_unit_type_counts(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<HashMap<UnitType, usize>>;

    /// If the specified player controls a city at location `loc`, return it
    async fn player_city_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<City>>;

    /// If the specified player controls a city with ID `city_id`, return it
    async fn player_city_by_id(
        &self,
        player_secret: PlayerSecret,
        city_id: CityID,
    ) -> UmpireResult<Option<City>>;

    /// If the specified player controls a unit with ID `id`, return it
    async fn player_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Unit>>;

    /// If the specified player controls a unit with ID `id`, return its location
    async fn player_unit_loc(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Location>>;

    /// If the current player controls the top-level unit at location `loc`, return it
    async fn player_toplevel_unit_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Unit>>;

    async fn player_production_set_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Location>>;

    async fn player_unit_orders_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>>;

    /// Which if the specified player's units need orders?
    ///
    /// In other words, which of the specified player's units have no orders and have moves remaining?
    async fn player_units_with_orders_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Unit>>;

    async fn player_units_with_pending_orders(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>>;

    // Movement-related methods

    /// Must be player's turn
    async fn move_toplevel_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    /// Must be player's turn
    async fn move_toplevel_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    /// Must be player's turn
    ///
    /// ## Errors
    /// * If unit at `src` doesn't exist
    /// * If requested move requires more moves than the unit has remaining
    /// * If `dest` is unreachable from `src` (may be subsumed by previous)
    ///
    /// FIXME Make the unit observe at each point along its path
    ///
    /// FIXME This function checks two separate times whether a unit exists at src
    async fn move_toplevel_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move>;

    /// Must be user's turn
    async fn move_toplevel_unit_by_loc_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move>;

    /// Move a unit one step in a particular direction
    ///
    /// Must be player's turn
    async fn move_unit_by_id_in_direction(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move>;

    /// Must be player's turn
    async fn move_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    async fn propose_move_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError>;

    /// Must be player's turn
    async fn move_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    async fn propose_move_unit_by_id_avoiding_combat(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError>;

    /// Disbands
    ///
    /// Must be player's turn
    async fn disband_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Unit>;

    /// Sets the production of the current player's city at location `loc` to `production`, returning the prior setting.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    async fn set_production_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>>;

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    async fn set_production_by_id(
        &mut self,
        player_secret: PlayerSecret,
        city_id: CityID,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>>;

    /// Clears the production of a city at location `loc` if one exists and is controlled by the
    /// specified player.
    ///
    /// Returns the prior production (if any) on success, otherwise `GameError::NoCityAtLocation`
    async fn clear_production(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<Option<UnitType>>;

    async fn clear_productions(
        &mut self,
        player_secret: PlayerSecret,
        ignore_cleared_production: bool,
    ) -> UmpireResult<()>;

    async fn turn(&self) -> TurnNum;

    async fn current_player(&self) -> PlayerNum;

    /// The logical dimensions of the game map
    async fn dims(&self) -> Dims;

    async fn wrapping(&self) -> Wrap2d;

    async fn valid_productions(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>>;

    async fn valid_productions_conservative(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>>;

    /// If the current player controls a unit with ID `id`, order it to sentry
    async fn order_unit_sentry(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult;

    async fn order_unit_skip(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult;

    async fn order_unit_go_to(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult;

    /// Simulate ordering the specified unit to go to the given location
    async fn propose_order_unit_go_to(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult;

    async fn order_unit_explore(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult;

    /// Simulate ordering the specified unit to explore.
    async fn propose_order_unit_explore(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> ProposedOrdersResult;

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    async fn activate_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<LocatedObsLite>;

    /// If the current player controls a unit with ID `id`, set its orders to `orders`
    ///
    /// # Errors
    /// `OrdersError::OrderedUnitDoesNotExist` if the order is not present under the control of the current player
    async fn set_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> UmpireResult<Option<Orders>>;

    /// Clear the orders of the unit controlled by the current player with ID `id`.
    ///
    /// Can happen at any time
    async fn clear_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Orders>>;

    /// Simulate setting the orders of unit with ID `id` to `orders` and then following them out.
    async fn propose_set_and_follow_orders(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> ProposedOrdersResult;

    async fn set_and_follow_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> OrdersResult;

    async fn current_player_score(&self) -> f64;

    async fn player_features(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<fX>>;

    async fn player_score(&self, player_secret: PlayerSecret) -> UmpireResult<f64>;

    async fn player_score_by_idx(&self, player: PlayerNum) -> UmpireResult<f64>;

    /// Each player's current score, indexed by player number
    async fn player_scores(&self) -> Vec<f64>;

    async fn take_simple_action(
        &mut self,
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome>;

    async fn take_next_city_action(
        &mut self,
        player_secret: PlayerSecret,
        action: NextCityAction,
    ) -> UmpireResult<PlayerActionOutcome>;

    async fn take_next_unit_action(
        &mut self,
        player_secret: PlayerSecret,
        action: NextUnitAction,
    ) -> UmpireResult<PlayerActionOutcome>;

    async fn take_action(
        &mut self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> UmpireResult<PlayerActionOutcome>;

    async fn propose_action(
        &self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> ProposedActionResult;

    /// This is an escape hatch for AI training; do NOT expose this via UmpireRpcClient
    fn clone_underlying_game_state(&self) -> Result<Game, String>;
}
