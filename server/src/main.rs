use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, RwLock as RwLockStd},
};

use common::{
    cli::{self, players_arg},
    conf,
    game::{
        action::{AiPlayerAction, PlayerAction, PlayerActionOutcome},
        ai::fX,
        city::{City, CityID},
        error::GameError,
        map::Tile,
        move_::Move,
        obs::{LocatedObs, LocatedObsLite, Obs, ObsTracker},
        player::PlayerControl,
        turn_async::TurnTaker,
        unit::{
            orders::{Orders, OrdersResult},
            Unit, UnitID, UnitType,
        },
        Game, IGame, PlayerNum, PlayerSecret, PlayerType, ProposedActionResult,
        ProposedOrdersResult, ProposedResult, TurnNum, TurnPhase, TurnStart, UmpireResult,
    },
    name::{city_namer, unit_namer},
    rpc::UmpireRpc,
    util::{Dims, Direction, Location, Wrap2d},
};
use futures::{future, prelude::*};
use serde::{Deserialize, Serialize};
use tarpc::{
    context::Context,
    server::{self, incoming::Incoming, Channel},
    tokio_serde::formats::Bincode,
};
use tokio::sync::RwLock as RwLockTokio;
use umpire_ai::AI;

#[derive(Debug, Deserialize, Serialize)]
enum ServerEvent {
    PlayerObservations {
        player: PlayerNum,
        observations: Vec<LocatedObs>,
    },
    PlayerTurnStart {
        player: PlayerNum,
        turn_start: TurnStart,
    },
    OtherPlayerTurnStart {
        player: PlayerNum,
        turn: TurnNum,
    },
}

// Implementation of the server API
#[derive(Clone)]
struct UmpireServer {
    game: Arc<RwLockTokio<Game>>,

    /// The player secrets for players controlled by this connection will be given, the rest omitted
    known_secrets: Vec<Option<PlayerSecret>>,

    player_types: Vec<PlayerType>,
}

#[tarpc::server]
impl UmpireRpc for UmpireServer {
    /// NOTE This is really aggressive!
    async fn wait_my_turn(self, _: Context) -> PlayerNum {
        loop {
            let g = self.game.read().await;
            let player = g.current_player();
            if self.known_secrets[player].is_some() {
                return player;
            }
        }
    }

    async fn player_secrets_known(self, _: Context) -> Vec<Option<PlayerSecret>> {
        self.known_secrets
    }

    async fn player_types(self, _: Context) -> Vec<PlayerType> {
        self.player_types
    }

    async fn num_players(self, _: Context) -> PlayerNum {
        self.game.read().await.num_players()
    }

    async fn turn_is_done(
        self,
        _: Context,
        player: PlayerNum,
        turn: TurnNum,
    ) -> UmpireResult<bool> {
        self.game.read().await.turn_is_done(player, turn)
    }

    async fn current_turn_is_done(self, _: Context) -> bool {
        self.game.read().await.current_turn_is_done()
    }

    async fn begin_turn(self, _: Context, player_secret: PlayerSecret) -> UmpireResult<TurnStart> {
        self.game.write().await.begin_turn(player_secret)
    }

    async fn begin_turn_clearing(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.game.write().await.begin_turn_clearing(player_secret)
    }

    async fn end_turn(self, _: Context, player_secret: PlayerSecret) -> UmpireResult<()> {
        self.game.write().await.end_turn(player_secret)
    }

    async fn force_end_turn(self, _: Context, player_secret: PlayerSecret) -> UmpireResult<()> {
        self.game.write().await.force_end_turn(player_secret)
    }

    async fn is_player_turn(self, _: Context, secret: PlayerSecret) -> UmpireResult<bool> {
        self.game.read().await.is_player_turn(secret)
    }

    async fn end_then_begin_turn(
        self,
        _: Context,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.game
            .write()
            .await
            .end_then_begin_turn(player_secret, next_player_secret)
    }

    async fn end_then_begin_turn_clearing(
        self,
        _: Context,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.game
            .write()
            .await
            .end_then_begin_turn_clearing(player_secret, next_player_secret)
    }

    async fn force_end_then_begin_turn(
        self,
        _: Context,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.game
            .write()
            .await
            .force_end_then_begin_turn(player_secret, next_player_secret)
    }

    async fn force_end_then_begin_turn_clearing(
        self,
        _: Context,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.game
            .write()
            .await
            .force_end_then_begin_turn_clearing(player_secret, next_player_secret)
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    async fn victor(self, _: Context) -> Option<PlayerNum> {
        self.game.read().await.victor()
    }

    async fn player_unit_legal_one_step_destinations(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<HashSet<Location>> {
        self.game
            .read()
            .await
            .player_unit_legal_one_step_destinations(player_secret, unit_id)
    }

    async fn player_unit_legal_directions(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Vec<Direction>> {
        self.game
            .read()
            .await
            .player_unit_legal_directions(player_secret, unit_id)
            .map(|d| d.collect())
    }

    async fn player_tile(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Tile>> {
        self.game
            .read()
            .await
            .player_tile(player_secret, loc)
            .map(|tile| tile.cloned())
    }

    async fn player_obs(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Obs> {
        self.game
            .read()
            .await
            .player_obs(player_secret, loc)
            .map(|obs| obs.clone())
    }

    async fn player_observations(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<ObsTracker> {
        self.game
            .read()
            .await
            .player_observations(player_secret)
            .map(|observations| observations.clone())
    }

    /// Every city controlled by the player whose secret is provided
    async fn player_cities(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<City>> {
        self.game
            .read()
            .await
            .player_cities(player_secret)
            .map(|cities| cities.cloned().collect())
    }

    /// All cities controlled by the current player which have a production target set
    async fn player_cities_with_production_target(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<City>> {
        self.game
            .read()
            .await
            .player_cities_with_production_target(player_secret)
            .map(|cities_iter| cities_iter.cloned().collect())
    }

    async fn player_city_count(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize> {
        self.game.read().await.player_city_count(player_secret)
    }

    async fn player_cities_producing_or_not_ignored(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize> {
        self.game
            .read()
            .await
            .player_cities_producing_or_not_ignored(player_secret)
    }

    async fn player_units(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Unit>> {
        self.game
            .read()
            .await
            .player_units(player_secret)
            .map(|units| units.cloned().collect())
    }

    async fn player_unit_type_counts(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<HashMap<UnitType, usize>> {
        self.game
            .read()
            .await
            .player_unit_type_counts(player_secret)
            .map(|counts| counts.clone())
    }

    async fn player_city_by_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<City>> {
        self.game
            .read()
            .await
            .player_city_by_loc(player_secret, loc)
            .map(|city| city.cloned())
    }

    async fn player_city_by_id(
        self,
        _: Context,
        player_secret: PlayerSecret,
        city_id: CityID,
    ) -> UmpireResult<Option<City>> {
        self.game
            .read()
            .await
            .player_city_by_id(player_secret, city_id)
            .map(|city| city.cloned())
    }

    async fn player_unit_by_id(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Unit>> {
        self.game
            .read()
            .await
            .player_unit_by_id(player_secret, id)
            .map(|maybe_unit| maybe_unit.cloned())
    }

    async fn player_unit_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Location>> {
        self.game.read().await.player_unit_loc(player_secret, id)
    }

    async fn player_toplevel_unit_by_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Unit>> {
        self.game
            .read()
            .await
            .player_toplevel_unit_by_loc(player_secret, loc)
            .map(|unit| unit.cloned())
    }

    async fn player_production_set_requests(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Location>> {
        self.game
            .read()
            .await
            .player_production_set_requests(player_secret)
            .map(|rqsts| rqsts.collect())
    }

    async fn player_unit_orders_requests(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>> {
        self.game
            .read()
            .await
            .player_unit_orders_requests(player_secret)
            .map(|rqsts| rqsts.collect())
    }

    async fn player_units_with_orders_requests(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Unit>> {
        self.game
            .read()
            .await
            .player_units_with_orders_requests(player_secret)
            .map(|units| units.cloned().collect())
    }

    async fn player_units_with_pending_orders(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>> {
        self.game
            .read()
            .await
            .player_units_with_pending_orders(player_secret)
            .map(|units| units.collect())
    }

    // Movement-related methods

    async fn move_toplevel_unit_by_id(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .await
            .move_toplevel_unit_by_id(player_secret, unit_id, dest)
    }

    async fn move_toplevel_unit_by_id_avoiding_combat(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .await
            .move_toplevel_unit_by_id_avoiding_combat(player_secret, unit_id, dest)
    }

    async fn move_toplevel_unit_by_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .await
            .move_toplevel_unit_by_loc(player_secret, src, dest)
    }

    async fn move_toplevel_unit_by_loc_avoiding_combat(
        self,
        _: Context,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .await
            .move_toplevel_unit_by_loc_avoiding_combat(player_secret, src, dest)
    }

    async fn move_unit_by_id_in_direction(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .await
            .move_unit_by_id_in_direction(player_secret, id, direction)
    }

    async fn move_unit_by_id(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .await
            .move_unit_by_id(player_secret, unit_id, dest)
    }

    async fn propose_move_unit_by_id(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        self.game
            .read()
            .await
            .propose_move_unit_by_id(player_secret, id, dest)
    }

    async fn move_unit_by_id_avoiding_combat(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .await
            .move_unit_by_id_avoiding_combat(player_secret, id, dest)
    }

    async fn propose_move_unit_by_id_avoiding_combat(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        self.game
            .read()
            .await
            .propose_move_unit_by_id_avoiding_combat(player_secret, id, dest)
    }

    async fn disband_unit_by_id(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Unit> {
        self.game
            .write()
            .await
            .disband_unit_by_id(player_secret, id)
    }

    /// Sets the production of the current player's city at location `loc` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    async fn set_production_by_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        self.game
            .write()
            .await
            .set_production_by_loc(player_secret, loc, production)
    }

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    async fn set_production_by_id(
        self,
        _: Context,
        player_secret: PlayerSecret,
        city_id: CityID,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        self.game
            .write()
            .await
            .set_production_by_id(player_secret, city_id, production)
    }

    async fn clear_production(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<Option<UnitType>> {
        self.game
            .write()
            .await
            .clear_production(player_secret, loc, ignore_cleared_production)
    }

    async fn clear_productions(
        self,
        _: Context,
        player_secret: PlayerSecret,
        ignore_cleared_production: bool,
    ) -> UmpireResult<()> {
        self.game
            .write()
            .await
            .clear_productions(player_secret, ignore_cleared_production)
    }

    async fn turn(self, _: Context) -> TurnNum {
        self.game.read().await.turn()
    }

    async fn turn_phase(self, _: Context) -> TurnPhase {
        self.game.read().await.turn_phase()
    }

    async fn current_player(self, _: Context) -> PlayerNum {
        self.game.read().await.current_player()
    }

    /// The logical dimensions of the game map
    async fn dims(self, _: Context) -> Dims {
        self.game.read().await.dims()
    }

    async fn wrapping(self, _: Context) -> Wrap2d {
        self.game.read().await.wrapping()
    }

    /// Units that could be produced by a city located at the given location
    async fn valid_productions(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>> {
        self.game
            .read()
            .await
            .valid_productions(player_secret, loc)
            .map(|prods| prods.collect())
    }

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    async fn valid_productions_conservative(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>> {
        self.game
            .read()
            .await
            .valid_productions_conservative(player_secret, loc)
            .map(|prods| prods.collect())
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    async fn order_unit_sentry(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.game
            .write()
            .await
            .order_unit_sentry(player_secret, unit_id)
    }

    async fn order_unit_skip(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.game
            .write()
            .await
            .order_unit_skip(player_secret, unit_id)
    }

    async fn order_unit_go_to(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult {
        self.game
            .write()
            .await
            .order_unit_go_to(player_secret, unit_id, dest)
    }

    async fn propose_order_unit_go_to(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult {
        self.game
            .read()
            .await
            .propose_order_unit_go_to(player_secret, unit_id, dest)
    }

    async fn order_unit_explore(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.game
            .write()
            .await
            .order_unit_explore(player_secret, unit_id)
    }

    async fn propose_order_unit_explore(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> ProposedOrdersResult {
        self.game
            .read()
            .await
            .propose_order_unit_explore(player_secret, unit_id)
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    async fn activate_unit_by_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<LocatedObsLite> {
        self.game
            .write()
            .await
            .activate_unit_by_loc(player_secret, loc)
    }

    async fn set_orders(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> UmpireResult<Option<Orders>> {
        self.game
            .write()
            .await
            .set_orders(player_secret, id, orders)
    }

    async fn clear_orders(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Orders>> {
        self.game.write().await.clear_orders(player_secret, id)
    }

    async fn propose_set_and_follow_orders(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> ProposedOrdersResult {
        self.game
            .read()
            .await
            .propose_set_and_follow_orders(player_secret, id, orders)
    }

    async fn set_and_follow_orders(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> OrdersResult {
        self.game
            .write()
            .await
            .set_and_follow_orders(player_secret, id, orders)
    }

    // async fn propose_end_turn(self, _: Context) -> (Game, Result<TurnStart, PlayerNum>) {
    //     let (mut game, secrets) = self.game.clone();
    //     let result = game.end_turn();
    //     (game, result)
    // }

    // async fn apply_proposal<T>(mut self, _: Context, proposal: Proposed<T>) -> T {
    //     proposal.apply(&mut self.game)
    // }

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
    async fn player_features(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<fX>> {
        self.game.read().await.player_features(player_secret)
    }

    async fn current_player_score(self, _: Context) -> f64 {
        self.game.read().await.current_player_score()
    }

    async fn player_score(self, _: Context, player_secret: PlayerSecret) -> UmpireResult<f64> {
        self.game.read().await.player_score(player_secret)
    }

    async fn player_score_by_idx(self, _: Context, player: PlayerNum) -> UmpireResult<f64> {
        self.game.read().await.player_score_by_idx(player)
    }

    async fn player_scores(self, _: Context) -> Vec<f64> {
        self.game.read().await.player_scores()
    }

    async fn take_simple_action(
        self,
        _: Context,
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        self.game
            .write()
            .await
            .take_simple_action(player_secret, action)
    }

    async fn take_action(
        self,
        _: Context,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> Result<PlayerActionOutcome, GameError> {
        self.game.write().await.take_action(player_secret, action)
    }

    async fn propose_action(
        self,
        _: Context,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> ProposedActionResult {
        self.game.read().await.propose_action(player_secret, action)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = cli::app("umpired", "fwWH")
        .version(conf::APP_VERSION)
        .author("Josh Hansen <hansen.joshuaa@gmail.com>")
        .about(conf::APP_SUBTITLE)
        .arg(players_arg().default_value("h123"))
        .get_matches();

    let fog_of_war = matches.get_one::<bool>("fog").unwrap().clone();

    let player_types = matches
        .get_one::<Vec<PlayerType>>("players")
        .unwrap()
        .clone();

    let num_players: PlayerNum = player_types.len();

    let human_player_indices: Vec<usize> = player_types
        .iter()
        .filter(|pt| **pt == PlayerType::Human)
        .enumerate()
        .map(|(i, _pt)| i)
        .collect();
    let num_humans = human_player_indices.len();

    let map_width = matches.get_one::<u16>("map_width").unwrap().clone();
    let map_height = matches.get_one::<u16>("map_height").unwrap().clone();
    let wrapping = matches.get_one::<Wrap2d>("wrapping").unwrap().clone();

    let map_dims: Dims = Dims::new(map_width, map_height);
    if (map_dims.area() as PlayerNum) < num_players {
        panic!("Map dimensions of {} give an area of {} which is not enough room for {} players; area of {} or greater required.",
        map_dims, map_dims.area(), num_players, num_players);
    }

    let city_namer = city_namer();
    let unit_namer = unit_namer();

    let (game, secrets) = Game::new(
        map_dims,
        city_namer,
        num_players,
        fog_of_war,
        Some(Arc::new(std::sync::RwLock::new(unit_namer))),
        wrapping,
    );

    // Vector of known player secrets for each player's connection
    let known_secrets: Vec<Vec<Option<PlayerSecret>>> = (0..num_players)
        .map(|player| {
            secrets
                .iter()
                .enumerate()
                .map(|(i, secret)| if i == player { Some(*secret) } else { None })
                .collect()
        })
        .collect();

    let game = Arc::new(RwLockTokio::new(game));
    // let secrets = Arc::new(RwLockTokio::new(secrets));
    // let player_types = Arc::new(RwLock::new(player_types));

    let connection_count = Arc::new(RwLockStd::new(0usize));

    // let server_addr = (IpAddr::V6(Ipv6Addr::LOCALHOST), 21131);

    let server_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), conf::PORT);

    // JSON transport is provided by the json_transport tarpc module. It makes it easy
    // to start up a serde-powered json serialization strategy over TCP.

    let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Bincode::default).await?;
    // tracing::info!("Listening on port {}", listener.local_addr().port());
    listener.config_mut().max_frame_length(usize::MAX);

    let ai_thread = {
        let game = Arc::clone(&game);
        let player_types = player_types.clone();
        tokio::spawn(async move {
            let unique_ai_ptypes: HashSet<PlayerType> = player_types
                .iter()
                .filter(|ptype| **ptype != PlayerType::Human)
                .cloned()
                .collect();

            let mut ais: HashMap<PlayerType, AI> = HashMap::with_capacity(unique_ai_ptypes.len());

            let mut ai_ctrls: Vec<Option<PlayerControl>> = Vec::with_capacity(num_players);

            for player in 0..num_players {
                ai_ctrls.push(match player_types[player] {
                    PlayerType::AI(ref _aispec) => {
                        let secret = secrets[player];
                        Some(
                            PlayerControl::new(
                                Arc::clone(&game) as Arc<RwLockTokio<dyn IGame>>,
                                player,
                                secret,
                            )
                            .await,
                        )
                    }
                    _ => None,
                });
            }

            for ptype in unique_ai_ptypes.iter() {
                let ai: AI = match ptype {
                    PlayerType::AI(aispec) => aispec.clone().into(),
                    _ => unreachable!(),
                };
                ais.insert(ptype.clone(), ai);
            }

            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                let g = game.read().await;

                let player = g.current_player();

                let ptype = &player_types[player];

                if let Some(ai) = ais.get_mut(&ptype) {
                    let ctrl = &mut ai_ctrls[player].as_mut().unwrap();

                    let mut turn = ctrl.turn_ctrl().await;

                    ai.take_turn(&mut turn, false).await;

                    turn.force_end_turn().await.unwrap();
                }
            }
        })
    };

    listener
        // Ignore accept errors.
        .filter_map(|r| future::ready(r.ok()))
        .map(server::BaseChannel::with_defaults)
        // Limit channels to 4 per IP.
        .max_channels_per_key(4, |t| t.transport().peer_addr().unwrap().ip())
        // serve is generated by the service attribute. It takes as input any type implementing
        // the generated World trait.
        .filter(|_channel| {
            let cc = connection_count.clone();

            let human = *cc.read().unwrap();

            future::ready(human_player_indices.contains(&human))
        })
        .map(|channel| {
            let cc = connection_count.clone();

            let human = *cc.read().unwrap();

            *cc.write().unwrap() += 1;

            let player = human_player_indices[human];

            println!("Serving player {} on connection {}", player, human);

            let server = UmpireServer {
                game: Arc::clone(&game),
                known_secrets: known_secrets[player].clone(),
                player_types: player_types.clone(),
            };

            channel.execute(server.serve())
        })
        // Max channels.
        .buffer_unordered(num_humans)
        .for_each(|_| async {})
        .await;

    ai_thread.await.unwrap();

    Ok(())
}
