use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
};

use async_trait::async_trait;
use tarpc::context;

use crate::{
    game::{
        action::{
            AiPlayerAction, NextCityAction, NextUnitAction, PlayerAction, PlayerActionOutcome,
        },
        ai::{fX, TrainingFocus},
        city::{City, CityID},
        error::GameError,
        map::Tile,
        move_::Move,
        obs::{LocatedObsLite, Obs, ObsTracker},
        unit::{
            orders::{Orders, OrdersResult},
            Unit, UnitID, UnitType,
        },
        ActionNum, Game, IGame, OrdersSet, PlayerNum, PlayerSecret, PlayerType, ProductionCleared,
        ProductionSet, ProposedActionResult, ProposedOrdersResult, ProposedResult, TurnEnded,
        TurnNum, TurnPhase, TurnStart, UmpireResult, UnitDisbanded,
    },
    util::{Dims, Direction, Location, Wrap2d},
};

/// The Umpire RPC interface. The macro generates a client impl called `UmpireRpcClient`.
#[tarpc::service]
pub trait UmpireRpc {
    async fn wait_my_turn() -> PlayerNum;

    /// For each player in the game, gives the player secret if the player is controlled by this connection
    async fn player_secrets_known() -> Vec<Option<PlayerSecret>>;

    async fn player_types() -> Vec<PlayerType>;

    /// The number of players in the game
    async fn num_players() -> PlayerNum;

    async fn turn_is_done(player: PlayerNum, turn: TurnNum) -> UmpireResult<bool>;

    async fn current_turn_is_done() -> bool;

    async fn begin_turn(
        player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart>;

    async fn end_turn(player_secret: PlayerSecret) -> UmpireResult<TurnEnded>;

    async fn force_end_turn(player_secret: PlayerSecret) -> UmpireResult<TurnEnded>;

    async fn is_player_turn(secret: PlayerSecret) -> UmpireResult<bool>;

    async fn end_then_begin_turn(
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart>;

    async fn force_end_then_begin_turn(
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart>;

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    async fn victor() -> Option<PlayerNum>;

    async fn player_unit_legal_one_step_destinations(
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<BTreeSet<Location>>;

    async fn player_unit_legal_directions(
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Vec<Direction>>;

    async fn player_tile(player_secret: PlayerSecret, loc: Location) -> UmpireResult<Option<Tile>>;

    async fn player_obs(player_secret: PlayerSecret, loc: Location) -> UmpireResult<Option<Obs>>;

    async fn player_observations(player_secret: PlayerSecret) -> UmpireResult<ObsTracker>;

    /// Every city controlled by the player whose secret is provided
    async fn player_cities(player_secret: PlayerSecret) -> UmpireResult<Vec<City>>;

    /// All cities controlled by the specified player which have a production target set
    async fn player_cities_with_production_target(
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<City>>;

    async fn player_cities_producing_or_not_ignored(
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize>;

    async fn player_city_count(player_secret: PlayerSecret) -> UmpireResult<usize>;

    async fn player_unit_type_counts(
        player_secret: PlayerSecret,
    ) -> UmpireResult<BTreeMap<UnitType, usize>>;

    async fn player_units(player_secret: PlayerSecret) -> UmpireResult<Vec<Unit>>;

    async fn player_city_by_loc(
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<City>>;

    async fn player_city_by_id(
        player_secret: PlayerSecret,
        city_id: CityID,
    ) -> UmpireResult<Option<City>>;

    async fn player_unit_by_id(
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Unit>>;

    async fn player_unit_loc(
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Location>>;

    async fn player_production_set_requests(
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Location>>;

    async fn player_unit_orders_requests(player_secret: PlayerSecret) -> UmpireResult<Vec<UnitID>>;

    async fn player_units_with_orders_requests(
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Unit>>;

    async fn player_units_with_pending_orders(
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>>;

    async fn player_next_unit_legal_actions(
        player_secret: PlayerSecret,
    ) -> UmpireResult<BTreeSet<NextUnitAction>>;

    async fn player_next_city_legal_actions(
        player_secret: PlayerSecret,
    ) -> UmpireResult<BTreeSet<NextCityAction>>;

    async fn player_toplevel_unit_by_loc(
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Unit>>;

    // Movement-related methods

    async fn move_toplevel_unit_by_id(
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    async fn move_toplevel_unit_by_id_avoiding_combat(
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    async fn move_toplevel_unit_by_loc(
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move>;

    async fn move_toplevel_unit_by_loc_avoiding_combat(
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move>;

    async fn move_unit_by_id_in_direction(
        player_secret: PlayerSecret,
        id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move>;

    async fn move_unit_by_id(
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    async fn propose_move_unit_by_id(
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError>;

    async fn move_unit_by_id_avoiding_combat(
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    async fn propose_move_unit_by_id_avoiding_combat(
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError>;

    async fn disband_unit_by_id(
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<UnitDisbanded>;

    /// Sets the production of the current player's city at location `loc` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    async fn set_production_by_loc(
        player_secret: PlayerSecret,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<ProductionSet>;

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    async fn set_production_by_id(
        player_secret: PlayerSecret,
        city_id: CityID,
        production: UnitType,
    ) -> UmpireResult<ProductionSet>;

    async fn clear_production(
        player_secret: PlayerSecret,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<ProductionCleared>;

    async fn clear_productions(
        player_secret: PlayerSecret,
        ignore_cleared_production: bool,
    ) -> UmpireResult<Vec<ProductionCleared>>;

    async fn turn() -> TurnNum;

    async fn player_action(player_secret: PlayerSecret) -> UmpireResult<ActionNum>;

    async fn turn_phase() -> TurnPhase;

    async fn current_player() -> PlayerNum;

    /// The logical dimensions of the game map
    async fn dims() -> Dims;

    async fn wrapping() -> Wrap2d;

    /// Units that could be produced by a city located at the given location
    async fn valid_productions(
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>>;

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    async fn valid_productions_conservative(
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>>;

    /// If the current player controls a unit with ID `id`, order it to sentry
    async fn order_unit_sentry(
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<OrdersSet>;

    async fn order_unit_skip(
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<OrdersSet>;

    async fn order_unit_go_to(
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult;

    /// Simulate ordering the specified unit to go to the given location
    async fn propose_order_unit_go_to(
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult;

    async fn order_unit_explore(player_secret: PlayerSecret, unit_id: UnitID) -> OrdersResult;

    /// Simulate ordering the specified unit to explore.
    async fn propose_order_unit_explore(
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> ProposedOrdersResult;

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    async fn activate_unit_by_loc(
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<LocatedObsLite>;

    async fn set_orders(
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> UmpireResult<OrdersSet>;

    async fn clear_orders(player_secret: PlayerSecret, id: UnitID) -> UmpireResult<Option<Orders>>;

    async fn propose_set_and_follow_orders(
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> ProposedOrdersResult;

    async fn set_and_follow_orders(
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> OrdersResult;

    async fn current_player_score() -> f64;

    async fn player_score(player_secret: PlayerSecret) -> UmpireResult<f64>;

    async fn player_score_by_idx(player: PlayerNum) -> UmpireResult<f64>;

    async fn player_scores() -> Vec<f64>;

    async fn take_simple_action(
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome>;

    async fn take_action(
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> UmpireResult<PlayerActionOutcome>;

    async fn propose_action(
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> ProposedActionResult;

    async fn player_features(
        player_secret: PlayerSecret,
        focus: TrainingFocus,
    ) -> UmpireResult<Vec<fX>>;
}

pub struct RpcGame {
    game: UmpireRpcClient,
}

impl RpcGame {
    pub fn new(game: UmpireRpcClient) -> Self {
        Self { game }
    }
}

#[async_trait]
impl IGame for RpcGame {
    async fn is_player_turn(&self, secret: PlayerSecret) -> UmpireResult<bool> {
        self.game
            .is_player_turn(context::current(), secret)
            .await
            .unwrap()
    }

    async fn num_players(&self) -> PlayerNum {
        self.game.num_players(context::current()).await.unwrap()
    }

    async fn begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        self.game
            .begin_turn(
                context::current(),
                player_secret,
                clear_after_unit_production,
            )
            .await
            .unwrap()
    }

    async fn turn_is_done(&self, player: PlayerNum, turn: TurnNum) -> UmpireResult<bool> {
        self.game
            .turn_is_done(context::current(), player, turn)
            .await
            .unwrap()
    }

    async fn current_turn_is_done(&self) -> bool {
        self.game
            .current_turn_is_done(context::current())
            .await
            .unwrap()
    }

    async fn victor(&self) -> Option<PlayerNum> {
        self.game.victor(context::current()).await.unwrap()
    }

    async fn end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<TurnEnded> {
        self.game
            .end_turn(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn force_end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<TurnEnded> {
        self.game
            .force_end_turn(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        self.game
            .end_then_begin_turn(
                context::current(),
                player_secret,
                next_player_secret,
                clear_after_unit_production,
            )
            .await
            .unwrap()
    }

    async fn force_end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        self.game
            .force_end_then_begin_turn(
                context::current(),
                player_secret,
                next_player_secret,
                clear_after_unit_production,
            )
            .await
            .unwrap()
    }

    async fn player_unit_legal_one_step_destinations(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<BTreeSet<Location>> {
        self.game
            .player_unit_legal_one_step_destinations(context::current(), player_secret, unit_id)
            .await
            .unwrap()
    }

    async fn player_unit_legal_directions(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Vec<Direction>> {
        self.game
            .player_unit_legal_directions(context::current(), player_secret, unit_id)
            .await
            .unwrap()
    }

    async fn player_tile(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Cow<Tile>>> {
        self.game
            .player_tile(context::current(), player_secret, loc)
            .await
            .unwrap()
            .map(|tile| tile.map(Cow::Owned))
    }

    async fn player_obs(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Obs>> {
        self.game
            .player_obs(context::current(), player_secret, loc)
            .await
            .unwrap()
    }

    async fn player_observations(&self, player_secret: PlayerSecret) -> UmpireResult<ObsTracker> {
        self.game
            .player_observations(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_cities(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<City>> {
        self.game
            .player_cities(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_cities_with_production_target(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<City>> {
        self.game
            .player_cities_with_production_target(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_city_count(&self, player_secret: PlayerSecret) -> UmpireResult<usize> {
        self.game
            .player_city_count(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_cities_producing_or_not_ignored(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize> {
        self.game
            .player_cities_producing_or_not_ignored(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_units(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<Unit>> {
        self.game
            .player_units(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_unit_type_counts(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<BTreeMap<UnitType, usize>> {
        self.game
            .player_unit_type_counts(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_city_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<City>> {
        self.game
            .player_city_by_loc(context::current(), player_secret, loc)
            .await
            .unwrap()
    }

    async fn player_city_by_id(
        &self,
        player_secret: PlayerSecret,
        city_id: CityID,
    ) -> UmpireResult<Option<City>> {
        self.game
            .player_city_by_id(context::current(), player_secret, city_id)
            .await
            .unwrap()
    }

    async fn player_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Unit>> {
        self.game
            .player_unit_by_id(context::current(), player_secret, id)
            .await
            .unwrap()
    }

    async fn player_unit_loc(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Location>> {
        self.game
            .player_unit_loc(context::current(), player_secret, id)
            .await
            .unwrap()
    }

    async fn player_toplevel_unit_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Unit>> {
        self.game
            .player_toplevel_unit_by_loc(context::current(), player_secret, loc)
            .await
            .unwrap()
    }

    async fn player_production_set_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Location>> {
        self.game
            .player_production_set_requests(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_unit_orders_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>> {
        self.game
            .player_unit_orders_requests(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_units_with_orders_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Unit>> {
        self.game
            .player_units_with_orders_requests(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_units_with_pending_orders(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>> {
        self.game
            .player_units_with_pending_orders(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_next_unit_legal_actions(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<BTreeSet<NextUnitAction>> {
        self.game
            .player_next_unit_legal_actions(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_next_city_legal_actions(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<BTreeSet<NextCityAction>> {
        self.game
            .player_next_city_legal_actions(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn move_toplevel_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .move_toplevel_unit_by_id(context::current(), player_secret, unit_id, dest)
            .await
            .unwrap()
    }

    async fn move_toplevel_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .move_toplevel_unit_by_id_avoiding_combat(
                context::current(),
                player_secret,
                unit_id,
                dest,
            )
            .await
            .unwrap()
    }

    async fn move_toplevel_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .move_toplevel_unit_by_loc(context::current(), player_secret, src, dest)
            .await
            .unwrap()
    }

    async fn move_toplevel_unit_by_loc_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .move_toplevel_unit_by_loc_avoiding_combat(context::current(), player_secret, src, dest)
            .await
            .unwrap()
    }

    async fn move_unit_by_id_in_direction(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move> {
        self.game
            .move_unit_by_id_in_direction(context::current(), player_secret, unit_id, direction)
            .await
            .unwrap()
    }

    async fn move_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .move_unit_by_id(context::current(), player_secret, unit_id, dest)
            .await
            .unwrap()
    }

    async fn propose_move_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        self.game
            .propose_move_unit_by_id(context::current(), player_secret, id, dest)
            .await
            .unwrap()
    }

    async fn move_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .move_unit_by_id_avoiding_combat(context::current(), player_secret, id, dest)
            .await
            .unwrap()
    }

    async fn propose_move_unit_by_id_avoiding_combat(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        self.game
            .propose_move_unit_by_id_avoiding_combat(context::current(), player_secret, id, dest)
            .await
            .unwrap()
    }

    async fn disband_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<UnitDisbanded> {
        self.game
            .disband_unit_by_id(context::current(), player_secret, unit_id)
            .await
            .unwrap()
    }

    async fn set_production_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<ProductionSet> {
        self.game
            .set_production_by_loc(context::current(), player_secret, loc, production)
            .await
            .unwrap()
    }

    async fn set_production_by_id(
        &mut self,
        player_secret: PlayerSecret,
        city_id: CityID,
        production: UnitType,
    ) -> UmpireResult<ProductionSet> {
        self.game
            .set_production_by_id(context::current(), player_secret, city_id, production)
            .await
            .unwrap()
    }

    async fn clear_production(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<ProductionCleared> {
        self.game
            .clear_production(
                context::current(),
                player_secret,
                loc,
                ignore_cleared_production,
            )
            .await
            .unwrap()
    }

    async fn clear_productions(
        &mut self,
        player_secret: PlayerSecret,
        ignore_cleared_productions: bool,
    ) -> UmpireResult<Vec<ProductionCleared>> {
        self.game
            .clear_productions(
                context::current(),
                player_secret,
                ignore_cleared_productions,
            )
            .await
            .unwrap()
    }

    async fn turn(&self) -> TurnNum {
        self.game.turn(context::current()).await.unwrap()
    }

    async fn player_action(&self, player_secret: PlayerSecret) -> UmpireResult<ActionNum> {
        self.game
            .player_action(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn turn_phase(&self) -> TurnPhase {
        self.game.turn_phase(context::current()).await.unwrap()
    }

    async fn current_player(&self) -> PlayerNum {
        self.game.current_player(context::current()).await.unwrap()
    }

    async fn dims(&self) -> Dims {
        self.game.dims(context::current()).await.unwrap()
    }

    async fn wrapping(&self) -> Wrap2d {
        self.game.wrapping(context::current()).await.unwrap()
    }

    async fn player_features(
        &self,
        _player_secret: PlayerSecret,
        _focus: TrainingFocus,
    ) -> UmpireResult<Vec<fX>> {
        unimplemented!();
    }

    async fn valid_productions(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>> {
        self.game
            .valid_productions(context::current(), player_secret, loc)
            .await
            .unwrap()
    }

    async fn valid_productions_conservative(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>> {
        self.game
            .valid_productions_conservative(context::current(), player_secret, loc)
            .await
            .unwrap()
    }

    async fn order_unit_sentry(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<OrdersSet> {
        self.game
            .order_unit_sentry(context::current(), player_secret, unit_id)
            .await
            .unwrap()
    }

    async fn order_unit_skip(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<OrdersSet> {
        self.game
            .order_unit_skip(context::current(), player_secret, unit_id)
            .await
            .unwrap()
    }

    async fn order_unit_go_to(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult {
        self.game
            .order_unit_go_to(context::current(), player_secret, unit_id, dest)
            .await
            .unwrap()
    }

    async fn propose_order_unit_go_to(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult {
        self.game
            .propose_order_unit_go_to(context::current(), player_secret, unit_id, dest)
            .await
            .unwrap()
    }

    async fn order_unit_explore(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.game
            .order_unit_explore(context::current(), player_secret, unit_id)
            .await
            .unwrap()
    }

    async fn propose_order_unit_explore(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> ProposedOrdersResult {
        self.game
            .propose_order_unit_explore(context::current(), player_secret, unit_id)
            .await
            .unwrap()
    }

    async fn activate_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<LocatedObsLite> {
        self.game
            .activate_unit_by_loc(context::current(), player_secret, loc)
            .await
            .unwrap()
    }

    async fn set_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> UmpireResult<OrdersSet> {
        self.game
            .set_orders(context::current(), player_secret, id, orders)
            .await
            .unwrap()
    }

    async fn clear_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Orders>> {
        self.game
            .clear_orders(context::current(), player_secret, id)
            .await
            .unwrap()
    }

    async fn propose_set_and_follow_orders(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> ProposedOrdersResult {
        self.game
            .propose_set_and_follow_orders(context::current(), player_secret, id, orders)
            .await
            .unwrap()
    }

    async fn set_and_follow_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> OrdersResult {
        self.game
            .set_and_follow_orders(context::current(), player_secret, id, orders)
            .await
            .unwrap()
    }

    async fn current_player_score(&self) -> f64 {
        self.game
            .current_player_score(context::current())
            .await
            .unwrap()
    }

    async fn player_score(&self, player_secret: PlayerSecret) -> UmpireResult<f64> {
        self.game
            .player_score(context::current(), player_secret)
            .await
            .unwrap()
    }

    async fn player_score_by_idx(&self, player: PlayerNum) -> UmpireResult<f64> {
        self.game
            .player_score_by_idx(context::current(), player)
            .await
            .unwrap()
    }

    async fn player_scores(&self) -> Vec<f64> {
        self.game.player_scores(context::current()).await.unwrap()
    }

    async fn take_simple_action(
        &mut self,
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        self.game
            .take_simple_action(context::current(), player_secret, action)
            .await
            .unwrap()
    }

    async fn take_action(
        &mut self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        self.game
            .take_action(context::current(), player_secret, action)
            .await
            .unwrap()
    }

    async fn propose_action(
        &self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> ProposedActionResult {
        self.game
            .propose_action(context::current(), player_secret, action)
            .await
            .unwrap()
    }

    //FIXME Refused bequest
    fn clone_underlying_game_state(&self) -> Result<Game, String> {
        Err(String::from(
            "RpcGame does not implement cloning of underlying game state because it's remote",
        ))
    }
}
