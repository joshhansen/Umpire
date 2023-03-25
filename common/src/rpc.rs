use std::collections::HashSet;

use crate::{
    game::{
        action::{AiPlayerAction, PlayerAction, PlayerActionOutcome},
        ai::fX,
        city::{City, CityID},
        error::GameError,
        map::Tile,
        move_::Move,
        obs::{Obs, ObsTracker},
        unit::{orders::OrdersResult, Unit, UnitID, UnitType},
        PlayerNum, PlayerSecret, TurnNum, UmpireResult,
    },
    util::{Dims, Direction, Location, Wrap2d},
};

/// The Umpire RPC interface. The macro generates a client impl called `UmpireRpcClient`.
#[tarpc::service]
pub trait UmpireRpc {
    async fn wait_my_turn() -> PlayerNum;

    /// For each player in the game, gives the player secret if the player is controlled by this connection
    async fn player_secrets_known() -> Vec<Option<PlayerSecret>>;

    /// The number of players in the game
    async fn num_players() -> PlayerNum;

    async fn turn_is_done() -> bool;

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    async fn victor() -> Option<PlayerNum>;

    async fn player_unit_legal_one_step_destinations(
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<HashSet<Location>>;

    async fn player_unit_legal_directions(
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Vec<Direction>>;

    async fn player_tile(player_secret: PlayerSecret, loc: Location) -> UmpireResult<Option<Tile>>;

    async fn player_obs(player_secret: PlayerSecret, loc: Location) -> UmpireResult<Obs>;

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

    // async fn propose_move_unit_by_id(
    //     id: UnitID,
    //     dest: Location,
    // ) -> Proposed2<Result<Move, MoveError>>;

    async fn move_unit_by_id_avoiding_combat(
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move>;

    // async fn propose_move_unit_by_id_avoiding_combat(
    //     id: UnitID,
    //     dest: Location,
    // ) -> Proposed2<MoveResult>;

    async fn disband_unit_by_id(player_secret: PlayerSecret, id: UnitID) -> UmpireResult<Unit>;

    /// Sets the production of the current player's city at location `loc` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    async fn set_production_by_loc(
        player_secret: PlayerSecret,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>>;

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    async fn set_production_by_id(
        player_secret: PlayerSecret,
        city_id: CityID,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>>;

    async fn clear_production(
        player_secret: PlayerSecret,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> Result<Option<UnitType>, GameError>;

    async fn turn() -> TurnNum;

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
    async fn order_unit_sentry(player_secret: PlayerSecret, unit_id: UnitID) -> OrdersResult;

    async fn order_unit_skip(player_secret: PlayerSecret, unit_id: UnitID) -> OrdersResult;

    async fn order_unit_go_to(
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult;

    /// Simulate ordering the specified unit to go to the given location
    // async fn propose_order_unit_go_to(unit_id: UnitID, dest: Location) -> Proposed<OrdersResult>;

    async fn order_unit_explore(player_secret: PlayerSecret, unit_id: UnitID) -> OrdersResult;

    /// Simulate ordering the specified unit to explore.
    // async fn propose_order_unit_explore(unit_id: UnitID) -> Proposed<OrdersResult>;

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    async fn activate_unit_by_loc(player_secret: PlayerSecret, loc: Location) -> UmpireResult<()>;

    /// Feature vector for use in AI training
    ///
    /// Map of the output vector:
    ///
    /// # 15: 1d features
    /// * 1: current turn
    /// * 1: current player city count
    /// * 1: number of tiles observed by current player
    /// * 1: percentage of tiles observed by current player
    /// * 11: the type of unit being represented, where "city" is also a type of unit (one hot encoded)
    /// * 10: number of units controlled by current player (infantry, armor, fighters, bombers, transports, destroyers
    ///                                                     submarines, cruisers, battleships, carriers)
    /// # 363: 2d features, three layers
    /// * 121: is_enemy_belligerent (11x11)
    /// * 121: is_observed (11x11)
    /// * 121: is_neutral (11x11)
    ///
    async fn features(player_secret: PlayerSecret) -> UmpireResult<Vec<fX>>;

    async fn player_score(player_secret: PlayerSecret) -> UmpireResult<f64>;

    async fn take_simple_action(
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome>;

    async fn take_action(
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> UmpireResult<PlayerActionOutcome>;
}
