use std::{
    collections::HashSet,
    net::{IpAddr, Ipv6Addr},
    sync::{Arc, RwLock},
};

use common::{
    cli::{self, players_arg},
    conf,
    game::{
        action::{AiPlayerAction, PlayerAction, PlayerActionOutcome},
        ai::{fX, player_features},
        city::{City, CityID},
        error::GameError,
        map::Tile,
        move_::Move,
        obs::{LocatedObs, Obs, ObsTracker},
        unit::{orders::OrdersResult, Unit, UnitID, UnitType},
        Game, PlayerNum, PlayerSecret, PlayerType, TurnNum, TurnStart, UmpireResult,
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
    tokio_serde::formats::Json,
};

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
    game: Arc<RwLock<Game>>,

    /// The player secrets for players controlled by this connection will be given, the rest omitted
    known_secrets: Vec<Option<PlayerSecret>>,
}

#[tarpc::server]
impl UmpireRpc for UmpireServer {
    async fn player_secrets_known(self, _: Context) -> Vec<Option<PlayerSecret>> {
        self.known_secrets
    }

    async fn num_players(self, _: Context) -> PlayerNum {
        self.game.read().unwrap().num_players()
    }

    async fn turn_is_done(self, _: Context) -> bool {
        self.game.read().unwrap().turn_is_done()
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    async fn victor(self, _: Context) -> Option<PlayerNum> {
        self.game.read().unwrap().victor()
    }

    async fn player_unit_legal_one_step_destinations(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<HashSet<Location>> {
        self.game
            .read()
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
            .player_cities_with_production_target(player_secret)
            .map(|cities_iter| cities_iter.cloned().collect())
    }

    async fn player_cities_producing_or_not_ignored(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize> {
        self.game
            .read()
            .unwrap()
            .player_cities_producing_or_not_ignored(player_secret)
    }

    async fn player_units(
        self,
        _: Context,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Unit>> {
        self.game
            .read()
            .unwrap()
            .player_units(player_secret)
            .map(|units| units.cloned().collect())
    }

    async fn player_city_by_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<City>> {
        self.game
            .read()
            .unwrap()
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
            .unwrap()
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
            .unwrap()
            .player_unit_by_id(player_secret, id)
            .map(|maybe_unit| maybe_unit.cloned())
    }

    async fn player_unit_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Location>> {
        self.game.read().unwrap().player_unit_loc(player_secret, id)
    }

    async fn player_toplevel_unit_by_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Unit>> {
        self.game
            .read()
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
            .move_unit_by_id(player_secret, unit_id, dest)
    }

    // async fn propose_move_unit_by_id(
    //     self,
    //     _: Context,
    //     id: UnitID,
    //     dest: Location,
    // ) -> Proposed<Result<Move, MoveError>> {
    //     self.game.propose_move_unit_by_id(id, dest)
    // }

    async fn move_unit_by_id_avoiding_combat(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.game
            .write()
            .unwrap()
            .move_unit_by_id_avoiding_combat(player_secret, id, dest)
    }

    // async fn propose_move_unit_by_id_avoiding_combat(
    //     self,
    //     _: Context,
    //     id: UnitID,
    //     dest: Location,
    // ) -> Proposed<MoveResult> {
    //     self.game.propose_move_unit_by_id_avoiding_combat(id, dest)
    // }

    async fn disband_unit_by_id(
        self,
        _: Context,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Unit> {
        self.game
            .write()
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
            .clear_production(player_secret, loc, ignore_cleared_production)
    }

    async fn turn(self, _: Context) -> TurnNum {
        self.game.read().unwrap().turn()
    }

    async fn current_player(self, _: Context) -> PlayerNum {
        self.game.read().unwrap().current_player()
    }

    /// The logical dimensions of the game map
    async fn dims(self, _: Context) -> Dims {
        self.game.read().unwrap().dims()
    }

    async fn wrapping(self, _: Context) -> Wrap2d {
        self.game.read().unwrap().wrapping()
    }

    /// Units that could be produced by a city located at the given location
    async fn valid_productions(self, _: Context, loc: Location) -> Vec<UnitType> {
        self.game.read().unwrap().valid_productions(loc).collect()
    }

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    async fn valid_productions_conservative(self, _: Context, loc: Location) -> Vec<UnitType> {
        self.game
            .read()
            .unwrap()
            .valid_productions_conservative(loc)
            .collect()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
            .order_unit_go_to(player_secret, unit_id, dest)
    }

    /// Simulate ordering the specified unit to go to the given location
    // async fn propose_order_unit_go_to(
    //     self,
    //     _: Context,
    //     unit_id: UnitID,
    //     dest: Location,
    // ) -> Proposed<OrdersResult> {
    //     self.game.propose_order_unit_go_to(unit_id, dest)
    // }

    async fn order_unit_explore(
        self,
        _: Context,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.game
            .write()
            .unwrap()
            .order_unit_explore(player_secret, unit_id)
    }

    /// Simulate ordering the specified unit to explore.
    // async fn propose_order_unit_explore(
    //     self,
    //     _: Context,
    //     unit_id: UnitID,
    // ) -> Proposed<OrdersResult> {
    //     self.game.propose_order_unit_explore(unit_id)
    // }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    async fn activate_unit_by_loc(
        self,
        _: Context,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<()> {
        self.game
            .write()
            .unwrap()
            .activate_unit_by_loc(player_secret, loc)
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
    async fn features(self, _: Context, player_secret: PlayerSecret) -> UmpireResult<Vec<fX>> {
        let g = self.game.read().unwrap();
        player_features(&g, player_secret)
    }

    async fn player_score(self, _: Context, player_secret: PlayerSecret) -> UmpireResult<f64> {
        self.game.read().unwrap().player_score(player_secret)
    }

    async fn take_simple_action(
        self,
        _: Context,
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        self.game
            .write()
            .unwrap()
            .take_simple_action(player_secret, action)
    }

    async fn take_action(
        self,
        _: Context,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> Result<PlayerActionOutcome, GameError> {
        self.game
            .write()
            .unwrap()
            .take_action(player_secret, action)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = cli::app("umpired", "fwWH")
        .version(conf::APP_VERSION)
        .author("Josh Hansen <hansen.joshuaa@gmail.com>")
        .about(conf::APP_SUBTITLE)
        .arg(players_arg())
        .get_matches();

    let fog_of_war = matches.get_one::<bool>("fog").unwrap().clone();

    let player_types = matches.get_one::<Vec<PlayerType>>("players").unwrap();

    let num_players: PlayerNum = player_types.len();
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

    let game: Arc<RwLock<Game>> = Arc::new(RwLock::new(game));

    let connection_count = Arc::new(RwLock::new(0usize));

    let server_addr = (IpAddr::V6(Ipv6Addr::LOCALHOST), 21131);

    // JSON transport is provided by the json_transport tarpc module. It makes it easy
    // to start up a serde-powered json serialization strategy over TCP.

    let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Json::default).await?;
    // tracing::info!("Listening on port {}", listener.local_addr().port());
    listener.config_mut().max_frame_length(usize::MAX);
    listener
        // Ignore accept errors.
        .filter_map(|r| future::ready(r.ok()))
        .map(server::BaseChannel::with_defaults)
        // Limit channels to 1 per IP.
        .max_channels_per_key(1, |t| t.transport().peer_addr().unwrap().ip())
        // serve is generated by the service attribute. It takes as input any type implementing
        // the generated World trait.
        .map(|channel| {
            let cc = connection_count.clone();

            let player = *cc.read().unwrap();

            *cc.write().unwrap() += 1;

            let known_secrets: Vec<Option<PlayerSecret>> = secrets
                .iter()
                .enumerate()
                .map(|(i, secret)| if i == player { Some(*secret) } else { None })
                .collect();

            println!("Serving player {}", player);

            let server = UmpireServer {
                game: game.clone(),
                known_secrets,
            };

            println!(
                "Server sees num players: {}",
                server.game.read().unwrap().num_players()
            );

            channel.execute(server.serve())
        })
        // Max channels.
        .buffer_unordered(num_players)
        .for_each(|_| async {})
        .await;

    // let (client_transport, server_transport) = tarpc::transport::channel::unbounded();

    // let server = server::BaseChannel::with_defaults(server_transport);

    // let server_handle = tokio::spawn(server.execute(umpire_player_rpc_server.serve()));

    // WorldClient is generated by the #[tarpc::service] attribute. It has a constructor `new`
    // that takes a config and any Transport as input.
    // let client = UmpirePlayerRpcClient::new(client::Config::default(), client_transport).spawn();

    // The client has an RPC method for each RPC defined in the annotated trait. It takes the same
    // args as defined, with the addition of a Context, which is always the first arg. The Context
    // specifies a deadline and trace information which can be helpful in debugging requests.
    // let hello = client.hello(context::current(), "Stim".to_string()).await?;

    // println!("{hello}");

    // let player_num = client
    //     .player_num(context::current())
    //     .await
    //     .map(|x| x)
    //     .map_err(|e| e)?;

    // println!("player_num: {}", player_num);

    // server_handle.await?;

    Ok(())
}
