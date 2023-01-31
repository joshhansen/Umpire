use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
    time::SystemTime,
};

use clap::Arg;

use common::{
    cli::{self, parse_player_spec, Specified},
    conf,
    game::{
        action::{AiPlayerAction, PlayerAction, PlayerActionOutcome},
        ai::{fX, player_features},
        city::{City, CityID},
        error::GameError,
        map::Tile,
        move_::{Move, MoveError, MoveResult},
        obs::{Obs, ObsTracker},
        proposed::Proposed2,
        unit::{orders::OrdersResult, Unit, UnitID, UnitType},
        Game, PlayerNum, PlayerType, TurnNum, TurnStart,
    },
    name::{city_namer, unit_namer},
    util::{Dims, Direction, Location, Wrap2d},
};
use futures::{
    future::{self, Ready},
    prelude::*,
};
use tarpc::{
    client::{self, RpcError},
    context::{self, Context},
    server::{self, incoming::Incoming, Channel},
};

// This is the service definition. It looks a lot like a trait definition.
// It defines one RPC, hello, which takes one arg, name, and returns a String.
#[tarpc::service]
trait UmpirePlayerRpc {
    /// The number of the player connected over this RPC channel
    async fn player_num() -> PlayerNum;
}

// trait Blah {
//     /// The number of players in the game
//     async fn num_players() -> PlayerNum;

//     async fn turn_is_done() -> bool;

//     /// The victor---if any---meaning the player who has defeated all other players.
//     ///
//     /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
//     /// has won.
//     async fn victor() -> Option<PlayerNum>;

//     async fn current_player_unit_legal_one_step_destinations(
//         unit_id: UnitID,
//     ) -> Result<HashSet<Location>, GameError>;

//     async fn current_player_unit_legal_directions(
//         unit_id: UnitID,
//     ) -> Result<Vec<Direction>, GameError>;

//     /// The current player's most recent observation of the tile at location `loc`, if any
//     async fn current_player_tile(loc: Location) -> Option<Tile>;

//     /// The current player's observation at location `loc`
//     async fn current_player_obs(loc: Location) -> Obs;

//     async fn current_player_observations() -> ObsTracker;

//     /// Every city controlled by the current player
//     async fn current_player_cities() -> Vec<City>;

//     /// All cities controlled by the current player which have a production target set
//     async fn current_player_cities_with_production_target() -> Vec<City>;

//     /// The number of cities controlled by the current player which either have a production target or are NOT set to be ignored when requesting productions to be set
//     ///
//     /// This basically lets us make sure a player doesn't set all their cities' productions to none since right now the UI has no way of getting out of that situation
//     ///
//     /// FIXME Get rid of this and just make the UI smarter
//     #[deprecated]
//     async fn player_cities_producing_or_not_ignored() -> usize;

//     /// Every unit controlled by the current player
//     async fn current_player_units() -> Vec<Unit>;

//     /// If the current player controls a city at location `loc`, return it
//     async fn current_player_city_by_loc(loc: Location) -> Option<City>;

//     /// If the current player controls a city with ID `city_id`, return it
//     async fn current_player_city_by_id(city_id: CityID) -> Option<City>;

//     /// If the current player controls a unit with ID `id`, return it
//     async fn current_player_unit_by_id(id: UnitID) -> Option<Unit>;

//     /// If the current player controls a unit with ID `id`, return its location
//     async fn current_player_unit_loc(id: UnitID) -> Option<Location>;

//     /// If the current player controls the top-level unit at location `loc`, return it
//     async fn current_player_toplevel_unit_by_loc(loc: Location) -> Option<Unit>;

//     async fn production_set_requests() -> Vec<Location>;

//     /// Which if the current player's units need orders?
//     ///
//     /// In other words, which of the current player's units have no orders and have moves remaining?
//     async fn unit_orders_requests() -> Vec<UnitID>;

//     /// Which if the current player's units need orders?
//     ///
//     /// In other words, which of the current player's units have no orders and have moves remaining?
//     async fn units_with_orders_requests() -> Vec<Unit>;

//     async fn units_with_pending_orders() -> Vec<UnitID>;

//     // Movement-related methods

//     async fn move_toplevel_unit_by_id(unit_id: UnitID, dest: Location) -> MoveResult;

//     async fn move_toplevel_unit_by_id_avoiding_combat(
//         unit_id: UnitID,
//         dest: Location,
//     ) -> MoveResult;

//     async fn move_toplevel_unit_by_loc(src: Location, dest: Location) -> MoveResult;

//     async fn move_toplevel_unit_by_loc_avoiding_combat(src: Location, dest: Location)
//         -> MoveResult;

//     async fn move_unit_by_id_in_direction(id: UnitID, direction: Direction) -> MoveResult;

//     async fn move_unit_by_id(unit_id: UnitID, dest: Location) -> MoveResult;

//     // async fn propose_move_unit_by_id(
//     //     id: UnitID,
//     //     dest: Location,
//     // ) -> Proposed2<Result<Move, MoveError>>;

//     async fn move_unit_by_id_avoiding_combat(id: UnitID, dest: Location) -> MoveResult;

//     // async fn propose_move_unit_by_id_avoiding_combat(
//     //     id: UnitID,
//     //     dest: Location,
//     // ) -> Proposed2<MoveResult>;

//     async fn disband_unit_by_id(id: UnitID) -> Result<Unit, GameError>;

//     /// Sets the production of the current player's city at location `loc` to `production`.
//     ///
//     /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
//     async fn set_production_by_loc(
//         loc: Location,
//         production: UnitType,
//     ) -> Result<Option<UnitType>, GameError>;

//     /// Sets the production of the current player's city with ID `city_id` to `production`.
//     ///
//     /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
//     async fn set_production_by_id(
//         city_id: CityID,
//         production: UnitType,
//     ) -> Result<Option<UnitType>, GameError>;

//     //FIXME Restrict to current player cities
//     async fn clear_production_without_ignoring(loc: Location) -> Result<(), String>;

//     //FIXME Restrict to current player cities
//     async fn clear_production_and_ignore(loc: Location) -> Result<(), String>;

//     async fn turn() -> TurnNum;

//     async fn current_player() -> PlayerNum;

//     /// The logical dimensions of the game map
//     async fn dims() -> Dims;

//     async fn wrapping() -> Wrap2d;

//     /// Units that could be produced by a city located at the given location
//     async fn valid_productions(loc: Location) -> Vec<UnitType>;

//     /// Units that could be produced by a city located at the given location, allowing only those which can actually
//     /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
//     async fn valid_productions_conservative(loc: Location) -> Vec<UnitType>;

//     /// If the current player controls a unit with ID `id`, order it to sentry
//     async fn order_unit_sentry(unit_id: UnitID) -> OrdersResult;

//     async fn order_unit_skip(unit_id: UnitID) -> OrdersResult;

//     async fn order_unit_go_to(unit_id: UnitID, dest: Location) -> OrdersResult;

//     /// Simulate ordering the specified unit to go to the given location
//     // async fn propose_order_unit_go_to(unit_id: UnitID, dest: Location) -> Proposed<OrdersResult>;

//     async fn order_unit_explore(unit_id: UnitID) -> OrdersResult;

//     /// Simulate ordering the specified unit to explore.
//     // async fn propose_order_unit_explore(unit_id: UnitID) -> Proposed<OrdersResult>;

//     /// If a unit at the location owned by the current player exists, activate it and any units it carries
//     async fn activate_unit_by_loc(loc: Location) -> Result<(), GameError>;

//     /// Feature vector for use in AI training
//     ///
//     /// Map of the output vector:
//     ///
//     /// # 15: 1d features
//     /// * 1: current turn
//     /// * 1: current player city count
//     /// * 1: number of tiles observed by current player
//     /// * 1: percentage of tiles observed by current player
//     /// * 11: the type of unit being represented, where "city" is also a type of unit (one hot encoded)
//     /// * 10: number of units controlled by current player (infantry, armor, fighters, bombers, transports, destroyers
//     ///                                                     submarines, cruisers, battleships, carriers)
//     /// # 363: 2d features, three layers
//     /// * 121: is_enemy_belligerent (11x11)
//     /// * 121: is_observed (11x11)
//     /// * 121: is_neutral (11x11)
//     ///
//     async fn features() -> Vec<fX>;

//     async fn player_score(player: PlayerNum) -> Result<f64, GameError>;

//     async fn take_simple_action(action: AiPlayerAction) -> Result<PlayerActionOutcome, GameError>;

//     async fn take_action(action: PlayerAction) -> Result<PlayerActionOutcome, GameError>;
// }

// Implementation of the server API
#[derive(Clone)]
struct UmpireServer {
    game: Game,
}

#[tarpc::server]
impl UmpirePlayerRpc for UmpireServer {
    async fn player_num(self, _: Context) -> PlayerNum {
        self.game.current_player()
    }
}

struct Tmp {
    game: Game,
    player: PlayerNum,
}
impl Tmp {
    // Each defined rpc generates two items in the trait, a fn that serves the RPC, and
    // an associated type representing the future output by the fn.

    // type HelloFut = Ready<String>;

    // fn hello(self, _: context::Context, name: String) -> Self::HelloFut {
    //     future::ready(format!("Hello, {name}!"))
    // }

    async fn num_players(self, _: Context) -> PlayerNum {
        self.game.num_players()
    }

    async fn turn_is_done(self, _: Context) -> bool {
        self.game.turn_is_done()
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    async fn victor(self, _: Context) -> Option<PlayerNum> {
        self.game.victor()
    }

    async fn current_player_unit_legal_one_step_destinations(
        self,
        _: Context,
        unit_id: UnitID,
    ) -> Result<HashSet<Location>, GameError> {
        self.game
            .current_player_unit_legal_one_step_destinations(unit_id)
    }

    async fn current_player_unit_legal_directions(
        self,
        _: Context,
        unit_id: UnitID,
    ) -> Result<Vec<Direction>, GameError> {
        self.game
            .current_player_unit_legal_directions(unit_id)
            .map(|d| d.collect())
    }

    /// The current player's most recent observation of the tile at location `loc`, if any
    async fn current_player_tile(self, _: Context, loc: Location) -> Option<Tile> {
        self.game.current_player_tile(loc).cloned()
    }

    /// The current player's observation at location `loc`
    async fn current_player_obs(self, _: Context, loc: Location) -> Obs {
        self.game.current_player_obs(loc).clone()
    }

    async fn current_player_observations(self, _: Context) -> ObsTracker {
        self.game.current_player_observations().clone()
    }

    /// Every city controlled by the current player
    async fn current_player_cities(self, _: Context) -> Vec<City> {
        self.game.current_player_cities().cloned().collect()
    }

    /// All cities controlled by the current player which have a production target set
    async fn current_player_cities_with_production_target(self, _: Context) -> Vec<City> {
        self.game
            .current_player_cities_with_production_target()
            .cloned()
            .collect()
    }

    /// The number of cities controlled by the current player which either have a production target or are NOT set to be ignored when requesting productions to be set
    ///
    /// This basically lets us make sure a player doesn't set all their cities' productions to none since right now the UI has no way of getting out of that situation
    ///
    /// FIXME Get rid of this and just make the UI smarter
    async fn player_cities_producing_or_not_ignored(self, _: Context) -> usize {
        self.game.player_cities_producing_or_not_ignored()
    }

    /// Every unit controlled by the current player
    async fn current_player_units(self, _: Context) -> Vec<Unit> {
        self.game.current_player_units().cloned().collect()
    }

    /// If the current player controls a city at location `loc`, return it
    async fn current_player_city_by_loc(self, _: Context, loc: Location) -> Option<City> {
        self.game.current_player_city_by_loc(loc).cloned()
    }

    /// If the current player controls a city with ID `city_id`, return it
    async fn current_player_city_by_id(self, _: Context, city_id: CityID) -> Option<City> {
        self.game.current_player_city_by_id(city_id).cloned()
    }

    /// If the current player controls a unit with ID `id`, return it
    async fn current_player_unit_by_id(self, _: Context, id: UnitID) -> Option<Unit> {
        self.game.current_player_unit_by_id(id).cloned()
    }

    /// If the current player controls a unit with ID `id`, return its location
    async fn current_player_unit_loc(self, _: Context, id: UnitID) -> Option<Location> {
        self.game.current_player_unit_loc(id)
    }

    /// If the current player controls the top-level unit at location `loc`, return it
    async fn current_player_toplevel_unit_by_loc(self, _: Context, loc: Location) -> Option<Unit> {
        self.game.current_player_toplevel_unit_by_loc(loc).cloned()
    }

    async fn production_set_requests(self, _: Context) -> Vec<Location> {
        self.game.production_set_requests().collect()
    }

    /// Which if the current player's units need orders?
    ///
    /// In other words, which of the current player's units have no orders and have moves remaining?
    async fn unit_orders_requests(self, _: Context) -> Vec<UnitID> {
        self.game.unit_orders_requests().collect()
    }

    /// Which if the current player's units need orders?
    ///
    /// In other words, which of the current player's units have no orders and have moves remaining?
    async fn units_with_orders_requests(self, _: Context) -> Vec<Unit> {
        self.game.units_with_orders_requests().cloned().collect()
    }

    async fn units_with_pending_orders(self, _: Context) -> Vec<UnitID> {
        self.game.units_with_pending_orders().collect()
    }

    // Movement-related methods

    async fn move_toplevel_unit_by_id(
        mut self,
        _: Context,
        unit_id: UnitID,
        dest: Location,
    ) -> MoveResult {
        self.game.move_toplevel_unit_by_id(unit_id, dest)
    }

    async fn move_toplevel_unit_by_id_avoiding_combat(
        mut self,
        _: Context,
        unit_id: UnitID,
        dest: Location,
    ) -> MoveResult {
        self.game
            .move_toplevel_unit_by_id_avoiding_combat(unit_id, dest)
    }

    async fn move_toplevel_unit_by_loc(
        mut self,
        _: Context,
        src: Location,
        dest: Location,
    ) -> MoveResult {
        self.game.move_toplevel_unit_by_loc(src, dest)
    }

    async fn move_toplevel_unit_by_loc_avoiding_combat(
        mut self,
        _: Context,
        src: Location,
        dest: Location,
    ) -> MoveResult {
        self.game
            .move_toplevel_unit_by_loc_avoiding_combat(src, dest)
    }

    async fn move_unit_by_id_in_direction(
        mut self,
        _: Context,
        id: UnitID,
        direction: Direction,
    ) -> MoveResult {
        self.game.move_unit_by_id_in_direction(id, direction)
    }

    async fn move_unit_by_id(mut self, _: Context, unit_id: UnitID, dest: Location) -> MoveResult {
        self.game.move_unit_by_id(unit_id, dest)
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
        mut self,
        _: Context,
        id: UnitID,
        dest: Location,
    ) -> MoveResult {
        self.game.move_unit_by_id_avoiding_combat(id, dest)
    }

    // async fn propose_move_unit_by_id_avoiding_combat(
    //     self,
    //     _: Context,
    //     id: UnitID,
    //     dest: Location,
    // ) -> Proposed<MoveResult> {
    //     self.game.propose_move_unit_by_id_avoiding_combat(id, dest)
    // }

    async fn disband_unit_by_id(mut self, _: Context, id: UnitID) -> Result<Unit, GameError> {
        self.game.disband_unit_by_id(id)
    }

    /// Sets the production of the current player's city at location `loc` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    async fn set_production_by_loc(
        mut self,
        _: Context,
        loc: Location,
        production: UnitType,
    ) -> Result<Option<UnitType>, GameError> {
        self.game.set_production_by_loc(loc, production)
    }

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    async fn set_production_by_id(
        mut self,
        _: Context,
        city_id: CityID,
        production: UnitType,
    ) -> Result<Option<UnitType>, GameError> {
        self.game.set_production_by_id(city_id, production)
    }

    //FIXME Restrict to current player cities
    async fn clear_production_without_ignoring(
        mut self,
        _: Context,
        loc: Location,
    ) -> Result<(), String> {
        self.game.clear_production_without_ignoring(loc)
    }

    //FIXME Restrict to current player cities
    async fn clear_production_and_ignore(
        mut self,
        _: Context,
        loc: Location,
    ) -> Result<(), String> {
        self.game.clear_production_and_ignore(loc)
    }

    async fn turn(self, _: Context) -> TurnNum {
        self.game.turn()
    }

    async fn current_player(self, _: Context) -> PlayerNum {
        self.game.current_player()
    }

    /// The logical dimensions of the game map
    async fn dims(self, _: Context) -> Dims {
        self.game.dims()
    }

    async fn wrapping(self, _: Context) -> Wrap2d {
        self.game.wrapping()
    }

    /// Units that could be produced by a city located at the given location
    async fn valid_productions(self, _: Context, loc: Location) -> Vec<UnitType> {
        self.game.valid_productions(loc).collect()
    }

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    async fn valid_productions_conservative(self, _: Context, loc: Location) -> Vec<UnitType> {
        self.game.valid_productions_conservative(loc).collect()
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    async fn order_unit_sentry(mut self, _: Context, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_sentry(unit_id)
    }

    async fn order_unit_skip(mut self, _: Context, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_skip(unit_id)
    }

    async fn order_unit_go_to(
        mut self,
        _: Context,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult {
        self.game.order_unit_go_to(unit_id, dest)
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

    async fn order_unit_explore(mut self, _: Context, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_explore(unit_id)
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
    async fn activate_unit_by_loc(mut self, _: Context, loc: Location) -> Result<(), GameError> {
        self.game.activate_unit_by_loc(loc)
    }

    // async fn propose_end_turn(self, _: Context) -> (Game, Result<TurnStart, PlayerNum>) {
    //     let mut game = self.game.clone();
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
    async fn features(self, _: Context) -> Vec<fX> {
        player_features(&self.game, self.game.current_player())
    }

    async fn player_score(self, _: Context, player: PlayerNum) -> Result<f64, GameError> {
        self.game.player_score(player)
    }

    async fn take_action(
        mut self,
        _: Context,
        action: AiPlayerAction,
    ) -> Result<PlayerActionOutcome, GameError> {
        self.game.take_simple_action(action)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = cli::app(format!("{}d", conf::APP_NAME), "fwWH")
        .version(conf::APP_VERSION)
        .author("Josh Hansen <hansen.joshuaa@gmail.com>")
        .about(conf::APP_SUBTITLE)
        .arg(
            Arg::with_name("players")
                .short("p")
                .long("players")
                .takes_value(true)
                .required(true)
                .default_value("h1233")
                .help(
                    format!(
                        "Player type specification string, {}",
                        PlayerType::values()
                            .iter()
                            .map(|player_type| format!(
                                "'{}' for {}",
                                player_type.spec(),
                                player_type.desc()
                            ))
                            .collect::<Vec<String>>()
                            .join(", ")
                    )
                    .as_str(),
                )
                .validator(|s| {
                    parse_player_spec(s.as_str()).map(|_| ())
                    // for spec_char in s.chars() {
                    //     PlayerType::from_spec_char(spec_char)
                    //     .map(|_| ())
                    //     .map_err(|_| format!("'{}' is not a valid player type", spec_char))?;
                    // }
                    // Ok(())
                }),
        )
        .get_matches();

    let fog_of_war = matches.value_of("fog").unwrap() == "on";
    // let player_types: Vec<PlayerType> = matches.value_of("players").unwrap()
    //     .chars()
    //     .map(|spec_char| {
    //         PlayerType::from_spec_char(spec_char)
    //                     .expect(format!("'{}' is not a valid player type", spec_char).as_str())
    //     })
    //     .collect()
    // ;

    let player_types: Vec<PlayerType> =
        parse_player_spec(matches.value_of("players").unwrap()).unwrap();

    let num_players: PlayerNum = player_types.len();
    let map_width: u16 = matches.value_of("map_width").unwrap().parse().unwrap();
    let map_height: u16 = matches.value_of("map_height").unwrap().parse().unwrap();
    let wrapping = Wrap2d::try_from(matches.value_of("wrapping").unwrap().as_ref()).unwrap();

    let map_dims: Dims = Dims::new(map_width, map_height);
    if (map_dims.area() as PlayerNum) < num_players {
        panic!("Map dimensions of {} give an area of {} which is not enough room for {} players; area of {} or greater required.",
        map_dims, map_dims.area(), num_players, num_players);
    }

    // let start_time = SystemTime::now();

    let city_namer = city_namer();
    let unit_namer = unit_namer();

    let (client_transport, server_transport) = tarpc::transport::channel::unbounded();

    let server = server::BaseChannel::with_defaults(server_transport);

    let game = Game::new(
        map_dims,
        city_namer,
        num_players,
        fog_of_war,
        Some(Arc::new(RwLock::new(unit_namer))),
        wrapping,
    );

    let umpire_player_rpc_server = UmpireServer { game };

    tokio::spawn(server.execute(umpire_player_rpc_server.serve()));

    // WorldClient is generated by the #[tarpc::service] attribute. It has a constructor `new`
    // that takes a config and any Transport as input.
    let client = UmpirePlayerRpcClient::new(client::Config::default(), client_transport).spawn();

    // The client has an RPC method for each RPC defined in the annotated trait. It takes the same
    // args as defined, with the addition of a Context, which is always the first arg. The Context
    // specifies a deadline and trace information which can be helpful in debugging requests.
    // let hello = client.hello(context::current(), "Stim".to_string()).await?;

    // println!("{hello}");

    let player_num = client
        .player_num(context::current())
        .await
        .map(|x| x)
        .map_err(|e| e)?;

    println!("player_num: {}", player_num);

    Ok(())
}
