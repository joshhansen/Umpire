//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

pub mod action;
pub mod ai;
pub mod alignment;
pub mod city;
pub mod combat;
pub mod error;
pub mod map;
pub mod move_;
pub mod obs;
pub mod player;
pub mod proposed;
pub mod unit;

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;

use rsrl::DerefVec;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    game::{
        action::AiPlayerAction,
        alignment::Alignment,
        city::{City, CityID},
        combat::CombatCapable,
        error::GameError,
        map::{
            dijkstra::{
                self, directions_unit_could_move_iter, neighbors_terrain_only,
                neighbors_unit_could_move_to_iter, AndFilter, Filter, NoCitiesButOursFilter,
                NoUnitsFilter, ShortestPaths, Source, UnitMovementFilter,
                UnitMovementFilterXenophile,
            },
            gen::generate_map,
            LocationGridI, MapData, NewUnitError, Tile,
        },
        obs::{Obs, ObsTracker, Observer, PlayerObsTracker},
        unit::{
            orders::{Orders, OrdersOutcome, OrdersResult, OrdersStatus},
            Unit, UnitID, UnitType,
        },
    },
    name::{IntNamer, Namer},
    util::{Dimensioned, Dims, Direction, Location, Vec2d, Wrap2d},
};

pub use self::player::{PlayerNum, PlayerTurnControl, PlayerType};

use self::{
    action::{PlayerAction, PlayerActionOutcome},
    ai::{fX, FEATS_LEN},
    alignment::{Aligned, AlignedMaybe},
    move_::{Move, MoveComponent, MoveError},
    proposed::Proposed2,
};

static UNIT_TYPES: [UnitType; 10] = UnitType::values();

/// How important is a city in and of itself?
const CITY_INTRINSIC_SCORE: f64 = 1000.0;

/// How valuable is it to have observed a tile at all?
const TILE_OBSERVED_BASE_SCORE: f64 = 10.0;

/// How much is each turn taken penalized?
const TURN_PENALTY: f64 = 0.0; //1000.0;

/// How much is each action penalized?
const ACTION_PENALTY: f64 = 100.0;

/// How much is each point of controlled unit production cost (downweighted for reduced HP) worth?
const UNIT_MULTIPLIER: f64 = 100.0;

/// How much is victory worth?
const VICTORY_SCORE: f64 = 1_000_000.0;

pub type PlayerSecret = Uuid;

pub type TurnNum = u32;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct TurnStart {
    pub turn: TurnNum,
    pub current_player: PlayerNum,
    pub orders_results: Vec<OrdersResult>,
    pub production_outcomes: Vec<UnitProductionOutcome>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum UnitProductionOutcome {
    UnitProduced {
        /// A copy of the unit that was produced
        unit: Unit,

        /// A copy of the city that did the producing
        city: City,
    },

    UnitAlreadyPresent {
        /// A copy of the unit that's already present
        prior_unit: Unit,

        /// The type of unit the city is trying to produce
        unit_type_under_production: UnitType,

        /// A copy of the city that would have produced the unit if a unit weren't already present
        city: City,
    },
}

pub type UmpireResult<T> = Result<T, GameError>;

pub type ProposedResult<Outcome, E> = Result<Proposed2<Outcome>, E>;

pub type ProposedUmpireResult<T> = UmpireResult<Proposed2<T>>;

pub type ProposedActionResult = ProposedUmpireResult<PlayerActionOutcome>;

pub type ProposedOrdersResult = ProposedResult<OrdersOutcome, GameError>; //TODO Make error type orders-specific

#[async_trait]
pub trait IGame: Send + Sync {
    async fn num_players(&self) -> PlayerNum;

    async fn player_turn_control<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)>;

    async fn player_turn_control_clearing<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)>;

    async fn player_turn_control_nonending<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)>;

    async fn is_player_turn(&self, secret: PlayerSecret) -> UmpireResult<bool>;

    async fn begin_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<TurnStart>;

    /// Begin the turn of the specified player, claring productions
    async fn begin_turn_clearing(&mut self, player_secret: PlayerSecret)
        -> UmpireResult<TurnStart>;

    /// Indicates whether the given player has completed the specified turn, or not
    ///
    /// This is public information.
    async fn turn_is_done(&self, player: PlayerNum, turn: TurnNum) -> UmpireResult<bool>;

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
    ) -> UmpireResult<()>;

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

    async fn player_score(&self, player_secret: PlayerSecret) -> UmpireResult<f64>;

    async fn player_score_by_idx(&self, player: PlayerNum) -> UmpireResult<f64>;

    /// Each player's current score, indexed by player number
    async fn player_scores(&self) -> Vec<f64>;

    async fn take_simple_action(
        &mut self,
        player_secret: PlayerSecret,
        action: AiPlayerAction,
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

    // async fn current_player_production_set_requests(&self) -> Vec<Location>;

    // async fn current_player_valid_productions_conservative(&self, loc: Location) -> Vec<UnitType>;

    // async fn current_player_unit_orders_requests(&self) -> Vec<UnitID>;

    // async fn current_player_unit_legal_directions(
    //     &self,
    //     unit_id: UnitID,
    // ) -> UmpireResult<Vec<Direction>>;

    /// This is an escape hatch for AI training; do NOT expose this via UmpireRpcClient
    fn clone_underlying_game_state(&self) -> Result<Game, String>;
}

/// The core engine that enforces Umpire's game rules
#[derive(Clone)]
pub struct Game {
    /// The underlying state of the game
    map: MapData,

    player_observations: PlayerObsTracker,

    /// The turn that it is right now
    turn: TurnNum,

    /// The number of players the game is set up for
    num_players: PlayerNum,

    /// As players register, they're given secrets which we track here
    player_secrets: Vec<Uuid>,

    /// The player that is currently the player right now
    current_player: PlayerNum,

    /// The wrapping policy for the game---can you loop around the map vertically, horizontally, or both?
    wrapping: Wrap2d,

    /// A name generator to give names to units
    unit_namer: Arc<RwLock<dyn Namer>>,

    /// Whether players have full information about the map, or have their knowledge obscured by the "fog of war".
    fog_of_war: bool,

    /// Action counts
    ///
    /// How many actions has each player taken? Used for score calculation.
    ///
    /// An action is basically every city production request and unit orders request taken.
    action_counts: Vec<u64>,

    /// The total hitpoints of all enemy units defeated by each player
    ///
    /// Stored for use in the score calculation.
    defeated_unit_hitpoints: Vec<u64>,
}
impl Game {
    /// Creates a new game instance
    ///
    /// The Game that is returned will already have begun with the first player's turn.
    ///
    /// A map with the specified dimensions will be generated. City names are taken from `city_namer`
    ///
    /// If `fog_of_war` is `true` then players' view of the map will be limited to what they have previously
    /// observed, with observations growing stale over time.
    ///
    /// Also returns the player secrets used for access control
    pub fn new<N: Namer>(
        map_dims: Dims,
        mut city_namer: N,
        num_players: PlayerNum,
        fog_of_war: bool,
        unit_namer: Option<Arc<RwLock<dyn Namer>>>,
        wrapping: Wrap2d,
    ) -> (Self, Vec<PlayerSecret>) {
        let map = generate_map(&mut city_namer, map_dims, num_players);
        Self::new_with_map(map, num_players, fog_of_war, unit_namer, wrapping)
    }

    /// Creates a new game instance from a pre-generated map
    ///
    /// Also returns the player secrets used for access control
    pub fn new_with_map(
        map: MapData,
        num_players: PlayerNum,
        fog_of_war: bool,
        unit_namer: Option<Arc<RwLock<dyn Namer>>>,
        wrapping: Wrap2d,
    ) -> (Self, Vec<PlayerSecret>) {
        let player_observations = PlayerObsTracker::new(num_players, map.dims());

        let mut game = Self {
            map,
            player_observations,
            turn: 0,
            num_players,
            player_secrets: Vec::new(),
            current_player: 0,
            wrapping,
            unit_namer: unit_namer.unwrap_or(Arc::new(RwLock::new(IntNamer::new("unit")))),
            fog_of_war,
            action_counts: vec![0; num_players],
            defeated_unit_hitpoints: vec![0; num_players],
        };

        let secrets: Vec<PlayerSecret> = (0..num_players)
            .map(|_player| game.register_player().unwrap())
            .collect();

        game.begin_turn(secrets[0]).unwrap();
        (game, secrets)
    }

    pub fn num_players(&self) -> PlayerNum {
        self.num_players
    }

    pub fn player_turn_control<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        PlayerTurnControl::new_sync(self, secret)
    }

    pub fn player_turn_control_clearing<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        PlayerTurnControl::new_sync_clearing(self, secret)
    }

    pub fn player_turn_control_nonending<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        PlayerTurnControl::new_sync_nonending(self, secret)
    }

    /// Register a player and get its secret
    ///
    /// The secret is used for access control on other methods
    ///
    /// Errors if all player slots are currently filled
    fn register_player(&mut self) -> Result<Uuid, GameError> {
        if self.player_secrets.len() == self.num_players {
            Err(GameError::NoPlayerSlotsAvailable)
        } else {
            let secret = Uuid::new_v4();
            self.player_secrets.push(secret);
            Ok(secret)
        }
    }

    fn validate_player_num(&self, player: PlayerNum) -> UmpireResult<()> {
        if player >= self.num_players {
            Err(GameError::NoSuchPlayer { player })
        } else {
            Ok(())
        }
    }

    pub fn is_player_turn(&self, secret: PlayerSecret) -> UmpireResult<bool> {
        self.player_with_secret(secret)
            .map(|player| player == self.current_player)
    }

    /// Ensure that it is the specified player's turn
    ///
    /// Returns the PlayerNum on success
    fn validate_is_player_turn(&self, secret: PlayerSecret) -> UmpireResult<PlayerNum> {
        let player = self.player_with_secret(secret)?;

        if player != self.current_player {
            Err(GameError::NotPlayersTurn { player })
        } else {
            Ok(player)
        }
    }

    fn produce_units(
        &mut self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitProductionOutcome>> {
        // let max_unit_cost: u16 = UnitType::values().iter().map(|ut| ut.cost()).max().unwrap();

        // for city in self.current_player_cities_with_production_target_mut() {
        //     // We cap the production progress since, in weird circumstances such as a city having a unit blocking its
        //     // production for a very long time, the production progress adds can overflow
        //     if city.production_progress < max_unit_cost {
        //         city.production_progress += 1;
        //     }
        // }

        let player = self.validate_is_player_turn(player_secret)?;

        self.map.increment_player_city_production_targets(player);

        let producing_city_locs: Vec<Location> = self
            .player_cities_with_production_target(player_secret)?
            .filter(|city| {
                let unit_under_production = city.production().unwrap();

                city.production_progress >= unit_under_production.cost()
            })
            .map(|city| city.loc)
            .collect();

        Ok(producing_city_locs
            .iter()
            .cloned()
            .map(|city_loc| {
                let (city_loc, city_alignment, unit_under_production) = {
                    let city = self.map.city_by_loc(city_loc).unwrap();
                    let unit_under_production = city.production().unwrap();
                    (city.loc, city.alignment, unit_under_production)
                };

                let name = {
                    let mut namer = self.unit_namer.write().unwrap();
                    namer.name()
                };

                // Attempt to create the new unit

                let result =
                    self.map
                        .new_unit(city_loc, unit_under_production, city_alignment, name);

                match result {
                    Ok(_new_unit_id) => {
                        // We know the unit will be at top-level because that's where freshly-minted units go

                        // let city = self.map.city_by_loc_mut(city_loc).unwrap();
                        // city.production_progress = 0;

                        self.map
                            .clear_city_production_progress_by_loc(city_loc)
                            .unwrap();
                        let city = self.map.city_by_loc(city_loc).unwrap().clone();

                        // let city = city.clone();
                        let unit = self.map.toplevel_unit_by_loc(city_loc).unwrap().clone();

                        UnitProductionOutcome::UnitProduced { city, unit }
                    }
                    Err(err) => match err {
                        NewUnitError::UnitAlreadyPresent {
                            prior_unit,
                            unit_type_under_production,
                            ..
                        } => {
                            let city = self.map.city_by_loc(city_loc).unwrap();

                            UnitProductionOutcome::UnitAlreadyPresent {
                                prior_unit,
                                unit_type_under_production,
                                city: city.clone(),
                            }
                        }
                        err => {
                            panic!("Error creating unit: {}", err)
                        }
                    },
                }
            })
            .collect())
    }

    fn refresh_moves_remaining(&mut self, player_secret: PlayerSecret) -> UmpireResult<()> {
        let player = self.validate_is_player_turn(player_secret)?;
        Ok(self.map.refresh_player_unit_moves_remaining(player))
    }

    /// Mark for accounting purposes that the current player took an action
    fn action_taken(&mut self) {
        self.action_counts[self.current_player] += 1;
    }

    /// Mark for accounting purposes that the current player defeated an enemy unit with
    /// the specified maximum number of hitpoints
    fn unit_defeated(&mut self, max_hp: u16) {
        self.defeated_unit_hitpoints[self.current_player] += max_hp as u64;
    }

    pub fn begin_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<TurnStart> {
        let player = self.validate_is_player_turn(player_secret)?;

        let production_outcomes = self.produce_units(player_secret)?;

        self.refresh_moves_remaining(player_secret)?;

        self.update_player_observations(player);

        let orders_results = self.follow_pending_orders(player_secret)?;

        Ok(TurnStart {
            turn: self.turn,
            current_player: self.current_player,
            orders_results,
            production_outcomes,
        })
    }

    /// Begin the turn of the specified player, claring productions
    pub fn begin_turn_clearing(&mut self, player_secret: PlayerSecret) -> UmpireResult<TurnStart> {
        let result = self.begin_turn(player_secret)?;

        let current_player_secret = self.player_secrets[self.current_player];

        for prod in result.production_outcomes.iter() {
            if let UnitProductionOutcome::UnitProduced { city, .. } = prod {
                self.clear_production(current_player_secret, city.loc, false)
                    .unwrap();
            }
        }

        Ok(result)
    }

    /// Indicates whether the given player has completed the specified turn, or not
    ///
    /// This is public information.
    pub fn turn_is_done(&self, player: PlayerNum, turn: TurnNum) -> UmpireResult<bool> {
        self.validate_player_num(player)?;

        if self.turn > turn {
            return Ok(true);
        }

        if self.turn < turn {
            return Ok(false);
        }

        // self.turn == turn

        if self.current_player > player {
            return Ok(true);
        }

        if self.current_player < player {
            return Ok(false);
        }

        // self.current_player == player
        // In this case the turn is considered done if there are no production or orders requests remaining
        self.player_production_set_requests_by_idx(player)
            .map(|mut rqsts| {
                rqsts.next().is_none()
                    && self.current_player_unit_orders_requests().next().is_none()
            })
    }

    pub fn current_turn_is_done(&self) -> bool {
        self.turn_is_done(self.current_player, self.turn).unwrap()
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    ///
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    ///
    /// Defeat is defined as having no cities and having no units that can capture cities
    pub fn victor(&self) -> Option<PlayerNum> {
        let mut represented: HashSet<PlayerNum> = HashSet::new();

        for city in self.map.cities() {
            if let Alignment::Belligerent { player } = city.alignment {
                represented.insert(player);
            }
            if represented.len() > 1 {
                return None;
            }
        }

        for unit in self.map.units() {
            if let Alignment::Belligerent { player } = unit.alignment {
                if unit.type_.can_occupy_cities() {
                    represented.insert(player);
                }
            }
            if represented.len() > 1 {
                return None;
            }
        }

        if represented.len() == 1 {
            return Some(*represented.iter().next().unwrap()); // unwrap to assert something's there
        }

        None
    }

    /// Ends the turn but doesn't check if requests are completed
    pub fn force_end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<()> {
        self.validate_is_player_turn(player_secret)?;

        self.player_observations_mut(player_secret)?.archive();

        self._inc_current_player();

        Ok(())
    }

    pub fn end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<()> {
        self.validate_is_player_turn(player_secret)?;

        if self.current_turn_is_done() {
            self.force_end_turn(player_secret)
        } else {
            Err(GameError::TurnEndRequirementsNotMet {
                player: self.current_player,
            })
        }
    }

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
    pub fn end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.validate_is_player_turn(player_secret)?;

        if self.current_turn_is_done() {
            Ok(self.force_end_then_begin_turn(player_secret, next_player_secret)?)
        } else {
            Err(GameError::TurnEndRequirementsNotMet {
                player: self.current_player,
            })
        }
    }

    pub fn end_then_begin_turn_clearing(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.validate_is_player_turn(player_secret)?;

        if self.current_turn_is_done() {
            self.force_end_then_begin_turn_clearing(player_secret, next_player_secret)
        } else {
            Err(GameError::TurnEndRequirementsNotMet {
                player: self.current_player,
            })
        }
    }

    fn _inc_current_player(&mut self) {
        self.current_player = (self.current_player + 1) % self.num_players;
        if self.current_player == 0 {
            self.turn += 1;
        }
    }

    /// End the turn without checking that the player has filled all production and orders requests.
    pub fn force_end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.force_end_turn(player_secret)?;

        self.begin_turn(next_player_secret)
    }

    /// End the turn without checking that the player has filled all production and orders requests.
    pub fn force_end_then_begin_turn_clearing(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.force_end_turn(player_secret)?;

        self.begin_turn_clearing(next_player_secret)
    }

    /// Register the current observations of player units
    ///
    /// This applies only to top-level units. Carried units (e.g. units in a transport or carrier) make no observations
    fn update_player_observations(&mut self, player: PlayerNum) {
        let obs_tracker = self.player_observations.tracker_mut(player).unwrap();

        if self.fog_of_war {
            for city in self.map.player_cities(player) {
                city.observe(&self.map, self.turn, self.wrapping, obs_tracker);
            }

            for unit in self.map.player_units(player) {
                unit.observe(&self.map, self.turn, self.wrapping, obs_tracker);
            }
        } else {
            //FIXME when fog of war is disabled we shouldn't need to track observations at all
            for loc in self.map.dims().iter_locs() {
                let tile = self.map.tile(loc).unwrap();
                obs_tracker.track_observation(loc, tile, self.turn);
            }
        }
    }

    /// The set of destinations that the specified unit could actually attempt a move onto in exactly one movement step.
    /// This excludes the unit's original location
    pub fn player_unit_legal_one_step_destinations(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<HashSet<Location>> {
        let unit = self
            .player_unit_by_id(player_secret, unit_id)?
            .ok_or_else(|| GameError::NoSuchUnit { id: unit_id })?;

        Ok(
            neighbors_unit_could_move_to_iter(&self.map, &unit, self.wrapping)
                .filter(|loc| *loc != unit.loc) // exclude the source location; needed because UnitMovementFilter inside of
                .collect(), // neighbors_unit_could_move_to_iter would allow a carried unit to "move"
                            // onto the carrier unit over again if it additional carrying space, thus
                            // resulting in zero-length moves
        )
    }

    pub fn current_player_unit_legal_directions<'a>(
        &'a self,
        unit_id: UnitID,
    ) -> UmpireResult<impl Iterator<Item = Direction> + 'a> {
        let player_secret = self.player_secrets[self.current_player];
        self.player_unit_legal_directions(player_secret, unit_id)
    }

    pub fn player_unit_legal_directions<'a>(
        &'a self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<impl Iterator<Item = Direction> + 'a> {
        let unit = self
            .player_unit_by_id(player_secret, unit_id)?
            .ok_or_else(|| GameError::NoSuchUnit { id: unit_id })?;

        Ok(directions_unit_could_move_iter(
            &self.map,
            &unit,
            self.wrapping,
        ))
    }

    /// The current player's most recent observation of the tile at location `loc`, if any
    pub fn player_tile(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<&Tile>> {
        self.player_with_secret(player_secret)
            .map(|player| self.player_tile_by_idx(player, loc))
    }

    fn current_player_tile(&self, loc: Location) -> Option<&Tile> {
        self.player_tile_by_idx(self.current_player, loc)
    }

    fn player_tile_by_idx(&self, player: PlayerNum, loc: Location) -> Option<&Tile> {
        if let Obs::Observed { tile, .. } = self.player_obs_by_idx(player, loc) {
            Some(tile)
        } else {
            None
        }
    }

    /// The current player's observation at location `loc`
    pub fn player_obs(&self, player_secret: PlayerSecret, loc: Location) -> UmpireResult<&Obs> {
        self.player_with_secret(player_secret)
            .map(|player| self.player_obs_by_idx(player, loc))
    }

    fn player_obs_by_idx(&self, player: PlayerNum, loc: Location) -> &Obs {
        self.player_observations_by_idx(player).get(loc)
    }

    fn current_player_observations(&self) -> &ObsTracker {
        let secret = self.player_secrets[self.current_player];
        self.player_observations(secret).unwrap()
    }

    fn current_player_obs(&self, loc: Location) -> &Obs {
        self.player_observations
            .tracker(self.current_player)
            .unwrap()
            .get(loc)
    }

    pub fn player_observations(&self, player_secret: PlayerSecret) -> UmpireResult<&ObsTracker> {
        self.player_with_secret(player_secret)
            .map(|player| self.player_observations_by_idx(player))
    }

    fn player_observations_by_idx(&self, player: PlayerNum) -> &ObsTracker {
        self.player_observations.tracker(player).unwrap()
    }

    pub fn player_observations_mut(
        &mut self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<&mut ObsTracker> {
        self.player_with_secret(player_secret)
            .map(|player| self.player_observations_by_idx_mut(player))
    }

    fn player_observations_by_idx_mut(&mut self, player: PlayerNum) -> &mut ObsTracker {
        self.player_observations.tracker_mut(player).unwrap()
    }

    /// FIXME Make this private
    /// NOTE: Don't include this in the RPC API - could allow searches for secrets, however improbable of success
    ///
    /// ## Errors
    /// * GameError::NoPlayerIdentifiedBySecret
    pub fn player_with_secret(&self, player_secret: PlayerSecret) -> UmpireResult<PlayerNum> {
        self.player_secrets
            .iter()
            .position(|ps| *ps == player_secret)
            .ok_or(GameError::NoPlayerIdentifiedBySecret)
    }

    /// Every city controlled by the player whose secret is provided
    pub fn player_cities(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<impl Iterator<Item = &City>> {
        let player = self.player_with_secret(player_secret)?;

        self.player_cities_by_idx(player)
    }

    fn player_cities_by_idx(&self, player: PlayerNum) -> UmpireResult<impl Iterator<Item = &City>> {
        self.validate_player_num(player)
            .map(|_| self.map.player_cities(player))
    }

    pub fn player_cities_with_production_target(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<impl Iterator<Item = &City>> {
        self.player_with_secret(player_secret)
            .map(|player| self.map.player_cities_with_production_target(player))
    }

    /// How many cities does the specified player control?
    pub fn player_city_count(&self, player_secret: PlayerSecret) -> UmpireResult<usize> {
        self.player_with_secret(player_secret)
            .map(|player| self.map.player_city_count(player).unwrap_or_default())
    }

    /// The number of cities controlled by the current player which either have a production target or
    /// are NOT set to be ignored when requesting productions to be set
    ///
    /// This basically lets us make sure a player doesn't set all their cities' productions to none since
    /// right now the UI has no way of getting out of that situation
    ///
    /// NOTE Maybe we could make the UI smarter and get rid of this?
    pub fn player_cities_producing_or_not_ignored(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize> {
        self.player_cities(player_secret).map(|cities| {
            cities
                .filter(|city| city.production().is_some() || !city.ignore_cleared_production())
                .count()
        })
    }

    pub fn player_units(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<impl Iterator<Item = &Unit>> {
        self.player_with_secret(player_secret)
            .map(|player| self.player_units_by_idx(player))
    }

    fn player_units_by_idx(&self, player: PlayerNum) -> impl Iterator<Item = &Unit> {
        self.map.player_units(player)
    }

    /// Every unit controlled by the current player
    #[cfg(test)]
    fn current_player_units(&self) -> impl Iterator<Item = &Unit> {
        self.player_units_by_idx(self.current_player())
    }

    /// The counts of unit types controlled by the specified player
    pub fn player_unit_type_counts(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<&HashMap<UnitType, usize>> {
        let player = self.player_with_secret(player_secret)?;
        self.map.player_unit_type_counts(player)
    }

    // /// Every enemy unit known to the current player (as of most recent observations)
    // fn current_player_observed_enemy_units<'a>(&'a self) -> impl Iterator<Item = &Unit> + 'a {
    //     let current_player = self.current_player();
    //     self.current_player_observations()
    //         .iter()
    //         .filter_map(move |obs| match obs {
    //             Obs::Observed { tile, .. } => {
    //                 if let Some(ref unit) = tile.unit {
    //                     if let Alignment::Belligerent { player } = unit.alignment {
    //                         if player != current_player {
    //                             Some(unit)
    //                         } else {
    //                             None
    //                         }
    //                     } else {
    //                         None
    //                     }
    //                 } else {
    //                     None
    //                 }
    //             }
    //             _ => None,
    //         })
    //     // self.map.units().filter(move |unit| unit.alignment != Alignment::Belligerent{player:current_player})
    // }

    /// If the specified player controls a city at location `loc`, return it
    pub fn player_city_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<&City>> {
        self.player_tile(player_secret, loc)
            .map(|tile| tile.and_then(|tile| tile.city.as_ref()))
    }

    /// If the specified player controls a city with ID `city_id`, return it
    pub fn player_city_by_id(
        &self,
        player_secret: PlayerSecret,
        city_id: CityID,
    ) -> UmpireResult<Option<&City>> {
        self.player_cities(player_secret)
            .map(|mut cities| cities.find(|city| city.id == city_id))
    }

    /// If the current player controls a unit with ID `id`, return it
    fn current_player_unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.player_unit_by_id_by_idx(self.current_player, id)
    }

    /// If the specified player controls a unit with ID `id`, return it
    pub fn player_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<&Unit>> {
        self.player_with_secret(player_secret)
            .map(|player| self.player_unit_by_id_by_idx(player, id))
    }

    fn player_unit_by_id_by_idx(&self, player: PlayerNum, id: UnitID) -> Option<&Unit> {
        self.map.player_unit_by_id(player, id)
    }

    /// If the current player controls a unit with ID `id`, return its location
    #[cfg(test)]
    fn current_player_unit_loc(&self, id: UnitID) -> Option<Location> {
        let player_secret = self.player_secrets[self.current_player];
        self.player_unit_loc(player_secret, id).unwrap()
    }

    /// If the specified player controls a unit with ID `id`, return its location
    pub fn player_unit_loc(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Location>> {
        self.player_unit_by_id(player_secret, id)
            .map(|maybe_unit| maybe_unit.map(|unit| unit.loc))
    }

    /// If the current player controls the top-level unit at location `loc`, return it
    pub fn player_toplevel_unit_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<&Unit>> {
        self.player_tile(player_secret, loc)
            .map(|tile| tile.and_then(|tile| tile.unit.as_ref()))
    }

    #[cfg(test)]
    fn current_player_toplevel_unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        self.player_tile_by_idx(self.current_player, loc)
            .and_then(|tile| tile.unit.as_ref())
    }

    fn current_player_production_set_requests<'a>(&'a self) -> impl Iterator<Item = Location> + 'a {
        let player_secret = self.player_secrets[self.current_player];
        self.player_production_set_requests(player_secret).unwrap()
    }

    pub fn player_production_set_requests<'a>(
        &'a self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<impl Iterator<Item = Location> + 'a> {
        self.player_with_secret(player_secret)
            .and_then(|player| self.player_production_set_requests_by_idx(player))
    }

    fn player_production_set_requests_by_idx<'a>(
        &'a self,
        player: PlayerNum,
    ) -> UmpireResult<impl Iterator<Item = Location> + 'a> {
        self.validate_player_num(player)?;
        Ok(self
            .map
            .player_cities_lacking_production_target(player)
            .map(|city| city.loc))
    }

    /// Which if the current player's units need orders?
    ///
    /// In other words, which of the current player's units have no orders and have moves remaining?
    pub fn current_player_unit_orders_requests<'a>(&'a self) -> impl Iterator<Item = UnitID> + 'a {
        let player_secret = self.player_secrets[self.current_player];
        self.player_unit_orders_requests(player_secret).unwrap()
    }

    pub fn player_unit_orders_requests<'a>(
        &'a self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<impl Iterator<Item = UnitID> + 'a> {
        self.player_with_secret(player_secret).map(|player| {
            self.map
                .player_units(player)
                .filter(|unit| unit.orders.is_none() && unit.moves_remaining() > 0)
                .map(|unit| unit.id)
        })
    }

    /// Which if the specified player's units need orders?
    ///
    /// In other words, which of the specified player's units have no orders and have moves remaining?
    pub fn player_units_with_orders_requests<'a>(
        &'a self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<impl Iterator<Item = &Unit> + 'a> {
        self.player_with_secret(player_secret).map(|player| {
            self.map
                .player_units(player)
                .filter(|unit| unit.orders.is_none() && unit.moves_remaining() > 0)
        })
    }

    #[cfg(test)]
    fn current_player_units_with_pending_orders<'a>(&'a self) -> impl Iterator<Item = UnitID> + 'a {
        let player_secret = self.player_secrets[self.current_player];
        self.player_units_with_pending_orders(player_secret)
            .unwrap()
    }

    pub fn player_units_with_pending_orders<'a>(
        &'a self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<impl Iterator<Item = UnitID> + 'a> {
        self.player_units(player_secret).map(|units| {
            units
                .filter(|unit| {
                    unit.moves_remaining() > 0
                        && unit.orders.is_some()
                        && *unit.orders.as_ref().unwrap() != Orders::Sentry
                })
                .map(|unit| unit.id)
        })
    }

    // Movement-related methods

    /// Must be player's turn
    pub fn move_toplevel_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        let src = self
            .player_unit_loc(player_secret, unit_id)?
            .ok_or(GameError::MoveError(MoveError::SourceUnitDoesNotExist {
                id: unit_id,
            }))?;
        self.move_toplevel_unit_by_loc(player_secret, src, dest)
    }

    /// Must be player's turn
    pub fn move_toplevel_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        let src = self
            .player_unit_loc(player_secret, unit_id)?
            .ok_or(GameError::MoveError(MoveError::SourceUnitDoesNotExist {
                id: unit_id,
            }))?;
        self.move_toplevel_unit_by_loc_avoiding_combat(player_secret, src, dest)
    }

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
    pub fn move_toplevel_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        let unit = self
            .player_toplevel_unit_by_loc(player_secret, src)?
            .ok_or(GameError::MoveError(MoveError::SourceUnitNotAtLocation {
                src,
            }))?
            .clone();

        let filter = UnitMovementFilter::new(&unit);
        self.move_toplevel_unit_by_loc_using_filter(player_secret, src, dest, &filter)
    }

    /// Must be user's turn
    pub fn move_toplevel_unit_by_loc_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        let unit = self
            .player_toplevel_unit_by_loc(player_secret, src)?
            .ok_or(GameError::MoveError(MoveError::SourceUnitNotAtLocation {
                src,
            }))?
            .clone();
        let unit_filter = AndFilter::new(
            AndFilter::new(
                NoUnitsFilter {},
                NoCitiesButOursFilter {
                    alignment: unit.alignment,
                },
            ),
            UnitMovementFilter { unit: &unit },
        );
        self.move_toplevel_unit_by_loc_using_filter(player_secret, src, dest, &unit_filter)
    }

    /// Must be player's turn
    fn move_toplevel_unit_by_loc_using_filter<F: Filter<Obs>>(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
        filter: &F,
    ) -> UmpireResult<Move> {
        let unit_id = self
            .player_toplevel_unit_by_loc(player_secret, src)?
            .map(|unit| unit.id)
            .ok_or(GameError::MoveError(MoveError::SourceUnitNotAtLocation {
                src,
            }))?;

        self.move_unit_by_id_using_filter(player_secret, unit_id, dest, filter)
    }

    /// Move a unit one step in a particular direction
    ///
    /// Must be player's turn
    pub fn move_unit_by_id_in_direction(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move> {
        // let unit_loc = self.map.unit_by_id(id)
        // .ok_or_else(|| MoveError::SourceUnitDoesNotExist {id})?.loc;
        let unit = self
            .player_unit_by_id(player_secret, unit_id)?
            .ok_or(GameError::MoveError(MoveError::SourceUnitDoesNotExist {
                id: unit_id,
            }))?
            .clone();

        let filter = UnitMovementFilter::new(&unit);

        let dest = unit
            .loc
            .shift_wrapped(direction, self.dims(), self.wrapping())
            .ok_or_else(|| GameError::MoveError(MoveError::DestinationOutOfBounds {}))?;

        // self.move_unit_by_id(id, dest)
        self.move_unit_by_id_using_filter(player_secret, unit_id, dest, &filter)
    }

    /// Must be player's turn
    pub fn move_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        // self.propose_move_unit_by_id(unit_id, dest).map(|proposed_move| proposed_move.take(self))
        // let unit = self.current_player_unit_by_id(unit_id).unwrap().clone();
        let unit = self
            .player_unit_by_id(player_secret, unit_id)?
            .ok_or(GameError::MoveError(MoveError::SourceUnitDoesNotExist {
                id: unit_id,
            }))?
            .clone();

        let filter = UnitMovementFilterXenophile::new(&unit);
        self.move_unit_by_id_using_filter(player_secret, unit_id, dest, &filter)
    }

    pub fn propose_move_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        let mut new = self.clone();
        let move_ = new.move_unit_by_id(player_secret, id, dest)?;
        Ok(Proposed2 {
            action: PlayerAction::MoveUnit { unit_id: id, dest },
            outcome: move_,
        })
    }

    /// Must be player's turn
    pub fn move_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        let unit = self.map.unit_by_id(id).unwrap().clone();
        let unit_filter = AndFilter::new(
            AndFilter::new(
                NoUnitsFilter {},
                NoCitiesButOursFilter {
                    alignment: unit.alignment,
                },
            ),
            UnitMovementFilter { unit: &unit },
        );
        self.move_unit_by_id_using_filter(player_secret, id, dest, &unit_filter)
    }

    pub fn propose_move_unit_by_id_avoiding_combat(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        self.clone()
            .move_unit_by_id_avoiding_combat(player_secret, id, dest)
            .map(|move_| Proposed2 {
                action: PlayerAction::MoveUnit { unit_id: id, dest },
                outcome: move_,
            })
    }

    /// Make a best-effort attempt to move the given unit to the destination, generating shortest paths repeatedly using
    /// the given tile filter. This is necessary because, as the unit advances, it observes tiles which may have been
    /// previously observed but are now stale. If the tile state changes, then the shortest path will change and
    /// potentially other behaviors like unit carrying and combat.
    ///
    /// *player* is the number of the player attempting to make the move; they must control the specified unit, however
    /// this function does not check that such is the case.
    ///
    /// Must be player's turn
    fn move_unit_by_id_using_filter<F: Filter<Obs>>(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
        tile_filter: &F,
    ) -> UmpireResult<Move> {
        let player = self.validate_is_player_turn(player_secret)?;

        if !self.dims().contain(dest) {
            return Err(MoveError::DestinationOutOfBounds {}).map_err(GameError::MoveError);
        }

        // Grab a copy of the unit to work with
        let mut unit = self
            .player_unit_by_id_by_idx(player, unit_id)
            .ok_or(GameError::MoveError(MoveError::SourceUnitDoesNotExist {
                id: unit_id,
            }))?
            .clone();

        if unit.loc == dest {
            return Err(MoveError::ZeroLengthMove).map_err(GameError::MoveError);
        }

        // let obs_tracker = self.current_player_observations_mut();
        let obs_tracker = self.player_observations.tracker_mut(player).unwrap();

        // Keep a copy of the source location around
        let src = unit.loc;

        // The move components we will populate along the way
        let mut moves = Vec::new();

        // If we occupy a city then we declare the move complete and set remaining moves to zero, so dispense with
        // piecewise recording of movements.
        let mut movement_complete = false;

        let mut shortest_paths: Option<ShortestPaths> = None;
        let mut moves_remaining_on_last_shortest_paths_calculation = unit.moves_remaining();

        while unit.loc != dest {
            if shortest_paths.is_none() {
                // Establish a new "baseline"---calculation of shortest paths from the unit's current location
                shortest_paths = Some(dijkstra::shortest_paths(
                    obs_tracker,
                    unit.loc,
                    tile_filter,
                    self.wrapping,
                    unit.moves_remaining(),
                ));
                moves_remaining_on_last_shortest_paths_calculation = unit.moves_remaining();
            }

            if let Some(distance_to_dest_on_last_shortest_paths_calculation) =
                shortest_paths.as_ref().unwrap().dist.get(dest).cloned()
            {
                let movement_since_last_shortest_paths_calculation =
                    moves_remaining_on_last_shortest_paths_calculation - unit.moves_remaining();

                // Distance to destination from unit's current location
                let distance = distance_to_dest_on_last_shortest_paths_calculation
                    - movement_since_last_shortest_paths_calculation;

                // if distance == 0 {// We might be able to just assert this
                //     return Err(MoveError::ZeroLengthMove);
                // }

                if distance > unit.moves_remaining() {
                    return Err(MoveError::RemainingMovesExceeded {
                        id: unit_id,
                        src,
                        dest,
                        intended_distance: distance,
                        moves_remaining: unit.moves_remaining(),
                    })
                    .map_err(GameError::MoveError);
                }

                let shortest_path: Vec<Location> = shortest_paths
                    .as_ref()
                    .unwrap()
                    .shortest_path(dest)
                    .unwrap();

                // skip the source location and steps we've taken since "baseline"
                let loc =
                    shortest_path[1 + movement_since_last_shortest_paths_calculation as usize];

                let prev_loc = unit.loc;

                // Move our simulated unit along the path
                unit.loc = loc;

                moves.push(MoveComponent::new(prev_loc, loc));
                let mut move_ = moves.last_mut().unwrap();

                // If there is a unit at the destination:
                //   If it is a friendly unit:
                //     If it has carrying capacity
                //       Have it carry this unit
                //     else
                //       This doesn't happen---the search algorithm won't consider the location of there's no capacity
                //   else
                //     It is an enemy unit.
                //     Fight it.
                //     If victorious:
                //       If there is a city at the destination:
                //         It must be an enemy or there wouldn't have been an enemy unit there
                //         If this unit can occupy cities:
                //           Fight the city
                //           If victorious:
                //             Move this unit to the destination
                //           else:
                //             Destroy this unit
                //             END THE OVERALL MOVE
                //     If defeated:
                //       Destroy this unit
                //       END THE OVERALL MOVE
                // else if there is a city at the destination:
                //   If it is a friendly city
                //     Move this unit to the destination
                //   else if this unit can occupy cities
                //     Fight the city
                //     If victorious:
                //       Move this unit to the destination
                //     else
                //       Destroy this unit
                //     END THE OVERALL MOVE
                // else:
                //   There is nothing at the destination
                //   Move this unit to the destination, free and easy

                // If there is a unit at the destination:
                if let Some(ref other_unit) = self.map.toplevel_unit_by_loc(loc).cloned() {
                    // CLONE to dodge mutability

                    // If it is a friendly unit:
                    if unit.is_friendly_to(other_unit) {
                        debug_assert_ne!(unit.id, other_unit.id);
                        debug_assert!(other_unit.can_carry_unit(&unit));

                        // the friendly unit must have space for us in its carrying capacity or else the
                        // path search wouldn't have included it
                        move_.carrier = Some(other_unit.id);
                        if let Err(e) = self.map.carry_unit_by_id(other_unit.id, unit_id) {
                            let src_tile = self.map.tile(prev_loc).unwrap();
                            let tile = self.map.tile(loc).unwrap();

                            panic!(
                                "Could not carry unit for some weird reason: {:?}
                                    tile: {:?}
                                    tile city: {:?}
                                    tile unit: {:?}
                                    unit: {:?}
                                    src_tile: {:?}
                                    src_tile city: {:?}
                                    src_tile unit: {:?}",
                                e,
                                tile,
                                tile.city,
                                tile.unit,
                                unit,
                                src_tile,
                                src_tile.city,
                                src_tile.unit
                            );
                        }
                        unit.record_movement(1).unwrap();
                    } else {
                        // It is an enemy unit.
                        // Fight it.
                        move_.unit_combat = Some(unit.fight(other_unit));
                        if move_.unit_combat.as_ref().unwrap().victorious() {
                            // We were victorious over the unit

                            // Record the victory for score calculation purposes
                            self.defeated_unit_hitpoints[self.current_player] +=
                                other_unit.max_hp() as u64;

                            // Destroy the conquered unit
                            self.map.pop_unit_by_loc_and_id(loc, other_unit.id).unwrap();

                            // Deal with any city
                            if let Some(city) = self.map.city_by_loc(loc) {
                                // It must be an enemy city or there wouldn't have been an enemy unit there

                                // If this unit can occupy cities
                                if unit.can_occupy_cities() {
                                    // Fight the enemy city
                                    move_.city_combat = Some(unit.fight(city));

                                    // If victorious
                                    if move_.city_combat.as_ref().unwrap().victorious() {
                                        self.map.occupy_city(unit_id, loc).unwrap();
                                        movement_complete = true;
                                    } else {
                                        // Destroy this unit
                                        self.map.pop_unit_by_id(unit_id).unwrap();
                                    }
                                } else {
                                    // This unit can't occupy cities
                                    // Nerf this move since we didn't actually go anywhere and end the overall move
                                    // We don't have to set the unit's location here since MapData takes care of that
                                    move_.loc = prev_loc;
                                    unit.loc = prev_loc;
                                }

                                // END THE OVERALL MOVE
                                // We either occupied an enemy city (thus ending movement), or were destroyed fighting
                                // a city, or had to stop because this unit cannot occupy cities
                                break;
                            } else {
                                // There was no city, we just defeated an enemy, move to the destination
                                let prior_unit =
                                    self.map.relocate_unit_by_id(unit_id, loc).unwrap();
                                debug_assert!(prior_unit.is_none());
                                unit.record_movement(1).unwrap();
                            }
                        } else {
                            // We were not victorious against the enemy unit
                            // Destroy this unit and end the overall move
                            self.map.pop_unit_by_id(unit_id).unwrap();
                            break;
                        }
                    }
                } else if let Some(city) = self.map.city_by_loc(loc) {
                    // If it is a friendly city
                    if unit.is_friendly_to(city) {
                        // Move this unit to the destination
                        self.map.relocate_unit_by_id(unit_id, loc).unwrap();
                        // self.map.record_unit_movement(unit_id, 1).unwrap().unwrap();
                        unit.record_movement(1).unwrap();
                    } else {
                        // If the unit couldn't occupy cities then this location wouldn't be in the path, but let's
                        // check the assumption
                        debug_assert!(unit.can_occupy_cities());

                        move_.city_combat = Some(unit.fight(city));

                        // If victorious
                        if move_.city_combat.as_ref().unwrap().victorious() {
                            self.map.occupy_city(unit_id, loc).unwrap();
                            movement_complete = true;
                        } else {
                            // Destroy this unit
                            self.map.pop_unit_by_id(unit_id).unwrap();
                        }

                        // END THE OVERALL MOVE
                        // We either occupied an enemy city (thus ending movement), or were destroyed fighting
                        // a city, or had to stop because this unit cannot occupy cities
                        break;
                    }
                } else {
                    // There is nothing at the destination
                    // Move this unit to the destination, free and easy

                    let prior_unit = self.map.relocate_unit_by_id(unit_id, loc).unwrap();
                    debug_assert!(prior_unit.is_none());
                    unit.record_movement(1).unwrap();
                }

                // ----- Make observations from the unit's new location -----
                move_.observations_after_move =
                    unit.observe(&self.map, self.turn, self.wrapping, obs_tracker);

                // Inspect all observations besides at the unit's previous and current location to see if any changes in
                // passability have occurred relevant to the unit's future moves.
                // If so, request shortest_paths to be recalculated
                let passability_changed = move_
                    .observations_after_move
                    .iter()
                    .filter(|located_obs| {
                        located_obs.loc != unit.loc && located_obs.loc != prev_loc
                    })
                    .any(|located_obs| located_obs.passability_changed(tile_filter));
                if passability_changed {
                    // Mark the shortest_paths stale so it gets recomputed
                    shortest_paths = None;
                }
            } else {
                return Err(MoveError::NoRoute {
                    src,
                    dest,
                    id: unit_id,
                })
                .map_err(GameError::MoveError);
            }
        } // while

        let move_ = moves.last_mut().unwrap();
        // ----- Make observations from the unit's new location -----
        move_.observations_after_move =
            unit.observe(&self.map, self.turn, self.wrapping, obs_tracker);

        // If the unit wasn't destroyed, register its movement in the map rather than just this clone
        if move_.moved_successfully() {
            if movement_complete {
                self.map.mark_unit_movement_complete(unit_id).unwrap();
                unit.movement_complete();
            } else {
                let distance_moved = moves
                    .iter()
                    .map(|move_| move_.distance_moved() as u16)
                    .sum();
                self.map
                    .record_unit_movement(unit_id, distance_moved)
                    .unwrap()
                    .unwrap();
            }
        }

        self.action_taken();

        Move::new(unit, src, moves).map_err(GameError::MoveError)
    }

    /// Disbands
    ///
    /// Must be player's turn
    pub fn disband_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Unit> {
        let player = self.validate_is_player_turn(player_secret)?;

        let unit = self
            .map
            .pop_player_unit_by_id(player, unit_id)
            .ok_or(GameError::NoSuchUnit { id: unit_id })?;

        // Make a fresh observation with the disbanding unit so that its absence is noted.
        let obs_tracker = self.player_observations.tracker_mut(player).unwrap();
        unit.observe(&self.map, self.turn, self.wrapping, obs_tracker);

        self.action_taken();

        Ok(unit)
    }

    /// Sets the production of the current player's city at location `loc` to `production`, returning the prior setting.
    ///
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    pub fn set_production_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        let player = self.player_with_secret(player_secret)?;

        let result = self
            .map
            .set_player_city_production_by_loc(player, loc, production);
        if result.is_ok() {
            self.action_taken();
        }
        result
    }

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    ///
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    pub fn set_production_by_id(
        &mut self,
        player_secret: PlayerSecret,
        city_id: CityID,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        let player = self.player_with_secret(player_secret)?;
        let result = self
            .map
            .set_player_city_production_by_id(player, city_id, production);
        if result.is_ok() {
            self.action_taken();
        }
        result
    }

    /// Clears the production of a city at location `loc` if one exists and is controlled by the
    /// specified player.
    ///
    /// Returns the prior production (if any) on success, otherwise `GameError::NoCityAtLocation`
    pub fn clear_production(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<Option<UnitType>> {
        let city = self.player_city_by_loc(player_secret, loc)?;
        if city.is_none() {
            return Err(GameError::NoCityAtLocation { loc });
        }

        self.map
            .clear_city_production_by_loc(loc, ignore_cleared_production)
    }

    pub fn turn(&self) -> TurnNum {
        self.turn
    }

    pub fn current_player(&self) -> PlayerNum {
        self.current_player
    }

    /// The logical dimensions of the game map
    pub fn dims(&self) -> Dims {
        self.map.dims()
    }

    pub fn wrapping(&self) -> Wrap2d {
        self.wrapping
    }

    /// Units that could be produced by a city located at the given location controlled by the specified player
    ///
    /// ## Parameters
    /// * `conservative`: only consider unit types which can actually leave the city (rather
    ///   than just attacking neighbor cities, potentially not occupying them)
    ///
    /// ## Errors
    /// * `GameError::NoPlayerIdentifiedBySecret`: If no such player exists
    /// * `GameError::NoCityAtLocation`: If a city controlled by the player doesn't exist at `loc`
    fn _valid_productions<'a>(
        &'a self,
        player_secret: PlayerSecret,
        loc: Location,
        conservative: bool,
    ) -> UmpireResult<impl Iterator<Item = UnitType> + 'a> {
        let player = self.player_with_secret(player_secret)?;

        // Make sure there's a city controlled by the player at the given location
        self.map
            .player_city_by_loc(player, loc)
            .ok_or(GameError::NoCityAtLocation { loc })?;

        Ok(UNIT_TYPES.iter().cloned().filter(move |unit_type| {
            for neighb_loc in neighbors_terrain_only(&self.map, loc, *unit_type, self.wrapping) {
                let tile = self.map.tile(neighb_loc).unwrap();

                let include = if conservative {
                    unit_type.can_occupy_tile(tile)
                } else {
                    unit_type.can_move_on_tile(tile)
                };

                if include {
                    return true;
                }
            }
            false
        }))
    }

    pub fn valid_productions<'a>(
        &'a self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<impl Iterator<Item = UnitType> + 'a> {
        self._valid_productions(player_secret, loc, false)
    }

    pub fn valid_productions_conservative<'a>(
        &'a self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<impl Iterator<Item = UnitType> + 'a> {
        self._valid_productions(player_secret, loc, true)
    }

    pub fn current_player_valid_productions_conservative<'a>(
        &'a self,
        loc: Location,
    ) -> impl Iterator<Item = UnitType> + 'a {
        let player_secret = self.player_secrets[self.current_player];

        self.valid_productions_conservative(player_secret, loc)
            .unwrap()
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    pub fn order_unit_sentry(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        let player = self.player_with_secret(player_secret)?;

        let orders = Orders::Sentry;

        self.map.set_player_unit_orders(player, unit_id, orders)?;

        let ordered_unit = self
            .player_unit_by_id(player_secret, unit_id)?
            .unwrap()
            .clone();

        self.action_taken();

        Ok(OrdersOutcome::completed_without_move(ordered_unit, orders))
    }

    pub fn order_unit_skip(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        let orders = Orders::Skip;
        let unit = self
            .player_unit_by_id(player_secret, unit_id)?
            .unwrap()
            .clone();
        let result = self
            .set_orders(player_secret, unit_id, orders)
            .map(|_| OrdersOutcome::in_progress_without_move(unit, orders));
        if result.is_ok() {
            self.action_taken();
        }
        result
    }

    pub fn order_unit_go_to(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult {
        self.set_and_follow_orders(player_secret, unit_id, Orders::GoTo { dest })
    }

    /// Simulate ordering the specified unit to go to the given location
    pub fn propose_order_unit_go_to(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult {
        self.propose_set_and_follow_orders(player_secret, unit_id, Orders::GoTo { dest })
    }

    pub fn order_unit_explore(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.set_and_follow_orders(player_secret, unit_id, Orders::Explore)
    }

    /// Simulate ordering the specified unit to explore.
    pub fn propose_order_unit_explore(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> ProposedOrdersResult {
        self.propose_set_and_follow_orders(player_secret, unit_id, Orders::Explore)
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    pub fn activate_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<()> {
        let player = self.validate_is_player_turn(player_secret)?;

        let unit_id = {
            let unit = self
                .player_toplevel_unit_by_loc(player_secret, loc)?
                .ok_or(GameError::NoUnitAtLocation { loc })?;

            if !unit.belongs_to_player(player) {
                return Err(GameError::UnitNotControlledByCurrentPlayer {});
            }

            unit.id
        };

        self.map.activate_player_unit(player, unit_id)
    }

    /// If the current player controls a unit with ID `id`, set its orders to `orders`
    ///
    /// # Errors
    /// `OrdersError::OrderedUnitDoesNotExist` if the order is not present under the control of the current player
    pub fn set_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> UmpireResult<Option<Orders>> {
        let player = self.player_with_secret(player_secret)?;

        self.map.set_player_unit_orders(player, id, orders)
    }

    /// Clear the orders of the unit controlled by the current player with ID `id`.
    ///
    /// Can happen at any time
    pub fn clear_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Orders>> {
        let player = self.player_with_secret(player_secret)?;

        self.map.clear_player_unit_orders(player, id)
    }

    fn follow_pending_orders(
        &mut self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<OrdersResult>> {
        self.validate_is_player_turn(player_secret)?;

        let pending_orders: Vec<UnitID> = self
            .player_units_with_pending_orders(player_secret)?
            .collect();

        Ok(pending_orders
            .iter()
            .map(|unit_id| self.follow_unit_orders(player_secret, *unit_id))
            .collect())
    }

    /// Make the unit with ID `id` under the current player's control follow its orders
    ///
    /// # Panics
    /// This will panic if the current player does not control such a unit.
    ///
    fn follow_unit_orders(&mut self, player_secret: PlayerSecret, id: UnitID) -> OrdersResult {
        let player = self.validate_is_player_turn(player_secret)?;

        let orders = self
            .player_unit_by_id(player_secret, id)?
            .unwrap()
            .orders
            .as_ref()
            .unwrap();

        let result = orders.carry_out(id, self, player_secret);

        // If the orders are already complete, clear them out
        if let Ok(OrdersOutcome {
            status: OrdersStatus::Completed,
            ..
        }) = result
        {
            self.map.clear_player_unit_orders(player, id)?;
        }

        result
    }

    /// Simulate setting the orders of unit with ID `id` to `orders` and then following them out.
    pub fn propose_set_and_follow_orders(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> ProposedOrdersResult {
        self.clone()
            .set_and_follow_orders(player_secret, id, orders)
            .map(|orders_outcome| Proposed2 {
                action: PlayerAction::OrderUnit {
                    unit_id: id,
                    orders,
                },
                outcome: orders_outcome,
            })
    }

    pub fn set_and_follow_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> OrdersResult {
        self.validate_is_player_turn(player_secret)?;

        self.set_orders(player_secret, id, orders)?;

        let result = self.follow_unit_orders(player_secret, id);

        if result.is_ok() {
            self.action_taken();
        }

        result
    }

    pub fn current_player_score(&self) -> f64 {
        self.player_score_by_idx(self.current_player).unwrap()
    }

    pub fn player_score(&self, player_secret: PlayerSecret) -> UmpireResult<f64> {
        let player = self.player_with_secret(player_secret)?;

        self.player_score_by_idx(player)
    }

    pub fn player_score_by_idx(&self, player: PlayerNum) -> UmpireResult<f64> {
        let mut score = 0.0;

        // Observations
        let observed_tiles = self
            .player_observations
            .tracker(player)
            .ok_or(GameError::NoSuchPlayer { player })?
            .num_observed();

        score += observed_tiles as f64 * TILE_OBSERVED_BASE_SCORE;

        // Controlled units
        for unit in self.player_units_by_idx(player) {
            // The cost of the unit scaled by the unit's current hitpoints relative to maximum
            score += UNIT_MULTIPLIER * (unit.type_.cost() as f64) * (unit.hp() as f64)
                / (unit.max_hp() as f64);
        }

        // Defeated units
        score += UNIT_MULTIPLIER * self.defeated_unit_hitpoints[player] as f64;

        // Controlled cities
        for city in self.player_cities_by_idx(player)? {
            // The city's intrinsic value plus any progress it's made toward producing its unit
            score += CITY_INTRINSIC_SCORE + city.production_progress as f64 * UNIT_MULTIPLIER;
        }

        // Turn penalty to discourage sitting around
        score -= TURN_PENALTY * self.turn as f64;

        // Penalty for each action taken
        score -= ACTION_PENALTY * self.action_counts[player] as f64;

        // Victory
        if let Some(victor) = self.victor() {
            if victor == player {
                score += VICTORY_SCORE;
            }
        }

        Ok(score)
    }

    /// Each player's current score, indexed by player number
    pub fn player_scores(&self) -> Vec<f64> {
        (0..self.num_players)
            .map(|player| self.player_score_by_idx(player).unwrap())
            .collect()
    }

    pub fn take_simple_action(
        &mut self,
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        self.take_action(
            player_secret,
            match action {
                AiPlayerAction::SetNextCityProduction { unit_type } => {
                    let city_loc = self
                        .current_player_production_set_requests()
                        .next()
                        .unwrap();
                    let city_id = self
                        .player_city_by_loc(player_secret, city_loc)?
                        .unwrap()
                        .id;
                    PlayerAction::SetCityProduction {
                        city_id,
                        production: unit_type,
                    }
                }
                AiPlayerAction::MoveNextUnit { direction } => {
                    let unit_id = self.current_player_unit_orders_requests().next().unwrap();
                    debug_assert!({
                        let legal: HashSet<Direction> = self
                            .current_player_unit_legal_directions(unit_id)
                            .unwrap()
                            .collect();

                        // println!("legal moves: {}", legal.len());

                        legal.contains(&direction)
                    });

                    PlayerAction::MoveUnitInDirection { unit_id, direction }
                }
                AiPlayerAction::DisbandNextUnit => {
                    let unit_id = self.current_player_unit_orders_requests().next().unwrap();
                    PlayerAction::DisbandUnit { unit_id }
                }
                AiPlayerAction::SkipNextUnit => {
                    let unit_id = self.current_player_unit_orders_requests().next().unwrap();
                    PlayerAction::OrderUnit {
                        unit_id,
                        orders: Orders::Skip,
                    }
                }
            },
        )
    }

    pub fn take_action(
        &mut self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> Result<PlayerActionOutcome, GameError> {
        action.take(self, player_secret)
    }

    pub fn propose_action(
        &self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> ProposedActionResult {
        let mut game = self.clone();
        let outcome = game.take_action(player_secret, action)?;

        Ok(Proposed2 { action, outcome })
    }

    /// Feature vector for use in AI training; the specified player's current state
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
    pub fn player_features(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<fX>> {
        // For every tile we add these f64's:
        // is the tile observed or not?
        // which player controls the tile (one hot encoded)
        // is there a city or not?
        // what is the unit type? (one hot encoded, could be none---all zeros)
        // for each of the five potential carried units:
        //   what is the unit type? (one hot encoded, could be none---all zeros)
        //

        let unit_id = self.player_unit_orders_requests(player_secret)?.next();
        let city_loc = self.player_production_set_requests(player_secret)?.next();

        let unit_type = if let Some(unit_id) = unit_id {
            self.player_unit_by_id(player_secret, unit_id)
                .map(|maybe_unit| maybe_unit.map(|unit| unit.type_))
                .unwrap()
        } else {
            None
        };

        // We also add a context around the currently active unit (if any)
        let mut x = Vec::with_capacity(FEATS_LEN as usize);

        // General statistics

        // NOTE Update dnn::ADDED_WIDE_FEATURES to reflect the number of generic features added here

        // - current turn
        x.push(self.turn as fX);

        // - number of cities player controls
        x.push(self.player_city_count(player_secret).unwrap() as fX);

        let observations = self.player_observations(player_secret).unwrap();

        // - number of tiles observed
        let num_observed: fX = observations.num_observed() as fX;
        x.push(num_observed);

        // - percentage of tiles observed
        let dims = self.dims();
        x.push(num_observed / dims.area() as fX);

        // - unit type writ large
        for unit_type_ in &UnitType::values() {
            x.push(if let Some(unit_type) = unit_type {
                if unit_type == *unit_type_ {
                    1.0
                } else {
                    0.0
                }
            } else {
                0.0
            });
        }
        // Also includes whether it's a city or not
        x.push(if city_loc.is_some() { 1.0 } else { 0.0 });

        // NOTE The unit counts are not included in dnn::ADDED_WIDE_FEATURES
        // - number of each type of unit controlled by player
        let empty_map = HashMap::new();
        let type_counts = self
            .player_unit_type_counts(player_secret)
            .unwrap_or(&empty_map);
        let counts_vec: Vec<fX> = UnitType::values()
            .iter()
            .map(|type_| *type_counts.get(type_).unwrap_or(&0) as fX)
            .collect();

        x.extend(counts_vec);

        // Relatively positioned around next unit (if any) or city

        let loc = if let Some(unit_id) = unit_id {
            Some(match self.player_unit_loc(player_secret, unit_id)? {
                Some(loc) => loc,
                None => {
                    panic!("Unit was in orders requests but not in current player observations")
                }
            })
        } else {
            city_loc
        };

        let mut is_enemy_belligerent = Vec::new();
        let mut is_observed = Vec::new();
        let mut is_neutral = Vec::new();
        let mut is_city = Vec::new();

        let player = self.current_player();

        // 2d features
        for inc_x in -5..=5 {
            for inc_y in -5..=5 {
                let inc: Vec2d<i32> = Vec2d::new(inc_x, inc_y);

                let obs = if let Some(origin) = loc {
                    self.wrapping
                        .wrapped_add(dims, origin, inc)
                        .map_or(&Obs::Unobserved, |loc| observations.get(loc))
                } else {
                    &Obs::Unobserved
                };

                // x.extend_from_slice(&obs_to_vec(&obs, self.num_players));
                // push_obs_to_vec(&mut x, &obs, self.num_players);

                let mut enemy = 0.0;
                let mut observed = 0.0;
                let mut neutral = 0.0;
                let mut city = 0.0;

                if let Obs::Observed { tile, .. } = obs {
                    observed = 1.0;

                    if tile.city.is_some() {
                        city = 1.0;
                    }

                    if let Some(alignment) = tile.alignment_maybe() {
                        if alignment.is_neutral() {
                            neutral = 1.0;
                        } else if alignment.is_belligerent() && alignment.is_enemy_of_player(player)
                        {
                            enemy = 1.0;
                        }
                    }
                }

                is_enemy_belligerent.push(enemy);
                is_observed.push(observed);
                is_neutral.push(neutral);
                is_city.push(city);
            }
        }

        x.extend(is_enemy_belligerent);
        x.extend(is_observed);
        x.extend(is_neutral);
        x.extend(is_city);

        Ok(x)
    }

    fn current_player_features(&self) -> Vec<fX> {
        let player_secret = self.player_secrets[self.current_player];
        self.player_features(player_secret).unwrap()
    }
}

impl Dimensioned for Game {
    fn dims(&self) -> Dims {
        self.dims()
    }
}

/// FIXME Unimplement this so we don't leak information? Or audit usage.
impl Source<Tile> for Game {
    fn get(&self, loc: Location) -> &Tile {
        self.current_player_tile(loc).unwrap()
    }
}

/// FIXME Unimplement this so we don't leak information? Or audit usage.
impl Source<Obs> for Game {
    fn get(&self, loc: Location) -> &Obs {
        self.current_player_obs(loc)
    }
}

fn push_obs_to_vec(x: &mut Vec<f64>, obs: &Obs, num_players: PlayerNum) {
    match obs {
        Obs::Unobserved => {
            let n_zeros = 1// unobserved
                + num_players// which player controls the tile (nobody, one hot encoded)
                + 1//city or not
                + 6 * UnitType::values().len()// what is the unit type? (one hot encoded), for this unit and any
                                              // carried units. Could be none (all zeros)
            ;
            x.extend(vec![0.0; n_zeros]);
            // for _ in 0..n_zeros {
            //     x.push(0.0);
            // }
        }
        Obs::Observed { tile, .. } => {
            // let mut x = vec![1.0];// observed
            x.push(1.0); // observed

            for p in 0..num_players {
                // which player controls the tile (one hot encoded)
                x.push(
                    if let Some(Alignment::Belligerent { player }) = tile.alignment_maybe() {
                        if player == p {
                            1.0
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    },
                );
            }

            x.push(if tile.city.is_some() { 1.0 } else { 0.0 }); // city or not

            let mut units_unaccounted_for = 6;

            if let Some(ref unit) = tile.unit {
                units_unaccounted_for -= 1;
                for t in UnitType::values().iter() {
                    x.push(if unit.type_ == *t { 1.0 } else { 0.0 });
                }

                for carried_unit in unit.carried_units() {
                    units_unaccounted_for -= 1;
                    for t in UnitType::values().iter() {
                        x.push(if carried_unit.type_ == *t { 1.0 } else { 0.0 });
                    }
                }
            }

            // fill in zeros for any missing units
            x.extend_from_slice(&vec![0.0; UnitType::values().len() * units_unaccounted_for]);

            // x
        }
    }
}

/// Push a one-hot-encoded representation of a direction (or none at all) onto a vector
fn push_dir_to_vec(x: &mut Vec<f64>, dir: Option<Direction>) {
    if let Some(dir) = dir {
        for dir2 in Direction::values().iter() {
            x.push(if dir == *dir2 { 1.0 } else { 0.0 });
        }
    } else {
        for _ in 0..Direction::values().len() {
            x.push(0.0);
        }
    }
}

impl fmt::Debug for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.map.fmt(f)
    }
}

impl DerefVec for Game {
    fn deref_vec(&self) -> Vec<fX> {
        self.current_player_features()
    }
}

#[async_trait]
impl IGame for Game {
    async fn num_players(&self) -> PlayerNum {
        self.num_players()
    }

    async fn player_turn_control<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        PlayerTurnControl::new(self, secret).await
    }

    async fn player_turn_control_clearing<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        PlayerTurnControl::new_clearing(self, secret).await
    }

    async fn player_turn_control_nonending<'a>(
        &'a mut self,
        secret: PlayerSecret,
    ) -> UmpireResult<(PlayerTurnControl<'a>, TurnStart)> {
        PlayerTurnControl::new_nonending(self, secret).await
    }

    async fn is_player_turn(&self, secret: PlayerSecret) -> UmpireResult<bool> {
        self.is_player_turn(secret)
    }

    async fn begin_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<TurnStart> {
        self.begin_turn(player_secret)
    }

    async fn begin_turn_clearing(
        &mut self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.begin_turn_clearing(player_secret)
    }

    async fn turn_is_done(&self, player: PlayerNum, turn: TurnNum) -> UmpireResult<bool> {
        self.turn_is_done(player, turn)
    }

    async fn current_turn_is_done(&self) -> bool {
        self.current_turn_is_done()
    }

    async fn victor(&self) -> Option<PlayerNum> {
        self.victor()
    }

    async fn end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<()> {
        self.end_turn(player_secret)
    }

    async fn force_end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<()> {
        self.force_end_turn(player_secret)
    }

    async fn end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.end_then_begin_turn(player_secret, next_player_secret)
    }

    async fn end_then_begin_turn_clearing(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.end_then_begin_turn_clearing(player_secret, next_player_secret)
    }

    async fn force_end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.force_end_then_begin_turn(player_secret, next_player_secret)
    }

    async fn force_end_then_begin_turn_clearing(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
    ) -> UmpireResult<TurnStart> {
        self.force_end_then_begin_turn_clearing(player_secret, next_player_secret)
    }

    async fn player_unit_legal_one_step_destinations(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<HashSet<Location>> {
        self.player_unit_legal_one_step_destinations(player_secret, unit_id)
    }

    async fn player_unit_legal_directions(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Vec<Direction>> {
        self.player_unit_legal_directions(player_secret, unit_id)
            .map(|dirs| dirs.collect())
    }

    async fn player_tile(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Cow<Tile>>> {
        self.player_tile(player_secret, loc)
            .map(|tile| tile.map(|tile| Cow::Borrowed(tile)))
    }

    async fn player_obs(&self, player_secret: PlayerSecret, loc: Location) -> UmpireResult<Obs> {
        self.player_obs(player_secret, loc).map(|obs| obs.clone())
    }

    async fn player_observations(&self, player_secret: PlayerSecret) -> UmpireResult<ObsTracker> {
        self.player_observations(player_secret)
            .map(|tracker| tracker.clone())
    }

    async fn player_cities(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<City>> {
        self.player_cities(player_secret)
            .map(|cities| cities.cloned().collect())
    }

    async fn player_cities_with_production_target(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<City>> {
        self.player_cities_with_production_target(player_secret)
            .map(|cities| cities.cloned().collect())
    }

    async fn player_city_count(&self, player_secret: PlayerSecret) -> UmpireResult<usize> {
        self.player_city_count(player_secret)
    }

    async fn player_cities_producing_or_not_ignored(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize> {
        self.player_cities_producing_or_not_ignored(player_secret)
    }

    async fn player_units(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<Unit>> {
        self.player_units(player_secret)
            .map(|units| units.cloned().collect())
    }

    async fn player_unit_type_counts(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<HashMap<UnitType, usize>> {
        self.player_unit_type_counts(player_secret)
            .map(|counts| counts.clone())
    }

    async fn player_city_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<City>> {
        self.player_city_by_loc(player_secret, loc)
            .map(|city| city.cloned())
    }

    async fn player_city_by_id(
        &self,
        player_secret: PlayerSecret,
        city_id: CityID,
    ) -> UmpireResult<Option<City>> {
        self.player_city_by_id(player_secret, city_id)
            .map(|city| city.cloned())
    }

    async fn player_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Unit>> {
        self.player_unit_by_id(player_secret, id)
            .map(|unit| unit.cloned())
    }

    async fn player_unit_loc(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Location>> {
        self.player_unit_loc(player_secret, id)
            .map(|loc| loc.clone())
    }

    async fn player_toplevel_unit_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Unit>> {
        self.player_toplevel_unit_by_loc(player_secret, loc)
            .map(|unit| unit.cloned())
    }

    async fn player_production_set_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Location>> {
        self.player_production_set_requests(player_secret)
            .map(|rqsts| rqsts.collect())
    }

    async fn player_unit_orders_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>> {
        self.player_unit_orders_requests(player_secret)
            .map(|rqsts| rqsts.collect())
    }

    async fn player_units_with_orders_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Unit>> {
        self.player_units_with_orders_requests(player_secret)
            .map(|units| units.cloned().collect())
    }

    async fn player_units_with_pending_orders(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>> {
        Game::player_units_with_pending_orders(self, player_secret).map(|units| units.collect())
    }

    async fn move_toplevel_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_toplevel_unit_by_id(self, player_secret, unit_id, dest)
    }

    async fn move_toplevel_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_toplevel_unit_by_id_avoiding_combat(self, player_secret, unit_id, dest)
    }

    async fn move_toplevel_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_toplevel_unit_by_loc(self, player_secret, src, dest)
    }

    async fn move_toplevel_unit_by_loc_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_toplevel_unit_by_loc_avoiding_combat(self, player_secret, src, dest)
    }

    async fn move_unit_by_id_in_direction(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move> {
        Game::move_unit_by_id_in_direction(self, player_secret, unit_id, direction)
    }

    async fn move_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_unit_by_id(self, player_secret, unit_id, dest)
    }

    async fn propose_move_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        Game::propose_move_unit_by_id(self, player_secret, id, dest)
    }

    async fn move_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.move_unit_by_id_avoiding_combat(player_secret, id, dest)
    }

    async fn propose_move_unit_by_id_avoiding_combat(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        self.propose_move_unit_by_id_avoiding_combat(player_secret, id, dest)
    }

    async fn disband_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Unit> {
        self.disband_unit_by_id(player_secret, unit_id)
    }

    async fn set_production_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        self.set_production_by_loc(player_secret, loc, production)
    }

    async fn set_production_by_id(
        &mut self,
        player_secret: PlayerSecret,
        city_id: CityID,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        self.set_production_by_id(player_secret, city_id, production)
    }

    async fn clear_production(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<Option<UnitType>> {
        self.clear_production(player_secret, loc, ignore_cleared_production)
    }

    async fn turn(&self) -> TurnNum {
        self.turn()
    }

    async fn current_player(&self) -> PlayerNum {
        self.current_player()
    }

    async fn dims(&self) -> Dims {
        self.dims()
    }

    async fn wrapping(&self) -> Wrap2d {
        self.wrapping()
    }

    async fn valid_productions(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>> {
        self.valid_productions(player_secret, loc)
            .map(|prods| prods.collect())
    }

    async fn valid_productions_conservative(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>> {
        self.valid_productions_conservative(player_secret, loc)
            .map(|prods| prods.collect())
    }

    async fn order_unit_sentry(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.order_unit_sentry(player_secret, unit_id)
    }

    async fn order_unit_skip(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.order_unit_skip(player_secret, unit_id)
    }

    async fn order_unit_go_to(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult {
        self.order_unit_go_to(player_secret, unit_id, dest)
    }

    async fn propose_order_unit_go_to(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult {
        self.propose_order_unit_go_to(player_secret, unit_id, dest)
    }

    async fn order_unit_explore(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.order_unit_explore(player_secret, unit_id)
    }

    async fn propose_order_unit_explore(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> ProposedOrdersResult {
        self.propose_order_unit_explore(player_secret, unit_id)
    }

    async fn activate_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<()> {
        self.activate_unit_by_loc(player_secret, loc)
    }

    async fn set_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> UmpireResult<Option<Orders>> {
        self.set_orders(player_secret, id, orders)
    }

    async fn clear_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Orders>> {
        self.clear_orders(player_secret, id)
    }

    /// Simulate setting the orders of unit with ID `id` to `orders` and then following them out.
    async fn propose_set_and_follow_orders(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> ProposedOrdersResult {
        self.propose_set_and_follow_orders(player_secret, id, orders)
    }

    async fn set_and_follow_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> OrdersResult {
        self.set_and_follow_orders(player_secret, id, orders)
    }

    async fn current_player_score(&self) -> f64 {
        self.current_player_score()
    }

    async fn player_score(&self, player_secret: PlayerSecret) -> UmpireResult<f64> {
        self.player_score(player_secret)
    }

    async fn player_score_by_idx(&self, player: PlayerNum) -> UmpireResult<f64> {
        self.player_score_by_idx(player)
    }

    async fn player_scores(&self) -> Vec<f64> {
        self.player_scores()
    }

    async fn take_simple_action(
        &mut self,
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        self.take_simple_action(player_secret, action)
    }

    async fn take_action(
        &mut self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        self.take_action(player_secret, action)
    }

    async fn propose_action(
        &self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> ProposedActionResult {
        self.propose_action(player_secret, action)
    }

    fn clone_underlying_game_state(&self) -> Result<Game, String> {
        Ok(self.clone())
    }
}

/// Test support functions
pub mod test_support {

    use std::sync::{Arc, RwLock};

    use crate::{
        game::{
            error::GameError,
            map::{MapData, Terrain},
            obs::Obs,
            unit::{UnitID, UnitType},
            Alignment, Game,
        },
        name::unit_namer,
        util::{Dims, Location, Wrap2d},
    };

    use super::PlayerSecret;

    pub fn test_propose_move_unit_by_id() {
        let src = Location { x: 0, y: 0 };
        let dest = Location { x: 1, y: 0 };

        let (game, secrets) = game_two_cities_two_infantry();

        let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();

        {
            let unit = game.current_player_unit_by_id(unit_id).unwrap();
            assert_eq!(unit.loc, src);
        }

        let proposed_move = game
            .propose_move_unit_by_id(secrets[0], unit_id, dest)
            .unwrap()
            .outcome;

        let component = proposed_move.components.get(0).unwrap();

        // Make sure the intended destination is now observed as containing this unit, and that no other observed tiles
        // are observed as containing it
        for located_obs in &component.observations_after_move {
            match located_obs.obs {
                Obs::Observed {
                    ref tile,
                    turn,
                    current,
                } => {
                    if located_obs.loc == dest {
                        let unit = tile.unit.as_ref().unwrap();
                        assert_eq!(unit.id, unit_id);
                        assert_eq!(turn, 6);
                        assert!(current);
                    } else if let Some(unit) = tile.unit.as_ref() {
                        assert_ne!(unit.id, unit_id);
                    }
                }
                Obs::Unobserved => panic!("This should be observed"),
            }
        }
    }

    /// 10x10 grid of land only with two cities:
    /// * Player 0's Machang at 0,0
    /// * Player 1's Zanzibar at 0,1
    fn map_two_cities(dims: Dims) -> MapData {
        let mut map = MapData::new(dims, |_loc| Terrain::Land);
        map.new_city(
            Location { x: 0, y: 0 },
            Alignment::Belligerent { player: 0 },
            "Machang",
        )
        .unwrap();
        map.new_city(
            Location { x: 0, y: 1 },
            Alignment::Belligerent { player: 1 },
            "Zanzibar",
        )
        .unwrap();
        map
    }

    pub fn game1() -> (Game, Vec<PlayerSecret>) {
        let players = 2;
        let fog_of_war = true;

        let map = map_two_cities(Dims::new(10, 10));
        let unit_namer = unit_namer();
        Game::new_with_map(
            map,
            players,
            fog_of_war,
            Some(Arc::new(RwLock::new(unit_namer))),
            Wrap2d::BOTH,
        )
    }

    pub fn game_two_cities_dims(dims: Dims) -> (Game, Vec<PlayerSecret>) {
        let players = 2;
        let fog_of_war = true;

        let map = map_two_cities(dims);
        let unit_namer = unit_namer();
        let (mut game, secrets) = Game::new_with_map(
            map,
            players,
            fog_of_war,
            Some(Arc::new(RwLock::new(unit_namer))),
            Wrap2d::BOTH,
        );

        let loc: Location = game
            .current_player_production_set_requests()
            .next()
            .unwrap();

        // println!("Setting production at {:?} to infantry", loc);
        game.set_production_by_loc(secrets[0], loc, UnitType::Infantry)
            .unwrap();

        let player = game
            .end_then_begin_turn(secrets[0], secrets[1])
            .unwrap()
            .current_player;
        assert_eq!(player, 1);

        let loc: Location = game
            .current_player_production_set_requests()
            .next()
            .unwrap();
        // println!("Setting production at {:?} to infantry", loc);
        game.set_production_by_loc(secrets[1], loc, UnitType::Infantry)
            .unwrap();

        let player = game
            .end_then_begin_turn(secrets[1], secrets[0])
            .unwrap()
            .current_player;
        assert_eq!(player, 0);

        (game, secrets)
    }

    fn map_tunnel(dims: Dims) -> MapData {
        let mut map = MapData::new(dims, |_loc| Terrain::Land);
        map.new_city(
            Location::new(0, dims.height / 2),
            Alignment::Belligerent { player: 0 },
            "City 0",
        )
        .unwrap();
        map.new_city(
            Location::new(dims.width - 1, dims.height / 2),
            Alignment::Belligerent { player: 1 },
            "City 1",
        )
        .unwrap();
        map
    }

    pub fn game_tunnel(dims: Dims) -> (Game, Vec<PlayerSecret>) {
        let players = 2;
        let fog_of_war = false;
        let map = map_tunnel(dims);
        let unit_namer = unit_namer();
        Game::new_with_map(
            map,
            players,
            fog_of_war,
            Some(Arc::new(RwLock::new(unit_namer))),
            Wrap2d::NEITHER,
        )
    }

    // pub(crate) fn game_two_cities() -> Game {
    //     game_two_cities_dims(Dims::new(10, 10))
    // }

    // pub(crate) fn game_two_cities_big() -> Game {
    //     game_two_cities_dims(Dims::new(100, 100))
    // }

    pub fn game_two_cities_two_infantry_dims(dims: Dims) -> (Game, Vec<PlayerSecret>) {
        let (mut game, secrets) = game_two_cities_dims(dims);

        for _ in 0..5 {
            let player = game
                .end_then_begin_turn(secrets[0], secrets[1])
                .unwrap()
                .current_player;
            assert_eq!(player, 1);
            let player = game
                .end_then_begin_turn(secrets[1], secrets[0])
                .unwrap()
                .current_player;
            assert_eq!(player, 0);
        }

        assert_eq!(
            game.end_then_begin_turn(secrets[0], secrets[1]),
            Err(GameError::TurnEndRequirementsNotMet { player: 0 })
        );
        assert_eq!(
            game.end_then_begin_turn(secrets[0], secrets[1]),
            Err(GameError::TurnEndRequirementsNotMet { player: 0 })
        );

        (game, secrets)
    }

    pub fn game_two_cities_two_infantry() -> (Game, Vec<PlayerSecret>) {
        game_two_cities_two_infantry_dims(Dims::new(10, 10))
    }

    pub fn game_two_cities_two_infantry_big() -> (Game, Vec<PlayerSecret>) {
        game_two_cities_two_infantry_dims(Dims::new(100, 100))
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{HashMap, HashSet},
        sync::{Arc, RwLock},
    };

    use rand::{thread_rng, Rng};

    use crate::{
        game::{
            map::{MapData, Terrain},
            move_::MoveError,
            test_support::game_two_cities_two_infantry,
            unit::{
                orders::{Orders, OrdersStatus},
                TransportMode, Unit, UnitID, UnitType,
            },
            Alignment, Game, GameError,
        },
        name::{unit_namer, Named},
        util::{Dimensioned, Dims, Direction, Location, Vec2d, Wrap2d},
    };

    #[test]
    fn test_game() {
        let (mut game, secrets) = game_two_cities_two_infantry();

        for player in 0..2 {
            assert_eq!(game.current_player_unit_orders_requests().count(), 1);
            let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();
            let loc = game.current_player_unit_loc(unit_id).unwrap();
            let new_x = (loc.x + 1) % game.dims().width;
            let new_loc = Location { x: new_x, y: loc.y };
            println!("Moving unit from {} to {}", loc, new_loc);

            match game.move_toplevel_unit_by_loc(secrets[player], loc, new_loc) {
                Ok(move_result) => {
                    println!("{:?}", move_result);
                }
                Err(msg) => {
                    panic!("Error during move: {}", msg);
                }
            }

            let result = game.end_then_begin_turn(secrets[player], secrets[(player + 1) % 2]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1 - player);
        }
    }

    #[test]
    fn test_move_unit_by_id_far() {
        let mut map = MapData::new(Dims::new(180, 90), |_| Terrain::Water);
        let unit_id = map
            .new_unit(
                Location::new(0, 0),
                UnitType::Fighter,
                Alignment::Belligerent { player: 0 },
                "Han Solo",
            )
            .unwrap();

        let (mut game, secrets) = Game::new_with_map(map, 1, true, None, Wrap2d::BOTH);

        let mut rand = thread_rng();
        for _ in 0..10 {
            let mut delta = Vec2d::new(0, 0);

            while delta.x == 0 && delta.y == 0 {
                delta = Vec2d::new(rand.gen_range(-5, 6), rand.gen_range(-5, 6));
            }

            let unit_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
            let dest = game
                .wrapping()
                .wrapped_add(game.dims(), unit_loc, delta)
                .unwrap();
            let result = game.move_unit_by_id(secrets[0], unit_id, dest).unwrap();
            assert!(result.moved_successfully());
            assert_eq!(result.ending_loc(), Some(dest));

            game.force_end_then_begin_turn(secrets[0], secrets[0])
                .unwrap();
        }
    }

    #[test]
    fn test_move_unit() {
        let map = MapData::try_from("--0-+-+-1--").unwrap();
        {
            let loc1 = Location { x: 2, y: 0 };
            let loc2 = Location { x: 8, y: 0 };

            let city1tile = map.tile(loc1).unwrap();
            let city2tile = map.tile(loc2).unwrap();
            assert_eq!(city1tile.terrain, Terrain::Land);
            assert_eq!(city2tile.terrain, Terrain::Land);

            let city1 = city1tile.city.as_ref().unwrap();
            let city2 = city2tile.city.as_ref().unwrap();
            assert_eq!(city1.alignment, Alignment::Belligerent { player: 0 });
            assert_eq!(city2.alignment, Alignment::Belligerent { player: 1 });
            assert_eq!(city1.loc, loc1);
            assert_eq!(city2.loc, loc2);
        }

        let (mut game, secrets) = Game::new_with_map(
            map,
            2,
            false,
            Some(Arc::new(RwLock::new(unit_namer()))),
            Wrap2d::BOTH,
        );
        assert_eq!(game.current_player(), 0);

        {
            let loc = game
                .player_production_set_requests(secrets[0])
                .unwrap()
                .next()
                .unwrap();

            assert_eq!(
                game.set_production_by_loc(secrets[0], loc, UnitType::Armor),
                Ok(None)
            );

            let result = game.end_then_begin_turn(secrets[0], secrets[1]);

            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1);
        }

        {
            let loc = game
                .player_production_set_requests(secrets[1])
                .unwrap()
                .next()
                .unwrap();

            assert_eq!(
                game.set_production_by_loc(secrets[1], loc, UnitType::Carrier),
                Ok(None)
            );

            let result = game.end_then_begin_turn(secrets[1], secrets[0]);

            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 0);
        }

        for _ in 0..11 {
            let result = game.end_then_begin_turn(secrets[0], secrets[1]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1);

            let result = game.end_then_begin_turn(secrets[1], secrets[0]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 0);
        }
        assert_eq!(
            game.end_then_begin_turn(secrets[0], secrets[1]),
            Err(GameError::TurnEndRequirementsNotMet { player: 0 })
        );

        // Move the armor unit to the right until it attacks the opposing city
        for round in 0..3 {
            assert_eq!(game.current_player_unit_orders_requests().count(), 1);
            let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();
            let loc = {
                let unit = game.current_player_unit_by_id(unit_id).unwrap();
                assert_eq!(unit.type_, UnitType::Armor);
                unit.loc
            };

            let dest_loc = Location {
                x: loc.x + 2,
                y: loc.y,
            };
            println!("Moving from {} to {}", loc, dest_loc);
            let move_result = game
                .move_toplevel_unit_by_loc(secrets[0], loc, dest_loc)
                .unwrap();
            println!("Result: {:?}", move_result);

            assert_eq!(move_result.unit.type_, UnitType::Armor);
            assert_eq!(
                move_result.unit.alignment,
                Alignment::Belligerent { player: 0 }
            );

            // Check the first move component
            assert_eq!(move_result.components.len(), 2);
            let move1 = move_result.components.get(0).unwrap();
            assert_eq!(
                move1.loc,
                Location {
                    x: loc.x + 1,
                    y: loc.y
                }
            );
            assert_eq!(move1.unit_combat, None);
            assert_eq!(move1.city_combat, None);

            if move_result.moved_successfully() {
                // the unit conquered the city

                assert_eq!(move_result.ending_loc().unwrap(), dest_loc);

                assert_eq!(move_result.unit.moves_remaining(), 0);

                // Check the second move component, only here because the unit wasn't destroyed
                let move2 = move_result.components.get(1).unwrap();
                assert_eq!(move2.loc, dest_loc);
                assert_eq!(move2.unit_combat, None);

                if round < 2 {
                    assert_eq!(move2.city_combat, None);
                } else {
                    assert!(move2.city_combat.is_some());

                    // Since the armor defeated the city, set its production so we can end the turn
                    let conquered_city = move_result.conquered_city().unwrap();
                    let production_set_result = game.set_production_by_loc(
                        secrets[0],
                        conquered_city.loc,
                        UnitType::Fighter,
                    );
                    assert_eq!(production_set_result, Ok(Some(UnitType::Carrier)));
                }
            } else {
                // The unit was destroyed
                assert_eq!(move_result.unit.moves_remaining(), 1);
            }

            let result = game.end_then_begin_turn(secrets[0], secrets[1]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1);

            let result = game.end_then_begin_turn(secrets[1], secrets[0]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 0);
        }
    }

    #[test]
    fn test_terrainwise_movement() {
        let mut map = MapData::try_from(" t-").unwrap();
        map.set_terrain(Location::new(1, 0), Terrain::Water)
            .unwrap();

        let transport_id = map.toplevel_unit_by_loc(Location::new(1, 0)).unwrap().id;

        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);

        game.move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Left)
            .unwrap();
        game.move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Right)
            .unwrap();

        assert_eq!(
            game.move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Right),
            Err(GameError::MoveError(MoveError::NoRoute {
                id: transport_id,
                src: Location::new(1, 0),
                dest: Location::new(2, 0),
            }))
        );
    }

    #[test]
    fn test_unit_moves_onto_transport() {
        let map = MapData::try_from("---it   ").unwrap();
        let infantry_loc = Location { x: 3, y: 0 };
        let transport_loc = Location { x: 4, y: 0 };

        let transport_id: UnitID = map.toplevel_unit_id_by_loc(transport_loc).unwrap();

        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);
        let move_result = game
            .move_toplevel_unit_by_loc(secrets[0], infantry_loc, transport_loc)
            .unwrap();
        assert_eq!(move_result.starting_loc, infantry_loc);
        assert_eq!(move_result.ending_loc(), Some(transport_loc));
        assert!(move_result.moved_successfully());
        assert_eq!(move_result.ending_carrier(), Some(transport_id));
    }

    #[test]
    fn test_loaded_transport_attack() {
        let mut victorious = false;
        let mut defeated = false;
        while !victorious || !defeated {
            let map = MapData::try_from("itP").unwrap();

            let infantry_id = map.toplevel_unit_by_loc(Location::new(0, 0)).unwrap().id;
            let transport_id = map.toplevel_unit_by_loc(Location::new(1, 0)).unwrap().id;
            let battleship_id = map.toplevel_unit_by_loc(Location::new(2, 0)).unwrap().id;

            let (mut game, secrets) = Game::new_with_map(map, 2, false, None, Wrap2d::NEITHER);

            // Load the infantry onto the transport
            let inf_move = game
                .move_unit_by_id_in_direction(secrets[0], infantry_id, Direction::Right)
                .unwrap();
            assert!(inf_move.moved_successfully());
            assert_eq!(
                inf_move.ending_loc(),
                game.current_player_unit_loc(transport_id)
            );
            assert_eq!(inf_move.ending_carrier(), Some(transport_id));

            // Attack the battleship with the transport
            let move_ = game
                .move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Right)
                .unwrap();
            if move_.moved_successfully() {
                victorious = true;

                assert!(game
                    .current_player_units()
                    .any(|unit| unit.id == infantry_id));
                assert!(game
                    .current_player_units()
                    .any(|unit| unit.id == transport_id));

                assert_eq!(
                    game.current_player_unit_by_id(infantry_id).unwrap().loc,
                    Location::new(2, 0)
                );
                assert_eq!(
                    game.current_player_unit_by_id(transport_id).unwrap().loc,
                    Location::new(2, 0)
                );

                assert_eq!(
                    game.current_player_tile(Location::new(0, 0))
                        .unwrap()
                        .unit
                        .as_ref(),
                    None
                );
                assert_eq!(
                    game.current_player_tile(Location::new(1, 0))
                        .unwrap()
                        .unit
                        .as_ref(),
                    None
                );
                {
                    let unit = game
                        .current_player_tile(Location::new(2, 0))
                        .unwrap()
                        .unit
                        .as_ref()
                        .unwrap();
                    assert_eq!(unit.type_, UnitType::Transport);
                    assert_eq!(unit.id, transport_id);
                    assert!(unit
                        .carried_units()
                        .any(|carried_unit| carried_unit.id == infantry_id));
                }

                game.force_end_then_begin_turn(secrets[0], secrets[1])
                    .unwrap(); // ignore remaining moves

                assert!(!game
                    .current_player_units()
                    .any(|unit| unit.id == battleship_id));
                assert!(!game
                    .current_player_unit_orders_requests()
                    .any(|unit_id| unit_id == battleship_id));
            } else {
                defeated = true;

                assert!(!game
                    .current_player_units()
                    .any(|unit| unit.id == infantry_id));
                assert!(!game
                    .current_player_units()
                    .any(|unit| unit.id == transport_id));

                assert_eq!(game.current_player_unit_by_id(infantry_id), None);
                assert_eq!(game.current_player_unit_by_id(transport_id), None);

                assert_eq!(
                    game.current_player_tile(Location::new(0, 0))
                        .unwrap()
                        .unit
                        .as_ref(),
                    None
                );
                assert_eq!(
                    game.current_player_tile(Location::new(1, 0))
                        .unwrap()
                        .unit
                        .as_ref(),
                    None
                );
                assert_eq!(
                    game.current_player_tile(Location::new(2, 0))
                        .unwrap()
                        .unit
                        .as_ref()
                        .unwrap()
                        .id,
                    battleship_id
                );

                game.end_then_begin_turn(secrets[0], secrets[1]).unwrap();

                assert!(game
                    .current_player_units()
                    .any(|unit| unit.id == battleship_id));
                assert!(game
                    .current_player_unit_orders_requests()
                    .any(|unit_id| unit_id == battleship_id));
            }
        }
    }

    #[test]
    fn test_set_orders() {
        let map = MapData::try_from("i").unwrap();
        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);
        let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();

        assert_eq!(
            game.current_player_unit_by_id(unit_id).unwrap().orders,
            None
        );
        assert_eq!(
            game.current_player_unit_by_id(unit_id).unwrap().name(),
            &String::from("Unit_0_0")
        );

        game.set_orders(secrets[0], unit_id, Orders::Sentry)
            .unwrap();

        assert_eq!(
            game.current_player_unit_by_id(unit_id).unwrap().orders,
            Some(Orders::Sentry)
        );
    }

    #[test]
    pub fn test_order_unit_explore() {
        let map = MapData::try_from("i--------------------").unwrap();
        let (mut game, secrets) = Game::new_with_map(map, 1, true, None, Wrap2d::NEITHER);

        let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();

        let outcome = game.order_unit_explore(secrets[0], unit_id).unwrap();
        assert_eq!(outcome.ordered_unit.id, unit_id);
        assert_eq!(outcome.orders, Orders::Explore);
        assert_eq!(outcome.status, OrdersStatus::InProgress);
    }

    #[test]
    pub fn test_propose_move_unit_by_id() {
        super::test_support::test_propose_move_unit_by_id();
    }

    #[test]
    pub fn test_current_player_unit_legal_one_step_destinations() {
        let dirs = [
            Direction::UpLeft,
            Direction::Up,
            Direction::UpRight,
            Direction::Left,
            Direction::Right,
            Direction::DownLeft,
            Direction::Down,
            Direction::DownRight,
        ];

        let possible: Vec<char> = " 01iI".chars().collect();
        let mut traversable: HashMap<char, bool> = HashMap::new();
        traversable.insert(' ', false); //water
        traversable.insert('0', true); //friendly city
        traversable.insert('1', true); //enemy city
        traversable.insert('i', false); //friendly unit
        traversable.insert('I', true); //enemy unit

        for up_left in &possible {
            for up in &possible {
                for up_right in &possible {
                    for left in &possible {
                        for right in &possible {
                            for down_left in &possible {
                                for down in &possible {
                                    for down_right in &possible {
                                        let cs: Vec<char> = dirs
                                            .iter()
                                            .map(|dir| match dir {
                                                Direction::UpLeft => up_left,
                                                Direction::Up => up,
                                                Direction::UpRight => up_right,
                                                Direction::Left => left,
                                                Direction::Right => right,
                                                Direction::DownLeft => down_left,
                                                Direction::Down => down,
                                                Direction::DownRight => down_right,
                                            })
                                            .cloned()
                                            .collect();

                                        let s = format!(
                                            "{}{}{}\n{}i{}\n{}{}{}",
                                            cs[0], cs[1], cs[2], cs[3], cs[4], cs[5], cs[6], cs[7]
                                        );

                                        let map = MapData::try_from(s.clone()).unwrap();
                                        assert_eq!(map.dims(), Dims::new(3, 3));

                                        let (game, secrets) =
                                            Game::new_with_map(map, 2, false, None, Wrap2d::BOTH);

                                        let id = game
                                            .current_player_toplevel_unit_by_loc(Location {
                                                x: 1,
                                                y: 1,
                                            })
                                            .unwrap()
                                            .id;

                                        let inclusions: Vec<bool> = cs
                                            .iter()
                                            .map(|c| traversable.get(&c).unwrap())
                                            .cloned()
                                            .collect();

                                        assert_eq!(cs.len(), inclusions.len());
                                        assert_eq!(cs.len(), dirs.len());

                                        let src = Location::new(1, 1);
                                        let dests: HashSet<Location> = game
                                            .player_unit_legal_one_step_destinations(secrets[0], id)
                                            .unwrap();

                                        for (i, loc) in dirs
                                            .iter()
                                            .map(|dir| {
                                                let v: Vec2d<i32> = (*dir).into();
                                                Location {
                                                    x: ((src.x as i32) + v.x) as u16,
                                                    y: ((src.y as i32) + v.y) as u16,
                                                }
                                            })
                                            .enumerate()
                                        {
                                            if inclusions[i] {
                                                assert!(
                                                    dests.contains(&loc),
                                                    "Erroneously omitted {:?} on \"{}\"",
                                                    loc,
                                                    s.replace("\n", "\\n")
                                                );
                                            } else {
                                                assert!(
                                                    !dests.contains(&loc),
                                                    "Erroneously included {:?} on \"{}\"",
                                                    loc,
                                                    s.replace("\n", "\\n")
                                                );
                                            }
                                        }
                                    } // down_right
                                } // down
                            } // down_left
                        } // right
                    } // left
                } // up_right
            } // up
        } // up_left
    }

    #[test]
    fn test_current_player_unit_legal_one_step_destinations_wrapping() {
        // Make sure the same destinations are found in these cases regardless of wrapping
        for wrapping in Wrap2d::values().iter().cloned() {
            {
                // 1x1
                let mut map = MapData::new(Dims::new(1, 1), |_loc| Terrain::Land);
                let unit_id = map
                    .new_unit(
                        Location::new(0, 0),
                        UnitType::Infantry,
                        Alignment::Belligerent { player: 0 },
                        "Eunice",
                    )
                    .unwrap();
                let (game, secrets) = Game::new_with_map(map, 1, false, None, wrapping);

                assert!(game
                    .player_unit_legal_one_step_destinations(secrets[0], unit_id)
                    .unwrap()
                    .is_empty());
            }

            {
                // 2x1
                let mut map = MapData::new(Dims::new(2, 1), |_loc| Terrain::Land);
                let unit_id = map
                    .new_unit(
                        Location::new(0, 0),
                        UnitType::Infantry,
                        Alignment::Belligerent { player: 0 },
                        "Eunice",
                    )
                    .unwrap();
                let (game, secrets) = Game::new_with_map(map, 1, false, None, wrapping);

                let dests: HashSet<Location> = game
                    .player_unit_legal_one_step_destinations(secrets[0], unit_id)
                    .unwrap();
                assert_eq!(
                    dests.len(),
                    1,
                    "Bad dests: {:?} with wrapping {:?}",
                    dests,
                    wrapping
                );
                assert!(dests.contains(&Location::new(1, 0)));
            }

            {
                // 3x1
                let mut map = MapData::new(Dims::new(3, 1), |_loc| Terrain::Land);
                let unit_id = map
                    .new_unit(
                        Location::new(1, 0),
                        UnitType::Infantry,
                        Alignment::Belligerent { player: 0 },
                        "Eunice",
                    )
                    .unwrap();
                let (game, secrets) = Game::new_with_map(map, 1, false, None, wrapping);

                let dests: HashSet<Location> = game
                    .player_unit_legal_one_step_destinations(secrets[0], unit_id)
                    .unwrap();
                assert_eq!(
                    dests.len(),
                    2,
                    "Bad dests: {:?} with wrapping {:?}",
                    dests,
                    wrapping
                );
                assert!(dests.contains(&Location::new(0, 0)));
                assert!(dests.contains(&Location::new(2, 0)));
            }

            {
                // 3x1 with infantry in transport
                let mut map = MapData::try_from(".ti").unwrap();
                let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
                let inf_id = map.toplevel_unit_id_by_loc(Location::new(2, 0)).unwrap();
                map.carry_unit_by_id(transport_id, inf_id).unwrap();

                let (game, secrets) = Game::new_with_map(map, 1, false, None, wrapping);

                let dests: HashSet<Location> = game
                    .player_unit_legal_one_step_destinations(secrets[0], inf_id)
                    .unwrap();
                assert_eq!(
                    dests.len(),
                    2,
                    "Bad dests: {:?} with wrapping {:?}",
                    dests,
                    wrapping
                );
                assert!(dests.contains(&Location::new(0, 0)));
                assert!(dests.contains(&Location::new(2, 0)));
            }
        }
    }

    #[test]
    pub fn test_one_step_routes() {
        let mut map = MapData::new(Dims::new(10, 10), |_loc| Terrain::Land);
        let unit_id = map
            .new_unit(
                Location::new(0, 0),
                UnitType::Armor,
                Alignment::Belligerent { player: 0 },
                "Forest Gump",
            )
            .unwrap();

        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);

        // let mut rand = thread_rng();

        for (i, src) in game.dims().iter_locs().enumerate() {
            // for _ in 0..1000 {
            //     let src = game.dims().sample(&mut rand);

            // // Recenter the unit on `src`
            // game.map.relocate_unit_by_id(unit_id, src).unwrap();

            // Recenter the unit on `src`
            if i > 0 {
                game.move_unit_by_id(secrets[0], unit_id, src).unwrap();
                game.order_unit_skip(secrets[0], unit_id).unwrap();
                game.end_then_begin_turn(secrets[0], secrets[0]).unwrap();
            }

            for dir in Direction::values().iter().cloned() {
                let src = game.current_player_unit_loc(unit_id).unwrap();
                let dest = game
                    .wrapping
                    .wrapped_add(game.dims(), src, dir.into())
                    .unwrap();

                game.move_unit_by_id(secrets[0], unit_id, dest).expect(
                    format!(
                        "Error moving unit with ID {:?} from {} to {}",
                        unit_id, src, dest
                    )
                    .as_str(),
                );
                assert_eq!(
                    game.current_player_unit_loc(unit_id),
                    Some(dest),
                    "Wrong location after moving {:?} from {:?} to {:?}",
                    dir,
                    src,
                    dest
                );

                game.move_unit_by_id(secrets[0], unit_id, src).expect(
                    format!(
                        "Error moving unit with ID {:?} from {} to {}",
                        unit_id, dest, src
                    )
                    .as_str(),
                );
                game.end_then_begin_turn(secrets[0], secrets[0]).unwrap();

                game.move_unit_by_id_in_direction(secrets[0], unit_id, dir)
                    .unwrap();
                assert_eq!(
                    game.current_player_unit_loc(unit_id),
                    Some(dest),
                    "Wrong location after moving {:?} from {:?} to {:?}",
                    dir,
                    src,
                    dest
                );

                game.move_unit_by_id_in_direction(secrets[0], unit_id, dir.opposite())
                    .unwrap();
                game.end_then_begin_turn(secrets[0], secrets[0]).unwrap();
            }
        }
    }

    #[test]
    pub fn test_order_unit_skip() {
        let mut map = MapData::new(Dims::new(10, 10), |_loc| Terrain::Land);
        let unit_id = map
            .new_unit(
                Location::new(0, 0),
                UnitType::Infantry,
                Alignment::Belligerent { player: 0 },
                "Skipper",
            )
            .unwrap();
        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);

        game.move_unit_by_id_in_direction(secrets[0], unit_id, Direction::Right)
            .unwrap();
        game.end_then_begin_turn(secrets[0], secrets[0]).unwrap();

        game.order_unit_skip(secrets[0], unit_id).unwrap();
        game.end_then_begin_turn(secrets[0], secrets[0]).unwrap();

        assert_eq!(
            game.current_player_unit_orders_requests().next(),
            Some(unit_id)
        );

        game.current_player_unit_by_id(unit_id).unwrap();
    }

    #[test]
    pub fn test_movement_matches_carry_status() {
        let l1 = Location::new(0, 0);
        let l2 = Location::new(1, 0);
        let a = Alignment::Belligerent { player: 0 };

        for type1 in UnitType::values().iter().cloned() {
            let u1 = Unit::new(UnitID::new(0), l2, type1, a, "u1");

            for type2 in UnitType::values().iter().cloned() {
                let u2 = Unit::new(UnitID::new(1), l2, type2, a, "u2");

                let mut map = MapData::new(Dims::new(2, 1), |loc| {
                    let mode = if loc == l1 {
                        u1.transport_mode()
                    } else {
                        u2.transport_mode()
                    };

                    match mode {
                        TransportMode::Sea => Terrain::Water,
                        TransportMode::Land => Terrain::Land,
                        TransportMode::Air => Terrain::Land,
                    }
                });

                map.set_unit(l1, u1.clone());
                map.set_unit(l2, u2.clone());

                let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);
                let result = game.move_unit_by_id_in_direction(secrets[0], u1.id, Direction::Right);

                if u2.can_carry_unit(&u1) {
                    assert!(result.is_ok());
                } else {
                    assert!(result.is_err());
                }
            }
        }
    }

    #[test]
    pub fn test_id_consistency() {
        let mut loc = Location::new(0, 0);

        let mut map = MapData::new(Dims::new(10, 1), |_| Terrain::Water);
        let unit_id = map
            .new_unit(
                loc,
                UnitType::Submarine,
                Alignment::Belligerent { player: 0 },
                "K-19",
            )
            .unwrap();

        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        for _ in 0..9 {
            game.move_unit_by_id_in_direction(secrets[0], unit_id, Direction::Right)
                .unwrap();
            loc = loc
                .shift_wrapped(Direction::Right, game.dims(), game.wrapping())
                .unwrap();

            let unit = game.current_player_toplevel_unit_by_loc(loc).unwrap();
            assert_eq!(unit.id, unit_id);

            game.force_end_then_begin_turn(secrets[0], secrets[0])
                .unwrap();
        }
    }

    #[test]
    fn test_transport_moves_on_transport_unloaded() {
        let l1 = Location::new(0, 0);
        let l2 = Location::new(1, 0);

        let map = MapData::try_from("tt").unwrap();

        let t1_id = map.toplevel_unit_id_by_loc(l1).unwrap();

        {
            let (mut game, secrets) =
                Game::new_with_map(map.clone(), 1, false, None, Wrap2d::NEITHER);

            game.move_unit_by_id_in_direction(secrets[0], t1_id, Direction::Right)
                .expect_err("Transport should not be able to move onto transport");
        }

        let mut map2 = map.clone();
        map2.new_city(l2, Alignment::Belligerent { player: 0 }, "city")
            .unwrap();

        let (mut game, secrets) = Game::new_with_map(map2, 1, false, None, Wrap2d::NEITHER);

        game.move_unit_by_id_in_direction(secrets[0], t1_id, Direction::Right)
            .expect_err("Transport should not be able to move onto transport");
    }

    #[test]
    fn test_transport_moves_on_transport_loaded() {
        let l1 = Location::new(1, 0);
        let l2 = Location::new(2, 0);

        let mut map = MapData::try_from(".tt.").unwrap();

        let t1_id = map.toplevel_unit_id_by_loc(l1).unwrap();
        let t2_id = map.toplevel_unit_id_by_loc(l2).unwrap();

        for i in 0..3 {
            println!("{}", i);
            let id = map
                .new_unit(
                    Location::new(0, 0),
                    UnitType::Infantry,
                    Alignment::Belligerent { player: 0 },
                    format!("inf{}", i),
                )
                .unwrap();
            map.carry_unit_by_id(t1_id, id).unwrap();
        }

        for i in 0..3 {
            let id = map
                .new_unit(
                    Location::new(3, 0),
                    UnitType::Infantry,
                    Alignment::Belligerent { player: 0 },
                    format!("inf{}", i + 100),
                )
                .unwrap();
            map.carry_unit_by_id(t2_id, id).unwrap();
        }

        {
            let (mut game, secrets) =
                Game::new_with_map(map.clone(), 1, false, None, Wrap2d::NEITHER);

            game.move_unit_by_id_in_direction(secrets[0], t1_id, Direction::Right)
                .expect_err("Transport should not be able to move onto transport");

            game.move_unit_by_id_in_direction(secrets[0], t2_id, Direction::Left)
                .expect_err("Transport should not be able to move onto transport");
        }

        let mut map2 = map.clone();
        map2.new_city(l2, Alignment::Belligerent { player: 0 }, "city")
            .unwrap();

        let (mut game, secrets) = Game::new_with_map(map2, 1, false, None, Wrap2d::NEITHER);

        game.move_unit_by_id_in_direction(secrets[0], t1_id, Direction::Right)
            .expect_err("Transport should not be able to move onto transport");
    }

    #[test]
    fn test_embark_disembark() {
        let map = MapData::try_from("at -").unwrap();
        let armor_id = map.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
        let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        // Embark
        game.move_unit_by_id_in_direction(secrets[0], armor_id, Direction::Right)
            .unwrap();
        assert_eq!(
            game.current_player_unit_loc(armor_id),
            Some(Location::new(1, 0))
        );
        assert_eq!(
            game.current_player_unit_loc(transport_id),
            Some(Location::new(1, 0))
        );

        // Move transport
        game.move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Right)
            .unwrap();
        assert_eq!(
            game.current_player_unit_loc(armor_id),
            Some(Location::new(2, 0))
        );
        assert_eq!(
            game.current_player_unit_loc(transport_id),
            Some(Location::new(2, 0))
        );

        // Disembark
        game.move_unit_by_id_in_direction(secrets[0], armor_id, Direction::Right)
            .unwrap();
        assert_eq!(
            game.current_player_unit_loc(armor_id),
            Some(Location::new(3, 0))
        );
        assert_eq!(
            game.current_player_unit_loc(transport_id),
            Some(Location::new(2, 0))
        );
    }

    #[test]
    fn test_embark_disembark_via_goto() {
        let map = MapData::try_from("at -").unwrap();
        let armor_id = map.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
        let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        // Embark
        game.order_unit_go_to(secrets[0], armor_id, Location::new(1, 0))
            .unwrap();
        assert_eq!(
            game.current_player_unit_loc(armor_id),
            Some(Location::new(1, 0))
        );
        assert_eq!(
            game.current_player_unit_loc(transport_id),
            Some(Location::new(1, 0))
        );

        // Move transport
        game.order_unit_go_to(secrets[0], transport_id, Location::new(2, 0))
            .unwrap();
        assert_eq!(
            game.current_player_unit_loc(armor_id),
            Some(Location::new(2, 0))
        );
        assert_eq!(
            game.current_player_unit_loc(transport_id),
            Some(Location::new(2, 0))
        );

        // Disembark
        game.order_unit_go_to(secrets[0], armor_id, Location::new(3, 0))
            .unwrap();
        assert_eq!(
            game.current_player_unit_loc(armor_id),
            Some(Location::new(3, 0))
        );
        assert_eq!(
            game.current_player_unit_loc(transport_id),
            Some(Location::new(2, 0))
        );
    }

    #[test]
    fn test_shortest_paths_carrying() {
        let map = MapData::try_from("t t  ").unwrap();

        let (mut game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        game.move_toplevel_unit_by_loc(secrets[0], Location::new(0, 0), Location::new(4, 0))
            .expect_err("Transports shouldn't traverse transports on their way somewhere");
    }

    #[test]
    fn test_valid_productions() {
        let map = MapData::try_from("...\n.0.\n...").unwrap();
        let (game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        let city_loc = game
            .current_player_production_set_requests()
            .next()
            .unwrap();

        let prods: HashSet<UnitType> = game
            .valid_productions(secrets[0], city_loc)
            .unwrap()
            .collect();

        for t in UnitType::values().iter().cloned() {
            if match t {
                UnitType::Armor => true,
                UnitType::Battleship => false,
                UnitType::Bomber => true,
                UnitType::Carrier => false,
                UnitType::Cruiser => false,
                UnitType::Destroyer => false,
                UnitType::Fighter => true,
                UnitType::Infantry => true,
                UnitType::Submarine => false,
                UnitType::Transport => false,
            } {
                assert!(prods.contains(&t));
            } else {
                assert!(!prods.contains(&t));
            }
        }
    }

    #[test]
    fn test_valid_productions_conservative() {
        let map = MapData::try_from("...\n.0.\n...").unwrap();
        let (game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        let city_loc = game
            .current_player_production_set_requests()
            .next()
            .unwrap();

        let prods: HashSet<UnitType> = game
            .valid_productions_conservative(secrets[0], city_loc)
            .unwrap()
            .collect();

        for t in UnitType::values().iter().cloned() {
            if match t {
                UnitType::Armor => true,
                UnitType::Battleship => false,
                UnitType::Bomber => true,
                UnitType::Carrier => false,
                UnitType::Cruiser => false,
                UnitType::Destroyer => false,
                UnitType::Fighter => true,
                UnitType::Infantry => true,
                UnitType::Submarine => false,
                UnitType::Transport => false,
            } {
                assert!(prods.contains(&t));
            } else {
                assert!(!prods.contains(&t));
            }
        }
    }

    #[test]
    fn test_move_fighter_over_water() {
        let mut map = MapData::new(Dims::new(180, 90), |_| Terrain::Water);
        let unit_id = map
            .new_unit(
                Location::new(0, 0),
                UnitType::Fighter,
                Alignment::Belligerent { player: 0 },
                "Han Solo",
            )
            .unwrap();

        let (mut game, secrets) = Game::new_with_map(map, 1, true, None, Wrap2d::BOTH);

        let unit_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
        let dest = game
            .wrapping()
            .wrapped_add(game.dims(), unit_loc, Vec2d::new(5, 5))
            .unwrap();
        game.move_unit_by_id(secrets[0], unit_id, dest).unwrap();
    }

    #[test]
    fn test_disband_unit_by_id() {
        {
            let map = MapData::try_from("i").unwrap();
            let (mut game, secrets) = Game::new_with_map(map, 1, true, None, Wrap2d::NEITHER);
            let id = UnitID::new(0);

            let unit = game.current_player_unit_by_id(id).cloned().unwrap();

            assert!(game
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == id)
                .is_some());

            assert_eq!(game.disband_unit_by_id(secrets[0], id), Ok(unit));

            let id2 = UnitID::new(1);

            assert_eq!(
                game.disband_unit_by_id(secrets[0], id2),
                Err(GameError::NoSuchUnit { id: id2 })
            );

            assert!(game
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == id)
                .is_none());
        }

        {
            let map2 = MapData::try_from("it ").unwrap();
            let infantry_id = map2.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
            let transport_id = map2.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();

            let (mut game2, secrets) = Game::new_with_map(map2, 1, true, None, Wrap2d::NEITHER);

            assert!(game2
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == infantry_id)
                .is_some());
            assert!(game2
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == transport_id)
                .is_some());

            game2
                .move_unit_by_id_in_direction(secrets[0], infantry_id, Direction::Right)
                .unwrap();

            game2
                .force_end_then_begin_turn(secrets[0], secrets[0])
                .unwrap();

            let infantry = game2
                .current_player_unit_by_id(infantry_id)
                .cloned()
                .unwrap();

            assert!(game2
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == infantry_id)
                .is_some());
            assert!(game2
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == transport_id)
                .is_some());

            assert_eq!(
                game2.disband_unit_by_id(secrets[0], infantry_id),
                Ok(infantry)
            );

            assert!(game2
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == infantry_id)
                .is_none());
            assert!(game2
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == transport_id)
                .is_some());

            let transport = game2
                .current_player_unit_by_id(transport_id)
                .cloned()
                .unwrap();

            assert_eq!(
                game2.disband_unit_by_id(secrets[0], transport_id),
                Ok(transport)
            );

            assert!(game2
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == infantry_id)
                .is_none());
            assert!(game2
                .current_player_unit_orders_requests()
                .find(|unit_id| *unit_id == transport_id)
                .is_none());
        }

        {
            let map = MapData::try_from("ii").unwrap();
            let a = map.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
            let b = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();

            let (mut game, secrets) = Game::new_with_map(map, 1, true, None, Wrap2d::NEITHER);

            assert!(game.disband_unit_by_id(secrets[0], a).is_ok());

            assert!(game.current_player_unit_by_id(a).is_none());

            assert!(game
                .move_unit_by_id_in_direction(secrets[0], b, Direction::Left)
                .is_ok());
        }
    }
}
