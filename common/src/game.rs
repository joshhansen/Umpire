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
mod igameimpl;
pub mod map;
pub mod move_;
pub mod obs;
pub mod player;
pub mod proposed;
pub mod traits;
pub mod turn;
pub mod turn_async;
pub mod unit;

use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, RwLock},
};

use rsrl::DerefVec;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as RwLockTokio;
use uuid::Uuid;

use crate::{
    game::{
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

pub use self::player::{PlayerNum, PlayerType};

use self::{
    action::{Actionable, PlayerAction, PlayerActionOutcome},
    ai::{fX, TrainingFocus, FEATS_LEN},
    alignment::{Aligned, AlignedMaybe},
    move_::{Move, MoveComponent, MoveError},
    obs::{LocatedObs, LocatedObsLite},
    player::PlayerControl,
    proposed::Proposed2,
};

pub use self::traits::IGame;

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

/// What turn is it? The round of play, in other words.
pub type TurnNum = u64;

/// The global count of actions taken in the game
///
/// A more granular way of keeping time than turns
pub type ActionNum = u64;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductionCleared {
    pub prior_production: Option<UnitType>,
    pub obs: LocatedObsLite,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct TurnStart {
    pub turn: TurnNum,
    pub current_player: PlayerNum,
    pub orders_results: Vec<OrdersResult>,
    pub production_outcomes: Vec<UnitProductionOutcome>,
    pub observations: Vec<LocatedObs>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitDisbanded {
    pub unit: Unit,
    pub obs: LocatedObsLite,
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum TurnPhase {
    Pre,
    Main,
}

/// The core engine that enforces Umpire's game rules
#[derive(Clone)]
pub struct Game {
    /// The underlying state of the game
    map: MapData,

    player_observations: PlayerObsTracker,

    /// Observations the player has made, but which the player hasn't viewed yet
    ///
    /// More or less a per-player observation queue
    player_pending_observations: Vec<Vec<LocatedObsLite>>,

    /// The turn that it is right now
    turn: TurnNum,

    turn_phase: TurnPhase,

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

    /// The total number of actions taken during the game, by all players
    action_count: ActionNum,

    /// Action counts
    ///
    /// How many actions has each player taken? Used for score calculation.
    ///
    /// An action is basically every city production request and unit orders request taken.
    action_counts: Vec<ActionNum>,

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
        let player_pending_observations = (0..num_players).map(|_| Vec::new()).collect();

        let mut game = Self {
            map,
            player_observations,
            player_pending_observations,
            turn: 0,
            turn_phase: TurnPhase::Pre,
            num_players,
            player_secrets: Vec::new(),
            current_player: 0,
            wrapping,
            unit_namer: unit_namer.unwrap_or(Arc::new(RwLock::new(IntNamer::new("unit")))),
            fog_of_war,
            action_count: 0,
            action_counts: vec![0; num_players],
            defeated_unit_hitpoints: vec![0; num_players],
        };

        let secrets: Vec<PlayerSecret> = (0..num_players)
            .map(|_player| game.register_player().unwrap())
            .collect();

        (game, secrets)
    }

    /// Set up a sharable game instance and return it and controls for each player
    pub async fn setup_with_map(
        map: MapData,
        num_players: PlayerNum,
        fog_of_war: bool,
        unit_namer: Option<Arc<RwLock<dyn Namer>>>,
        wrapping: Wrap2d,
    ) -> (Arc<RwLockTokio<Self>>, Vec<PlayerControl>) {
        let (game, secrets) =
            Self::new_with_map(map, num_players, fog_of_war, unit_namer, wrapping);

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

        (game, ctrls)
    }

    pub fn num_players(&self) -> PlayerNum {
        self.num_players
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

    fn validate_is_player_turn_pre_phase(&self, secret: PlayerSecret) -> UmpireResult<PlayerNum> {
        let player = self.validate_is_player_turn(secret)?;

        match self.turn_phase {
            TurnPhase::Pre => Ok(player),
            TurnPhase::Main => Err(GameError::WrongPhase {
                turn: self.turn,
                player,
                phase: self.turn_phase,
            }),
        }
    }

    fn validate_is_player_turn_main_phase(&self, secret: PlayerSecret) -> UmpireResult<PlayerNum> {
        let player = self.validate_is_player_turn(secret)?;

        match self.turn_phase {
            TurnPhase::Pre => Err(GameError::WrongPhase {
                turn: self.turn,
                player,
                phase: self.turn_phase,
            }),
            TurnPhase::Main => Ok(player),
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

    /// Reset unit moves remaining and send updated observations
    fn refresh_moves_remaining(&mut self, player_secret: PlayerSecret) -> UmpireResult<()> {
        let player = self.validate_is_player_turn(player_secret)?;

        self.map.refresh_player_unit_moves_remaining(player);

        // Since their moves remaining changed, refresh observations of the units
        let unit_locs: HashSet<Location> = self
            .player_units_by_idx(player)
            .map(|unit| unit.loc)
            .collect();

        for loc in unit_locs {
            self.observe(loc)?;
        }

        Ok(())
    }

    /// Mark for accounting purposes that the player took an action
    fn action_taken(&mut self, player: PlayerNum) {
        self.action_count += 1;
        self.action_counts[player] += 1;
    }

    pub fn current_turn_begun(&self) -> bool {
        self.turn_phase == TurnPhase::Main
    }

    /// Run the initial phase of the player's turn, producing units, refreshing moves remaining, and making
    /// observations.
    ///
    /// ## Errors
    /// * GameError::NotPlayersTurn
    /// * GameError::TurnAlreadyBegun
    pub fn begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        let player = self.validate_is_player_turn_pre_phase(player_secret)?;

        // "Beginning" a turn is what moves us from Pre to Main phase
        self.turn_phase = TurnPhase::Main;

        let production_outcomes = self.produce_units(player_secret)?;

        if clear_after_unit_production {
            for prod in production_outcomes.iter() {
                if let UnitProductionOutcome::UnitProduced { city, .. } = prod {
                    self.clear_production(player_secret, city.loc, false)
                        .unwrap();
                }
            }
        }

        self.refresh_moves_remaining(player_secret)?;

        let observations = self.update_player_observations(player);

        let orders_results = self.follow_pending_orders(player_secret)?;

        Ok(TurnStart {
            turn: self.turn,
            current_player: self.current_player,
            orders_results,
            production_outcomes,
            observations,
        })
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

        if self.turn_phase == TurnPhase::Pre {
            return Ok(false);
        }

        // self.current_player == player
        // In this case the turn is considered done if there are no production or orders requests remaining
        self.player_production_set_requests_by_idx(player)
            .map(|mut rqsts| {
                rqsts.next().is_none()
                    && self
                        .player_unit_orders_requests_by_idx(player)
                        .next()
                        .is_none()
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
        self.validate_is_player_turn_main_phase(player_secret)?;

        self.player_observations_mut(player_secret)?.archive();

        self._inc_current_player();

        // The next player's turn starts out in the Pre phase
        self.turn_phase = TurnPhase::Pre;

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
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        self.validate_is_player_turn(player_secret)?;

        if self.current_turn_is_done() {
            Ok(self.force_end_then_begin_turn(
                player_secret,
                next_player_secret,
                clear_after_unit_production,
            )?)
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
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        self.force_end_turn(player_secret)?;

        self.begin_turn(next_player_secret, clear_after_unit_production)
    }

    /// Register the current observations of player units
    ///
    /// This applies only to top-level units. Carried units (e.g. units in a transport or carrier) make no observations
    fn update_player_observations(&mut self, player: PlayerNum) -> Vec<LocatedObs> {
        let obs_tracker = self.player_observations.tracker_mut(player).unwrap();

        if self.fog_of_war {
            let mut observations: Vec<LocatedObs> = Vec::new();
            for city in self.map.player_cities(player) {
                observations.extend(city.observe(
                    &self.map,
                    self.turn,
                    self.action_count,
                    self.wrapping,
                    obs_tracker,
                ));
            }

            for unit in self.map.player_units(player) {
                observations.extend(unit.observe(
                    &self.map,
                    self.turn,
                    self.action_count,
                    self.wrapping,
                    obs_tracker,
                ));
            }

            observations
        } else {
            //FIXME when fog of war is disabled we shouldn't need to track observations at all
            let mut observations: Vec<LocatedObs> = Vec::new();
            for loc in self.map.dims().iter_locs() {
                let tile = self.map.tile(loc).unwrap();
                observations.push(obs_tracker.track_observation(
                    loc,
                    tile,
                    self.turn,
                    self.action_count,
                ));
            }
            observations
        }
    }

    /// Indicate that an observable event has just occurred at the given location
    ///
    /// Observations of the state of the specified tile should be routed to players as appropriate
    /// based on the position of units and fog of war status.
    ///
    /// Returns the LocatedObs (including old_obs) from the perspective of the current player
    ///
    ///
    /// ## Errors
    /// * If the location is out of bounds
    fn observe(&mut self, loc: Location) -> UmpireResult<LocatedObs> {
        let tile = self
            .map
            .tile(loc)
            .ok_or(GameError::NoTileAtLocation { loc })?
            .clone();
        let obs = Obs::Observed {
            tile,
            turn: self.turn,
            action_count: self.action_count,
            current: true,
        };
        let obs_lite = LocatedObsLite::new(loc, obs.clone());
        let mut old_obs = Obs::Unobserved;

        if self.fog_of_war {
            for player in 0..self.num_players {
                // Make the observation available to the player if at least one of its top-level units or cities
                // can see it
                let include = self
                    .player_active_observers_by_idx(player)?
                    .any(|observer| observer.can_see(loc));

                if include {
                    {
                        let obs_queue = &mut self.player_pending_observations[player];
                        obs_queue.push(obs_lite.clone());
                    }

                    // Also keep track on our side
                    let observations = self.player_observations.tracker_mut(player).unwrap();
                    let old_obs_incoming = observations.track_lite(obs_lite.clone());

                    if player == self.current_player {
                        if let Some(old_obs_incoming) = old_obs_incoming {
                            old_obs = old_obs_incoming;
                        }
                    }
                }
            }
        } else {
            // Without fog of war, we give all players all observations
            self.player_pending_observations
                .iter_mut()
                .for_each(|obs_queue| obs_queue.push(obs_lite.clone()));

            // Also keep track on our side
            for player in 0..self.num_players {
                let observations = self.player_observations.tracker_mut(player).unwrap();
                let old_obs_incoming = observations.track_lite(obs_lite.clone());

                if player == self.current_player {
                    if let Some(old_obs_incoming) = old_obs_incoming {
                        old_obs = old_obs_incoming;
                    }
                }
            }
        }

        Ok(LocatedObs::new(loc, obs, old_obs))
    }

    /// The observers belonging to a player that can currently make observation
    ///
    /// This consists of all cities, and toplevel (non-carried) units
    fn player_active_observers_by_idx(
        &self,
        player: PlayerNum,
    ) -> UmpireResult<impl Iterator<Item = &dyn Observer>> {
        self.validate_player_num(player)?;

        Ok(self
            .map
            .player_toplevel_units(player)
            .map(|unit| unit as &dyn Observer)
            .chain(
                self.player_cities_by_idx(player)?
                    .map(|city| city as &dyn Observer),
            ))
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

    #[cfg(test)]
    fn player_action_count(&self, player_secret: PlayerSecret) -> UmpireResult<ActionNum> {
        let player = self.player_with_secret(player_secret)?;

        Ok(self.action_counts[player])
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
        self.player_with_secret(player_secret)
            .map(|player| self.player_unit_orders_requests_by_idx(player))
    }

    fn player_unit_orders_requests_by_idx<'a>(
        &'a self,
        player: PlayerNum,
    ) -> impl Iterator<Item = UnitID> + 'a {
        self.map
            .player_units(player)
            .filter(|unit| unit.orders.is_none() && unit.moves_remaining() > 0)
            .map(|unit| unit.id)
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
    /// *player_secret* is the secret of the player attempting to make the move; they must control the specified unit.
    ///
    /// Must be player's turn
    fn move_unit_by_id_using_filter<F: Filter<Obs>>(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
        tile_filter: &F,
    ) -> UmpireResult<Move> {
        let player = self.validate_is_player_turn_main_phase(player_secret)?;

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
                let obs_tracker = self.player_observations.tracker_mut(player).unwrap();
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
                if let Some(other_unit) = self.map.toplevel_unit_by_loc(loc) {
                    // If it is a friendly unit:
                    if unit.is_friendly_to(other_unit) {
                        debug_assert_ne!(unit.id, other_unit.id);
                        debug_assert!(other_unit.can_carry_unit(&unit));

                        // the friendly unit must have space for us in its carrying capacity or else the
                        // path search wouldn't have included it
                        move_.carrier = Some(other_unit.id);

                        self.map
                            .carry_unit_by_id(other_unit.id, unit_id)
                            .expect("Could not carry unit for some weird reason");

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

                move_.observations_after_move =
                    vec![self.observe(prev_loc).unwrap(), self.observe(loc).unwrap()];

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

        // ----- Make observations from the unit's new location -----
        let observations_after_move = {
            let obs_tracker = self.player_observations.tracker_mut(player).unwrap();
            unit.observe(
                &self.map,
                self.turn,
                self.action_count,
                self.wrapping,
                obs_tracker,
            )
        };

        let move_ = moves.last_mut().unwrap();

        move_.observations_after_move = observations_after_move;

        // move_
        //     .observations_after_move
        //     .push(self.observe(unit.loc).unwrap());

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

        self.action_taken(player);

        Move::new(unit, src, moves).map_err(GameError::MoveError)
    }

    /// Disbands a unit
    ///
    /// Must be main phase of player's turn
    pub fn disband_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<UnitDisbanded> {
        let player = self.validate_is_player_turn_main_phase(player_secret)?;

        let unit = self
            .map
            .pop_player_unit_by_id(player, unit_id)
            .ok_or(GameError::NoSuchUnit { id: unit_id })?;

        // Mark the action as taken so the change shows up in the observation
        self.action_taken(player);

        // Let everyone in line of sight know the unit is gone
        let obs = self.observe(unit.loc).unwrap().lite();

        // Also explicitly update this player's observations, since its unit was no longer
        // there to see it---otherwise the player's observations continue to show the
        // disbanded unit, even though it's know to the player that it's no longer there.
        self.player_observations_by_idx_mut(player)
            .track_lite(obs.clone());

        Ok(UnitDisbanded { unit, obs })
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
            self.action_taken(player);
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
            self.action_taken(player);
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
    ) -> UmpireResult<ProductionCleared> {
        let city = self.player_city_by_loc(player_secret, loc)?;
        if city.is_none() {
            return Err(GameError::NoCityAtLocation { loc });
        }

        let prior = self
            .map
            .clear_city_production_by_loc(loc, ignore_cleared_production)
            .unwrap();

        let obs = self.observe(loc).unwrap().lite();

        Ok(ProductionCleared {
            prior_production: prior,
            obs,
        })
    }

    /// Clear the production on all cities belonging to the specified player
    pub fn clear_productions<'a>(
        &'a mut self,
        player_secret: PlayerSecret,
        ignore_cleared_production: bool,
    ) -> UmpireResult<impl Iterator<Item = ProductionCleared> + 'a> {
        let player = self.validate_is_player_turn(player_secret)?;

        let city_locs: Vec<Location> = self
            .map
            .player_cities(player)
            .map(|city| city.loc)
            .collect();

        Ok(city_locs.into_iter().map(move |city_loc| {
            self.clear_production(player_secret, city_loc, ignore_cleared_production)
                .unwrap()
        }))
    }

    pub fn turn(&self) -> TurnNum {
        self.turn
    }

    pub fn turn_phase(&self) -> TurnPhase {
        self.turn_phase
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

        self.action_taken(player);

        Ok(OrdersOutcome::completed_without_move(ordered_unit, orders))
    }

    pub fn order_unit_skip(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        let player = self.player_with_secret(player_secret)?;

        let orders = Orders::Skip;
        let unit = self
            .player_unit_by_id(player_secret, unit_id)?
            .unwrap()
            .clone();

        let result = self
            .set_orders(player_secret, unit_id, orders)
            .map(|_| OrdersOutcome::in_progress_without_move(unit, orders));

        if result.is_ok() {
            self.action_taken(player);
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
    ///
    /// Returns a fresh observation of the location
    pub fn activate_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<LocatedObsLite> {
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

        self.map.activate_player_unit(player, unit_id)?;

        Ok(self.observe(loc).unwrap().lite())
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
        let player = self.validate_is_player_turn(player_secret)?;

        self.set_orders(player_secret, id, orders)?;

        let result = self.follow_unit_orders(player_secret, id);

        if result.is_ok() {
            self.action_taken(player);
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

    pub fn take_action<A: Actionable>(
        &mut self,
        player_secret: PlayerSecret,
        action: A,
    ) -> UmpireResult<PlayerActionOutcome> {
        action
            .to_action(self, player_secret)?
            .take(self, player_secret)
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
    /// Possibly split unit-relevant from city-relevant features
    /// FIXME Maintain this vector in the client, incrementally
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
    pub fn player_features(
        &self,
        player_secret: PlayerSecret,
        focus: TrainingFocus,
    ) -> UmpireResult<Vec<fX>> {
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

        // Relatively positioned around city or unit, depending on the training focus
        let loc =
            match focus {
                TrainingFocus::City => Some(city_loc.expect(
                    "There should be a next city if we're generating a city feature vector",
                )),
                TrainingFocus::Unit => Some(
                    self.player_unit_loc(
                        player_secret,
                        unit_id.expect(
                            "There should be a next unit if we're generating a unit feature vector",
                        ),
                    )?
                    .unwrap(),
                ),
                TrainingFocus::UnitIfExistsElseCity => {
                    if let Some(unit_id) = unit_id {
                        self.player_unit_loc(player_secret, unit_id)?
                    } else {
                        city_loc
                    }
                }
            };

        let mut is_enemy_belligerent = Vec::new();
        let mut is_observed = Vec::new();
        let mut is_neutral = Vec::new();
        let mut is_city = Vec::new();

        let player = self.player_with_secret(player_secret)?;

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

    fn current_player_features(&self, focus: TrainingFocus) -> Vec<fX> {
        let player_secret = self.player_secrets[self.current_player];
        self.player_features(player_secret, focus).unwrap()
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

impl fmt::Debug for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.map.fmt(f)
    }
}

impl DerefVec for Game {
    fn deref_vec(&self) -> Vec<fX> {
        self.current_player_features(TrainingFocus::UnitIfExistsElseCity)
    }
}

pub mod test_support;

#[cfg(test)]
mod tests;
