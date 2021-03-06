//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

pub mod ai;
pub mod city;
pub mod combat;
pub mod map;
pub mod move_;
pub mod obs;
pub mod player;
pub mod unit;

use std::{
    collections::{HashMap,HashSet},
    fmt,
    rc::Rc,
    sync::{
        RwLock,
    },
};

use failure::{
    Fail,
};

use rsrl::DerefVec;

use crate::{
    color::{Colors,Colorized},
    game::{
        city::{CityID,City},
        combat::CombatCapable,
        map::{
            LocationGridI,
            MapData,
            NewUnitError,
            Tile,
            gen::generate_map,
            dijkstra::{
                self,
                AndFilter,
                Filter,
                NoCitiesButOursFilter,
                NoUnitsFilter,
                ShortestPaths,
                Source,
                UnitMovementFilter,
                UnitMovementFilterXenophile,
                directions_unit_could_move_iter,
                neighbors_terrain_only,
                neighbors_unit_could_move_to_iter,
            },
        },
        obs::{Obs,Observer,ObsTracker,ObsTrackerI},
        unit::{
            TransportMode,
            Unit,
            UnitID,
            UnitType,
            orders::{
                Orders,
                OrdersStatus,
                OrdersOutcome,
                OrdersResult,
            },
        },
    },
    name::{
        IntNamer,
        Namer,
    },
    util::{
        Dims,
        Dimensioned,
        Direction,
        Location,
        Vec2d,
        Wrap2d,
    },
};

pub use self::player::{
    PlayerNum,
    PlayerTurnControl,
    PlayerType,
};

use self::{
    ai::dnn::FEATS_LEN,
    move_::{
        Move,
        MoveComponent,
        MoveError,
        MoveResult,
    }
};
use ai::UmpireAction;

static UNIT_TYPES: [UnitType;10] = UnitType::values();

/// How important is a city in and of itself?
const CITY_INTRINSIC_SCORE: f64 = 1000.0;

/// How valuable is it to have observed a tile at all?
const TILE_OBSERVED_BASE_SCORE: f64 = 10.0;

/// How much is each turn taken penalized?
const TURN_PENALTY: f64 = 0.0;//1000.0;

/// How much is each action penalized?
const ACTION_PENALTY: f64 = 100.0;

/// How much is each point of controlled unit production cost (downweighted for reduced HP) worth?
const UNIT_MULTIPLIER: f64 = 100.0;

/// How much is victory worth?
const VICTORY_SCORE: f64 = 1_000_000.0;

type fX = f64;

/// A proposed change to the game state.
/// 
/// `delta` encapsulates the change, and `new_state` is the state as it will be after the change is applied.
pub struct Proposed<T> {
    new_state: Game,
    pub delta: T,
}
impl <T> Proposed<T> {
    pub fn new(new_state: Game, delta: T) -> Self {
        Self {
            new_state, delta,
        }
    }

    /// Apply the proposed change to the given game instance. This overwrites the game instance with `new_state`.
    pub fn apply(self, state: &mut Game) -> T {
        *state = self.new_state;
        self.delta
    }
}

pub type TurnNum = u32;

#[derive(Copy,Clone,Debug,PartialEq,Hash,Eq)]
pub enum Alignment {
    Neutral,
    Belligerent { player: PlayerNum }
    // active neutral, chaotic, etc.
}

impl Alignment {
    fn is_friendly_to(self, other: Alignment) -> bool {
        self == other
    }

    fn is_friendly_to_player(self, player: PlayerNum) -> bool {
        self == Alignment::Belligerent{player}
    }

    fn is_enemy_of(self, other: Alignment) -> bool {
        !self.is_friendly_to(other)
    }

    fn is_enemy_of_player(self, player: PlayerNum) -> bool {
        !self.is_friendly_to_player(player)
    }

    fn is_neutral(self) -> bool {
        self == Alignment::Neutral
    }

    fn is_belligerent(self) -> bool {
        if let Alignment::Belligerent{..} = self {
            true
        } else {
            false
        }
    }
}

impl Colorized for Alignment {
    fn color(&self) -> Option<Colors> {
        Some(match self {
            Alignment::Neutral => Colors::Neutral,
            Alignment::Belligerent{player} => Colors::Player(*player),
        })
    }
}

impl fmt::Display for Alignment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Alignment::Neutral => write!(f, "Neutral"),
            Alignment::Belligerent{player} => write!(f, "Player {}", player),
        }
    }
}

pub trait Aligned : AlignedMaybe {
    fn alignment(&self) -> Alignment;

    fn is_friendly_to<A:Aligned>(&self, other: &A) -> bool {
        self.alignment().is_friendly_to(other.alignment())
    }

    fn is_friendly_to_player(&self, player: PlayerNum) -> bool {
        self.alignment().is_friendly_to_player(player)
    }

    fn is_enemy_of<A:Aligned>(&self, other: &A) -> bool {
        self.alignment().is_enemy_of(other.alignment())
    }

    fn is_enemy_of_player(&self, player: PlayerNum) -> bool {
        self.alignment().is_enemy_of_player(player)
    }

    fn is_neutral(&self) -> bool {
        self.alignment().is_neutral()
    }

    fn is_belligerent(&self) -> bool {
        self.alignment().is_belligerent()
    }
}

pub trait AlignedMaybe {
    fn alignment_maybe(&self) -> Option<Alignment>;

    fn belongs_to_player(&self, player: PlayerNum) -> bool {
        if let Some(alignment) = self.alignment_maybe() {
            if let Alignment::Belligerent{player:player_} = alignment {
                player==player_
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl <T:Aligned> AlignedMaybe for T {
    fn alignment_maybe(&self) -> Option<Alignment> {
        Some(self.alignment())
    }
}

#[derive(Debug,PartialEq)]
pub struct TurnStart {
    pub turn: TurnNum,
    pub current_player: PlayerNum,
    pub orders_results: Vec<OrdersResult>,
    pub production_outcomes: Vec<UnitProductionOutcome>,
}

#[derive(Debug,Fail,PartialEq)]
pub enum GameError {
    #[fail(display = "There is no player {}", player)]
    NoSuchPlayer { player: PlayerNum },

    #[fail(display = "No unit with ID {:?} exists", id)]
    NoSuchUnit { id: UnitID },

    #[fail(display = "No unit at location {} exists", loc)]
    NoUnitAtLocation { loc: Location },

    #[fail(display = "No city with ID {:?} exists", id)]
    NoSuchCity { id: CityID },

    #[fail(display = "No city at location {} exists", loc)]
    NoCityAtLocation { loc: Location },

    #[fail(display = "No tile at location {} exists", loc)]
    NoTileAtLocation { loc: Location },

    #[fail(display = "Specified unit is not controlled by the current player")]
    UnitNotControlledByCurrentPlayer,

    #[fail(display = "The unit with ID {:?} has no carrying space", carrier_id)]
    UnitHasNoCarryingSpace { carrier_id: UnitID },

    #[fail(display = "The relevant carrying space cannot carry the unit with ID {:?} because its transport mode {:?} is
                      incompatible with the carrier's accepted transport mode {:?}", carried_id, carried_transport_mode,
                      carrier_transport_mode)]
    WrongTransportMode { carried_id: UnitID, carrier_transport_mode: TransportMode, carried_transport_mode: TransportMode },

    #[fail(display = "The relevant carrying space cannot carry the unit with ID {:?} due insufficient space.", carried_id)]
    InsufficientCarryingSpace { carried_id: UnitID },

    #[fail(display = "The relevant carrying space cannot carry the unit with ID {:?} because its alignment {:?} differs
                      from the space owner's alignment {:?}.", carried_id, carried_alignment, carrier_alignment)]
    OnlyAlliesCarry { carried_id: UnitID, carrier_alignment: Alignment, carried_alignment: Alignment },

    #[fail(display = "The unit with ID {:?} cannot occupy the city with ID {:?} because the unit with ID {:?} is still
                      garrisoned there. The garrison must be destroyed prior to occupation.", occupier_unit_id,
                    city_id, garrisoned_unit_id)]
    CannotOccupyGarrisonedCity { occupier_unit_id: UnitID, city_id: CityID, garrisoned_unit_id: UnitID },

    #[fail(display="There was a problem moving the unit: {}", 0)]
    MoveError(MoveError)
}

#[derive(Debug,PartialEq)]
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
    }
}

/// The core engine that enforces Umpire's game rules
#[derive(Clone)]
pub struct Game {
    /// The underlying state of the game
    map: MapData,

    /// The information that each player has about the state of the game
    player_observations: HashMap<PlayerNum,ObsTracker>,

    /// The turn that it is right now
    turn: TurnNum,

    /// Specification of who is human and who is what kind of robot
    num_players: PlayerNum,

    /// The player that is currently the player right now
    /// 
    /// Stored in a mutex to facilitate shared control of the game state by the UI and any AIs
    current_player: PlayerNum,

    /// The wrapping policy for the game---can you loop around the map vertically, horizontally, or both?
    wrapping: Wrap2d,

    /// A name generator to give names to units
    unit_namer: Rc<RwLock<dyn Namer>>,

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
    pub fn new<N:Namer>(
            map_dims: Dims,
            mut city_namer: N,
            num_players: PlayerNum,
            fog_of_war: bool,
            unit_namer: Option<Rc<RwLock<dyn Namer>>>,
            wrapping: Wrap2d) -> Self {

        let map = generate_map(&mut city_namer, map_dims, num_players);
        Self::new_with_map(map, num_players, fog_of_war, unit_namer, wrapping)
    }

    /// Creates a new game instance from a pre-generated map
    pub fn new_with_map(
            map: MapData,
            num_players: PlayerNum,
            fog_of_war: bool,
            unit_namer: Option<Rc<RwLock<dyn Namer>>>,
            wrapping: Wrap2d) -> Self {

        let mut player_observations = HashMap::new();
        for player_num in 0..num_players {
            player_observations.insert(player_num, ObsTracker::new(map.dims()));
        }

        let mut game = Self {
            map,
            player_observations,
            turn: 0,
            num_players,
            current_player: 0,
            wrapping,
            unit_namer: unit_namer.unwrap_or(Rc::new(RwLock::new(IntNamer::new("unit")))),
            fog_of_war,
            action_counts: vec![0; num_players],
            defeated_unit_hitpoints: vec![0; num_players],
        };

        game.begin_turn();
        game
    }

    pub fn num_players(&self) -> PlayerNum {
        self.num_players
    }

    pub fn player_turn_control(&mut self, player: PlayerNum) -> PlayerTurnControl {
        debug_assert_eq!(player, self.current_player);

        PlayerTurnControl::new(self, true, false)
    }

    pub fn player_turn_control_clearing(&mut self, player: PlayerNum) -> PlayerTurnControl {
        debug_assert_eq!(player, self.current_player);

        PlayerTurnControl::new_clearing(self)
    }

    pub fn player_turn_control_nonending(&mut self, player: PlayerNum) -> PlayerTurnControl {
        PlayerTurnControl::new(self, false, false)
    }

    fn produce_units(&mut self) -> Vec<UnitProductionOutcome> {
        // let max_unit_cost: u16 = UnitType::values().iter().map(|ut| ut.cost()).max().unwrap();

        // for city in self.current_player_cities_with_production_target_mut() {
        //     // We cap the production progress since, in weird circumstances such as a city having a unit blocking its
        //     // production for a very long time, the production progress adds can overflow
        //     if city.production_progress < max_unit_cost {
        //         city.production_progress += 1;
        //     }
        // }

        self.map.increment_player_city_production_targets(self.current_player());

        let producing_city_locs: Vec<Location> = self.current_player_cities_with_production_target()
            .filter(|city| {
                let unit_under_production = city.production().unwrap();

                city.production_progress >= unit_under_production.cost()
            })
            .map(|city| city.loc).collect()
        ;

        producing_city_locs.iter().cloned().map(|city_loc| {

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

            let result = self.map.new_unit(city_loc, unit_under_production, city_alignment, name);

            match result {
                Ok(_new_unit_id) => {
                    // We know the unit will be at top-level because that's where freshly-minted units go
                    
                    // let city = self.map.city_by_loc_mut(city_loc).unwrap();
                    // city.production_progress = 0;

                    self.map.clear_city_production_progress_by_loc(city_loc).unwrap();
                    let city = self.map.city_by_loc(city_loc).unwrap().clone();

                    // let city = city.clone();
                    let unit = self.map.toplevel_unit_by_loc(city_loc).unwrap().clone();

                    UnitProductionOutcome::UnitProduced {
                        city, unit,
                    }
                },
                Err(err) => match err {
                    NewUnitError::UnitAlreadyPresent{ prior_unit, unit_type_under_production, .. } => {
                        let city = self.map.city_by_loc(city_loc).unwrap();

                        UnitProductionOutcome::UnitAlreadyPresent {
                            prior_unit, unit_type_under_production, city: city.clone(),
                        }
                    },
                    err => {
                        panic!(format!("Error creating unit: {}", err))
                    }
                }
            }
        }).collect()
    }

    fn refresh_moves_remaining(&mut self) {
        // self.current_player_units_mut(|unit: &mut Unit| unit.refresh_moves_remaining());
        self.map.refresh_player_unit_moves_remaining(self.current_player());
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

    fn begin_turn(&mut self) -> TurnStart {
        let production_outcomes = self.produce_units();

        self.refresh_moves_remaining();

        self.update_current_player_observations();

        let orders_results = self.follow_pending_orders();

        TurnStart {
            turn: self.turn,
            current_player: self.current_player(),
            orders_results,
            production_outcomes,
        }
    }

    fn begin_turn_clearing(&mut self) -> TurnStart {
        let result = self.begin_turn();

        for prod in result.production_outcomes.iter() {
            if let UnitProductionOutcome::UnitProduced{city, ..} = prod {
                self.clear_production_without_ignoring(city.loc).unwrap();
            }
        }

        result
    }

    pub fn turn_is_done(&self) -> bool {
        self.production_set_requests().next().is_none() && self.unit_orders_requests().next().is_none()
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
            if let Alignment::Belligerent{player} = city.alignment {
                represented.insert(player);
            }
            if represented.len() > 1 {
                return None;
            }
        }

        for unit in self.map.units() {
            if let Alignment::Belligerent{player} = unit.alignment {
                if unit.type_.can_occupy_cities() {
                    represented.insert(player);
                }
            }
            if represented.len() > 1 {
                return None;
            }
        }

        if represented.len() == 1 {
            return Some(*represented.iter().next().unwrap());// unwrap to assert something's there
        }

        None
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
    pub fn end_turn(&mut self) -> Result<TurnStart,PlayerNum> {
        if self.turn_is_done() {
            Ok(self.force_end_turn())
        } else {
            Err(self.current_player())
        }
    }

    pub fn end_turn_clearing(&mut self) -> Result<TurnStart,PlayerNum> {
        if self.turn_is_done() {
            Ok(self.force_end_turn_clearing())
        } else {
            Err(self.current_player())
        }
    }

    fn _inc_current_player(&mut self) {
        self.current_player = (self.current_player + 1) % self.num_players();
        if self.current_player == 0 {
            self.turn += 1;
        }
    }

    /// End the turn without checking that the player has filled all production and orders requests.
    fn force_end_turn(&mut self) -> TurnStart {
        self.player_observations.get_mut(&self.current_player()).unwrap().archive();

        self._inc_current_player();

        self.begin_turn()
    }

    /// End the turn without checking that the player has filled all production and orders requests.
    fn force_end_turn_clearing(&mut self) -> TurnStart {
        self.player_observations.get_mut(&self.current_player()).unwrap().archive();

        self._inc_current_player();

        self.begin_turn_clearing()
    }

    pub fn propose_end_turn(&self) -> Proposed<Result<TurnStart,PlayerNum>> {
        let mut new = self.clone();
        let result = new.end_turn();
        Proposed::new(new, result)
    }

    /// Register the current observations of current player units
    /// 
    /// This applies only to top-level units. Carried units (e.g. units in a transport or carrier) make no observations
    fn update_current_player_observations(&mut self) {
        let current_player = self.current_player();
        let obs_tracker = self.player_observations.get_mut(&current_player).unwrap();

        if self.fog_of_war {
            for city in self.map.player_cities(self.current_player) {
                city.observe(&self.map, self.turn, self.wrapping, obs_tracker);
            }

            for unit in self.map.player_units(self.current_player) {
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
    pub fn current_player_unit_legal_one_step_destinations(&self, unit_id: UnitID) -> Result<HashSet<Location>,GameError> {
        let unit = self.current_player_unit_by_id(unit_id).ok_or_else(||
            GameError::NoSuchUnit { id: unit_id }
        )?;

        Ok(
            neighbors_unit_could_move_to_iter(&self.map, &unit, self.wrapping)
            .filter(|loc| *loc != unit.loc)// exclude the source location; needed because UnitMovementFilter inside of
            .collect()                     // neighbors_unit_could_move_to_iter would allow a carried unit to "move"
                                           // onto the carrier unit over again if it additional carrying space, thus
                                           // resulting in zero-length moves
        )
    }

    pub fn current_player_unit_legal_directions<'a>(&'a self, unit_id: UnitID) -> Result<impl Iterator<Item=Direction>+'a,GameError> {
        let unit = self.current_player_unit_by_id(unit_id).ok_or_else(||
            GameError::NoSuchUnit { id: unit_id }
        )?;

        Ok(directions_unit_could_move_iter(&self.map, &unit, self.wrapping))
    }

    /// The current player's most recent observation of the tile at location `loc`, if any
    pub fn current_player_tile(&self, loc: Location) -> Option<&Tile> {
        if let Obs::Observed{tile,..} = self.current_player_obs(loc) {
            Some(tile)
        } else {
            None
        }
    }

    /// The current player's observation at location `loc`
    pub fn current_player_obs(&self, loc: Location) -> &Obs {
        self.player_observations[&self.current_player()].get(loc)
    }

    pub fn current_player_observations(&self) -> &ObsTracker {
        self.player_observations(self.current_player)
    }

    pub fn player_observations(&self, player: PlayerNum) -> &ObsTracker {
        self.player_observations.get(&player).unwrap()
    }

    // fn current_player_observations_mut(&mut self) -> &mut ObsTracker {
    //     self.player_observations.get_mut(&self.current_player).unwrap()
    // }

    fn player_cities(&self, player: PlayerNum) -> impl Iterator<Item=&City> {
        self.map.player_cities(player)
    }

    /// Every city controlled by the current player
    pub fn current_player_cities(&self) -> impl Iterator<Item=&City> {
        self.player_cities(self.current_player())
    }

    /// All cities controlled by the current player which have a production target set
    pub fn current_player_cities_with_production_target(&self) -> impl Iterator<Item=&City> {
        self.map.player_cities_with_production_target(self.current_player())
    }

    /// How many cities does the current player control?
    pub fn current_player_city_count(&self) -> usize {
        self.player_city_count(self.current_player)
    }

    /// How many cities does the specified player control?
    pub fn player_city_count(&self, player: PlayerNum) -> usize {
        self.map.player_city_count(player).unwrap_or_default()
    }

    // /// All cities controlled by the current player which have a production target set, mutably
    // fn current_player_cities_with_production_target_mut(&mut self) -> impl Iterator<Item=&mut City> {
    //     self.map.player_cities_with_production_target_mut(self.current_player())
    // }

    // /// Mutate all units controlled by the current player according to the callback `callback`
    // fn current_player_units_mut<F:FnMut(&mut Unit)>(&mut self, callback: F) {
    //     self.map.player_units_mut(self.current_player(), callback);
    // }

    /// The number of cities controlled by the current player which either have a production target or are NOT set to be ignored when requesting productions to be set
    /// 
    /// This basically lets us make sure a player doesn't set all their cities' productions to none since right now the UI has no way of getting out of that situation
    /// 
    /// FIXME Get rid of this and just make the UI smarter
    #[deprecated]
    pub fn player_cities_producing_or_not_ignored(&self) -> usize {
        self.current_player_cities().filter(|city| city.production().is_some() || !city.ignore_cleared_production()).count()
    }

    fn player_units(&self, player: PlayerNum) -> impl Iterator<Item=&Unit> {
        self.map.player_units(player)
    }

    /// Every unit controlled by the current player
    pub fn current_player_units(&self) -> impl Iterator<Item=&Unit> {
        self.player_units(self.current_player())
    }

    /// The counts of unit types controlled by the current player
    pub fn current_player_unit_type_counts(&self) -> Result<&HashMap<UnitType,usize>,GameError> {
        self.player_unit_type_counts(self.current_player)
    }

    /// The counts of unit types controlled by the specified player
    pub fn player_unit_type_counts(&self, player: PlayerNum) -> Result<&HashMap<UnitType,usize>,GameError> {
        self.map.player_unit_type_counts(player)
    }

    /// Every enemy unit known to the current player (as of most recent observations)
    pub fn observed_enemy_units<'a>(&'a self) -> impl Iterator<Item=&Unit> + 'a {
        let current_player = self.current_player();
        self.current_player_observations().iter().filter_map(move |obs| match obs {
            Obs::Observed { tile, .. } => if let Some(ref unit) = tile.unit {
                if let Alignment::Belligerent{player} = unit.alignment {
                    if player != current_player {
                        Some(unit)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            },
            _ => None,
        })
        // self.map.units().filter(move |unit| unit.alignment != Alignment::Belligerent{player:current_player})
    }

    /// If the current player controls a city at location `loc`, return it
    pub fn current_player_city_by_loc(&self, loc: Location) -> Option<&City> {
        self.current_player_tile(loc).and_then(|tile| tile.city.as_ref())
    }

    /// If the current player controls a city with ID `city_id`, return it
    pub fn current_player_city_by_id(&self, city_id: CityID) -> Option<&City> {
        self.current_player_cities().find(|city| city.id==city_id)
    }

    // /// If the current player controls a city with ID `city_id`, return it mutably
    // pub fn current_player_city_by_id_mut(&mut self, city_id: CityID) -> Option<&mut City> {
    //     self.map.player_cities_mut(self.current_player()).find(|city| city.id==city_id)
    // }

    /// If the current player controls a unit with ID `id`, return it
    pub fn current_player_unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.player_unit_by_id(self.current_player, id)
    }

    /// If the specified player controls a unit with ID `id`, return it
    pub fn player_unit_by_id(&self, player: PlayerNum, id: UnitID) -> Option<&Unit> {
        self.map.player_unit_by_id(player, id)
    }

    /// If the current player controls a unit with ID `id`, return its location
    pub fn current_player_unit_loc(&self, id: UnitID) -> Option<Location> {
        self.player_unit_loc(self.current_player, id)
    }

    /// If the specified player controls a unit with ID `id`, return its location
    pub fn player_unit_loc(&self, player: PlayerNum, id: UnitID) -> Option<Location> {
        self.player_unit_by_id(player, id).map(|unit| unit.loc)
    }

    /// If the current player controls the top-level unit at location `loc`, return it
    pub fn current_player_toplevel_unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        self.current_player_tile(loc).and_then(|tile| tile.unit.as_ref())
    }

    pub fn production_set_requests<'a>(&'a self) -> impl Iterator<Item=Location> + 'a {
        self.player_production_set_requests(self.current_player)
    }

    pub fn player_production_set_requests<'a>(&'a self, player: PlayerNum) -> impl Iterator<Item=Location> + 'a {
        self.map.player_cities_lacking_production_target(player).map(|city| city.loc)
    }

    /// Which if the current player's units need orders?
    /// 
    /// In other words, which of the current player's units have no orders and have moves remaining?
    pub fn unit_orders_requests<'a>(&'a self) -> impl Iterator<Item=UnitID> + 'a {
        self.player_unit_orders_requests(self.current_player)
    }

    pub fn player_unit_orders_requests<'a>(&'a self, player: PlayerNum) -> impl Iterator<Item=UnitID> + 'a {
        self.map.player_units(player)
            .filter(|unit| unit.orders.is_none() && unit.moves_remaining() > 0)
            .map(|unit| unit.id)
    }

    /// Which if the current player's units need orders?
    /// 
    /// In other words, which of the current player's units have no orders and have moves remaining?
    pub fn units_with_orders_requests<'a>(&'a self) -> impl Iterator<Item=&Unit> + 'a {
        self.map.player_units(self.current_player())
            .filter(|unit| unit.orders.is_none() && unit.moves_remaining() > 0)
    }

    pub fn units_with_pending_orders<'a>(&'a self) -> impl Iterator<Item=UnitID> + 'a {
        self.current_player_units()
            .filter(|unit| unit.moves_remaining() > 0 && unit.orders.is_some() && *unit.orders.as_ref().unwrap() != Orders::Sentry)
            .map(|unit| unit.id)
    }


    // Movement-related methods

    pub fn move_toplevel_unit_by_id(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        let src = self.map.unit_loc(unit_id).unwrap();
        self.move_toplevel_unit_by_loc(src, dest)
    }

    pub fn move_toplevel_unit_by_id_avoiding_combat(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        let src = self.map.unit_loc(unit_id).unwrap();
        self.move_toplevel_unit_by_loc_avoiding_combat(src, dest)
    }

    /*
        Errors:
        * If unit at `src` doesn't exist
        * If requested move requires more moves than the unit has remaining
        * If `dest` is unreachable from `src` (may be subsumed by previous)

        FIXME Make the unit observe at each point along its path

        FIXME This function checks two separate times whether a unit exists at src
    */
    pub fn move_toplevel_unit_by_loc(&mut self, src: Location, dest: Location) -> MoveResult {
        let unit = self.map.toplevel_unit_by_loc(src)
                                  .ok_or(MoveError::SourceUnitNotAtLocation{src})?
                                  .clone();
        
        let filter = UnitMovementFilter::new(&unit);
        self.move_toplevel_unit_by_loc_using_filter(src, dest, &filter)
    }

    pub fn move_toplevel_unit_by_loc_avoiding_combat(&mut self, src: Location, dest: Location) -> MoveResult {
        let unit = self.map.toplevel_unit_by_loc(src).unwrap().clone();
        let unit_filter = AndFilter::new(
            AndFilter::new(
                NoUnitsFilter{},
                NoCitiesButOursFilter{alignment: unit.alignment }
            ),
            UnitMovementFilter{unit: &unit}
        );
        self.move_toplevel_unit_by_loc_using_filter(src, dest, &unit_filter)
    }

    fn move_toplevel_unit_by_loc_using_filter<F:Filter<Obs>>(&mut self, src: Location, dest: Location, filter: &F) -> MoveResult {
        let unit_id = self.current_player_toplevel_unit_by_loc(src)
                                          .map(|unit| unit.id)
                                          .ok_or(MoveError::SourceUnitNotAtLocation{src})?;
        
        self.move_unit_by_id_using_filter(unit_id, dest, filter)
    }

    // fn propose_move_toplevel_unit_by_loc_using_filter<F:Filter<Obs>>(&self, src: Location, dest: Location, filter: &F) -> MoveResult {
    //     let mut new = self.clone();
    //     new.move_toplevel_unit_by_loc_using_filter(src, dest, filter)
    // }

    /// Move a unit one step in a particular direction
    pub fn move_unit_by_id_in_direction(&mut self, unit_id: UnitID, direction: Direction) -> MoveResult {
        // let unit_loc = self.map.unit_by_id(id)
            // .ok_or_else(|| MoveError::SourceUnitDoesNotExist {id})?.loc;
        let unit = self.current_player_unit_by_id(unit_id)
            .ok_or(MoveError::SourceUnitDoesNotExist{id: unit_id})?.clone();

        let filter = UnitMovementFilter::new(&unit);

        let dest = unit.loc.shift_wrapped(direction, self.dims(), self.wrapping())
            .ok_or_else(|| MoveError::DestinationOutOfBounds{})?;

        // self.move_unit_by_id(id, dest)
        self.move_unit_by_id_using_filter(unit_id, dest, &filter)
    }

    pub fn move_unit_by_id(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        // self.propose_move_unit_by_id(unit_id, dest).map(|proposed_move| proposed_move.take(self))
        // let unit = self.current_player_unit_by_id(unit_id).unwrap().clone();
        let unit = self.current_player_unit_by_id(unit_id)
            .ok_or(MoveError::SourceUnitDoesNotExist{id: unit_id})?.clone();

        let filter = UnitMovementFilterXenophile::new(&unit);
        self.move_unit_by_id_using_filter(unit_id, dest, &filter)
    }

    pub fn propose_move_unit_by_id(&self, id: UnitID, dest: Location) -> Proposed<MoveResult> {
        let mut new = self.clone();
        let result = new.move_unit_by_id(id, dest);
        Proposed::new(new, result)
    }

    pub fn move_unit_by_id_avoiding_combat(&mut self, id: UnitID, dest: Location) -> MoveResult {
        let unit = self.map.unit_by_id(id).unwrap().clone();
        let unit_filter = AndFilter::new(
            AndFilter::new(
                NoUnitsFilter{},
                NoCitiesButOursFilter{alignment: unit.alignment }
            ),
            UnitMovementFilter{unit: &unit}
        );
        self.move_unit_by_id_using_filter(id, dest, &unit_filter)
    }

    pub fn propose_move_unit_by_id_avoiding_combat(&self, id: UnitID, dest: Location) -> Proposed<MoveResult> {
        let mut new = self.clone();
        let result = new.move_unit_by_id_avoiding_combat(id, dest);
        Proposed::new(new, result)
    }

    /// Make a best-effort attempt to move the given unit to the destination, generating shortest paths repeatedly using
    /// the given tile filter. This is necessary because, as the unit advances, it observes tiles which may have been
    /// previously observed but are now stale. If the tile state changes, then the shortest path will change and
    /// potentially other behaviors like unit carrying and combat.
    fn move_unit_by_id_using_filter<F:Filter<Obs>>(
        &mut self,
        unit_id: UnitID,
        dest: Location,
        tile_filter: &F) -> MoveResult {

        if !self.dims().contain(dest) {
            return Err(MoveError::DestinationOutOfBounds {});
        }

        // Grab a copy of the unit to work with
        let mut unit = self.current_player_unit_by_id(unit_id)
            .ok_or(MoveError::SourceUnitDoesNotExist{id: unit_id})?.clone();

        if unit.loc == dest {
            return Err(MoveError::ZeroLengthMove);
        }

        // let obs_tracker = self.current_player_observations_mut();
        let obs_tracker = self.player_observations.get_mut(&self.current_player).unwrap();

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
                shortest_paths = Some(dijkstra::shortest_paths(obs_tracker, unit.loc, tile_filter, self.wrapping, unit.moves_remaining()));
                moves_remaining_on_last_shortest_paths_calculation = unit.moves_remaining();
            }

            if let Some(distance_to_dest_on_last_shortest_paths_calculation) = shortest_paths.as_ref().unwrap().dist.get(dest).cloned() {
                let movement_since_last_shortest_paths_calculation = moves_remaining_on_last_shortest_paths_calculation - unit.moves_remaining();

                // Distance to destination from unit's current location
                let distance = distance_to_dest_on_last_shortest_paths_calculation - movement_since_last_shortest_paths_calculation;

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
                    });
                }

                let shortest_path: Vec<Location> = shortest_paths.as_ref().unwrap().shortest_path(dest).unwrap();

                // skip the source location and steps we've taken since "baseline"
                let loc = shortest_path[1 + movement_since_last_shortest_paths_calculation as usize];
                
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
                if let Some(ref other_unit) = self.map.toplevel_unit_by_loc(loc).cloned() {// CLONE to dodge mutability

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

                            panic!("Could not carry unit for some weird reason: {:?}
                                    tile: {:?}
                                    tile city: {:?}
                                    tile unit: {:?}
                                    unit: {:?}
                                    src_tile: {:?}
                                    src_tile city: {:?}
                                    src_tile unit: {:?}",
                                    e, tile, tile.city, tile.unit, unit,
                                       src_tile, src_tile.city, src_tile.unit
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
                            self.defeated_unit_hitpoints[self.current_player] += other_unit.max_hp() as u64;

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
                                let prior_unit = self.map.relocate_unit_by_id(unit_id, loc).unwrap();
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
                move_.observations_after_move = unit.observe(&self.map, self.turn, self.wrapping, obs_tracker);

                // Inspect all observations besides at the unit's previous and current location to see if any changes in
                // passability have occurred relevant to the unit's future moves.
                // If so, request shortest_paths to be recalculated
                let passability_changed = move_.observations_after_move
                    .iter()
                    .filter(|located_obs| located_obs.loc != unit.loc && located_obs.loc != prev_loc)
                    .any(|located_obs| {
                        located_obs.passability_changed(tile_filter)
                    }
                );
                if passability_changed {
                    // Mark the shortest_paths stale so it gets recomputed
                    shortest_paths = None;
                }

            } else {
                return Err(MoveError::NoRoute{src, dest, id: unit_id});
            }
        }// while

        let move_ = moves.last_mut().unwrap();
        // ----- Make observations from the unit's new location -----
        move_.observations_after_move = unit.observe(&self.map, self.turn, self.wrapping, obs_tracker);

        // If the unit wasn't destroyed, register its movement in the map rather than just this clone
        if move_.moved_successfully() {
            if movement_complete {
                self.map.mark_unit_movement_complete(unit_id).unwrap();
                unit.movement_complete();
            } else {
                let distance_moved = moves.iter().map(|move_| move_.distance_moved() as u16)
                                                                .sum();
                self.map.record_unit_movement(unit_id, distance_moved).unwrap().unwrap();
            }
        }

        self.action_taken();

        Move::new(unit, src, moves)
    }

    /// Disbands
    pub fn disband_unit_by_id(&mut self, unit_id: UnitID) -> Result<Unit,GameError> {
        let unit = self.map.pop_player_unit_by_id(self.current_player, unit_id)
            .ok_or(GameError::NoSuchUnit{id: unit_id})?;

        // Make a fresh observation with the disbanding unit so that its absence is noted.
        let obs_tracker = self.player_observations.get_mut(&self.current_player).unwrap();
        unit.observe(&self.map, self.turn, self.wrapping, obs_tracker);

        self.action_taken();
        
        Ok(unit)
    }

    /// Sets the production of the current player's city at location `loc` to `production`, returning the prior setting.
    /// 
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    pub fn set_production_by_loc(&mut self, loc: Location, production: UnitType) -> Result<Option<UnitType>,GameError> {
        let result = self.map.set_player_city_production_by_loc(self.current_player, loc, production);
        if result.is_ok() {
            self.action_taken();
        }
        result
    }

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    /// 
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    pub fn set_production_by_id(&mut self, city_id: CityID, production: UnitType) -> Result<Option<UnitType>,GameError> {
        let result = self.map.set_player_city_production_by_id(self.current_player(), city_id, production);
        if result.is_ok() {
            self.action_taken();
        }
        result
    }

    //FIXME Restrict to current player cities
    pub fn clear_production_without_ignoring(&mut self, loc: Location) -> Result<(),String> {
        self.map.clear_city_production_without_ignoring_by_loc(loc).map_err(|_| format!(
            "Attempted to clear production for city at location {} but there is no city at that location",
            loc
        ))
    }

    //FIXME Restrict to current player cities
    pub fn clear_production_and_ignore(&mut self, loc: Location) -> Result<(),String> {
        self.map.clear_city_production_and_ignore_by_loc(loc).map_err(|_|
            format!(
                "Attempted to clear production for city at location {} but there is no city at that location",
                loc
            )
        )
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

    /// Units that could be produced by a city located at the given location
    pub fn valid_productions<'a>(&'a self, loc: Location) -> impl Iterator<Item=UnitType> + 'a {
        UNIT_TYPES.iter()
        .cloned()
        .filter(move |unit_type| {
            for neighb_loc in neighbors_terrain_only(&self.map, loc, *unit_type, self.wrapping) {
                let tile = self.map.tile(neighb_loc).unwrap();
                if unit_type.can_move_on_tile( &tile ) {
                    return true;
                }
            }
            false
        })
    }

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    pub fn valid_productions_conservative<'a>(&'a self, loc: Location) -> impl Iterator<Item=UnitType> + 'a {
        UNIT_TYPES.iter()
        .cloned()
        .filter(move |unit_type| {
            for neighb_loc in neighbors_terrain_only(&self.map, loc, *unit_type, self.wrapping) {
                let tile = self.map.tile(neighb_loc).unwrap();
                if unit_type.can_occupy_tile( &tile ) {
                    return true;
                }
            }
            false
        })
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    pub fn order_unit_sentry(&mut self, unit_id: UnitID) -> OrdersResult {
        let orders = Orders::Sentry;

        self.map.set_player_unit_orders(self.current_player(), unit_id, orders)?;

        let ordered_unit = self.current_player_unit_by_id(unit_id).unwrap().clone();

        self.action_taken();

        Ok(OrdersOutcome::completed_without_move(ordered_unit, orders))
    }

    pub fn order_unit_skip(&mut self, unit_id: UnitID) -> OrdersResult {
        let orders = Orders::Skip;
        let unit = self.current_player_unit_by_id(unit_id).unwrap().clone();
        let result = self.set_orders(unit_id, orders).map(|_| OrdersOutcome::in_progress_without_move(unit, orders));
        if result.is_ok() {
            self.action_taken();
        }
        result
    }

    pub fn order_unit_go_to(&mut self, unit_id: UnitID, dest: Location) -> OrdersResult {
        // self.propose_order_unit_go_to(unit_id, dest).take(self)
        self.set_and_follow_orders(unit_id, Orders::GoTo{dest})
    }

    /// Simulate ordering the specified unit to go to the given location
    pub fn propose_order_unit_go_to(&self, unit_id: UnitID, dest: Location) -> Proposed<OrdersResult> {
        self.propose_set_and_follow_orders(unit_id, Orders::GoTo{dest})
    }

    pub fn order_unit_explore(&mut self, unit_id: UnitID) -> OrdersResult {
        // self.propose_order_unit_explore(unit_id).take(self)
        self.set_and_follow_orders(unit_id, Orders::Explore)
    }

    /// Simulate ordering the specified unit to explore.
    pub fn propose_order_unit_explore(&self, unit_id: UnitID) -> Proposed<OrdersResult> {
        self.propose_set_and_follow_orders(unit_id, Orders::Explore)
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    pub fn activate_unit_by_loc(&mut self, loc: Location) -> Result<(),GameError> {
        let unit_id = {
            let unit = self.map.toplevel_unit_by_loc(loc)
                                    .ok_or(GameError::NoUnitAtLocation{loc})?;

            if !unit.belongs_to_player(self.current_player()) {
                return Err(GameError::UnitNotControlledByCurrentPlayer{});
            }

            unit.id
        };

        self.map.activate_player_unit(self.current_player(), unit_id)
    }

    /// If the current player controls a unit with ID `id`, set its orders to `orders`
    /// 
    /// # Errors
    /// `OrdersError::OrderedUnitDoesNotExist` if the order is not present under the control of the current player
    fn set_orders(&mut self, id: UnitID, orders: Orders) -> Result<Option<Orders>,GameError> {
        self.map.set_player_unit_orders(self.current_player(), id, orders)
    }

    /// Clear the orders of the unit controlled by the current player with ID `id`.
    fn clear_orders(&mut self, id: UnitID) -> Result<Option<Orders>,GameError> {
        self.map.clear_player_unit_orders(self.current_player(), id)
    }

    fn follow_pending_orders(&mut self) -> Vec<OrdersResult> {
        let pending_orders: Vec<UnitID> = self.units_with_pending_orders().collect();

        pending_orders.iter()
            .map(|unit_id| self.follow_unit_orders(*unit_id))
            .collect()
    }

    /// Make the unit with ID `id` under the current player's control follow its orders
    /// 
    /// # Panics
    /// This will panic if the current player does not control such a unit.
    /// 
    fn follow_unit_orders(&mut self, id: UnitID) -> OrdersResult {
        let orders = self.current_player_unit_by_id(id).unwrap().orders.as_ref().unwrap();

        let result = orders.carry_out(id, self);

        // If the orders are already complete, clear them out
        if let Ok(OrdersOutcome{ status: OrdersStatus::Completed, .. }) = result {
            self.map.clear_player_unit_orders(self.current_player(), id)?;
        }
        
        result
    }

    /// Simulate setting the orders of unit with ID `id` to `orders` and then following them out.
    fn propose_set_and_follow_orders(&self, id: UnitID, orders: Orders) -> Proposed<OrdersResult> {
        let mut new = self.clone();
        let orders_result = new.set_and_follow_orders(id, orders);
        Proposed::new(new, orders_result)
    }

    fn set_and_follow_orders(&mut self, id: UnitID, orders: Orders) -> OrdersResult {
        self.set_orders(id, orders)?;

        let result = self.follow_unit_orders(id);

        if result.is_ok() {
            self.action_taken();
        }

        result

    }

    pub fn current_player_score(&self) -> f64 {
        self.player_score(self.current_player).unwrap()
    }

    fn player_score(&self, player: PlayerNum) -> Result<f64,GameError> {
        let mut score = 0.0;

        // Observations
        let observed_tiles = self.player_observations.get(&player)
                                 .ok_or(GameError::NoSuchPlayer{player})?
                                 .num_observed();

        score += observed_tiles as f64 * TILE_OBSERVED_BASE_SCORE;
    
        // Controlled units
        for unit in self.player_units(player) {
            // The cost of the unit scaled by the unit's current hitpoints relative to maximum
            score += UNIT_MULTIPLIER * (unit.type_.cost() as f64) * (unit.hp() as f64) / (unit.max_hp() as f64);
        }

        // Defeated units
        score += UNIT_MULTIPLIER * self.defeated_unit_hitpoints[self.current_player] as f64;
    
        // Controlled cities
        for city in self.player_cities(player) {
            // The city's intrinsic value plus any progress it's made toward producing its unit
            score += CITY_INTRINSIC_SCORE + city.production_progress as f64 * UNIT_MULTIPLIER;
        }

        // Turn penalty to discourage sitting around
        score -= TURN_PENALTY * self.turn as f64;

        // Penalty for each action taken
        score -= ACTION_PENALTY * self.action_counts[self.current_player] as f64;
    
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
        .map(|player| self.player_score(player).unwrap())
        .collect()
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
    pub fn player_features(&self, player: PlayerNum) -> Vec<fX> {
        // For every tile we add these f64's:
        // is the tile observed or not?
        // which player controls the tile (one hot encoded)
        // is there a city or not?
        // what is the unit type? (one hot encoded, could be none---all zeros)
        // for each of the five potential carried units:
        //   what is the unit type? (one hot encoded, could be none---all zeros)
        // 

        let unit_id = self.player_unit_orders_requests(player).next();
        let city_loc = self.player_production_set_requests(player).next();

        let unit_type = unit_id.and_then(|unit_id| self.player_unit_by_id(player, unit_id).map(|unit| unit.type_));

        // We also add a context around the currently active unit (if any)
        let mut x = Vec::with_capacity(FEATS_LEN as usize);

        // General statistics

        // NOTE Update dnn::ADDED_WIDE_FEATURES to reflect the number of generic features added here

        // - current turn
        x.push(self.turn as fX);

        // - number of cities player controls
        x.push(self.player_city_count(player) as fX);

        let observations = self.player_observations(player);

        // - number of tiles observed
        let num_observed: fX = observations.num_observed() as fX;
        x.push(num_observed);

        // - percentage of tiles observed
        x.push(num_observed / self.dims().area() as fX);

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
        let type_counts = self.player_unit_type_counts(player).unwrap_or(&empty_map);
        let counts_vec: Vec<fX> = UnitType::values().iter()
                                    .map(|type_| *type_counts.get(type_).unwrap_or(&0) as fX)
                                    .collect();

        x.extend(counts_vec);

        
        

        // Relatively positioned around next unit (if any) or city
        
        let loc = unit_id.map(|unit_id| {
            match self.player_unit_loc(player, unit_id) {
                Some(loc) => loc,
                None => {
                    panic!("Unit was in orders requests but not in current player observations")
                },
            }
        }).or(city_loc);

        let mut is_enemy_belligerent = Vec::new();
        let mut is_observed = Vec::new();
        let mut is_neutral = Vec::new();
        let mut is_city = Vec::new();

        // 2d features
        for inc_x in -5..=5 {
            for inc_y in -5..=5 {
                let inc: Vec2d<i32> = Vec2d::new(inc_x, inc_y);

                let obs = if let Some(origin) = loc {
                    self.wrapping.wrapped_add(self.dims(), origin, inc)
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

                if let Obs::Observed{tile, ..} = obs {
                    observed = 1.0;

                    if tile.city.is_some() {
                        city = 1.0;
                    }

                    if let Some(alignment) = tile.alignment_maybe() {
                        if alignment.is_neutral() {
                            neutral = 1.0;
                        } else if alignment.is_belligerent() && alignment.is_enemy_of_player(self.current_player) {
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

        x
    }

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
    fn features(&self) -> Vec<fX> {
        self.player_features(self.current_player)
    }

    fn take_action(&mut self, action: UmpireAction) -> Result<(),GameError> {
        action.take(self)
    }
}

impl Dimensioned for Game {
    fn dims(&self) -> Dims {
        self.dims()
    }
}

impl Source<Tile> for Game {
    fn get(&self, loc: Location) -> &Tile {
        self.current_player_tile(loc).unwrap()
    }
}
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
        },
        Obs::Observed{tile,..} => {

            // let mut x = vec![1.0];// observed
            x.push(1.0);// observed

            for p in 0..num_players {// which player controls the tile (one hot encoded)
                x.push(if let Some(Alignment::Belligerent{player}) = tile.alignment_maybe() {
                    if player==p {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    0.0
                });
            }

            x.push(if tile.city.is_some() { 1.0 } else { 0.0 });// city or not

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
            x.push(if dir==*dir2 { 1.0 } else { 0.0 } );
        }
    } else {
        for _ in 0..Direction::values().len() {
            x.push(0.0);
        }
    }
}

impl DerefVec for Game {
    fn deref_vec(&self) -> Vec<fX> {
        self.features()
    }
}

impl fmt::Debug for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.map.fmt(f)
    }
}

/// Test support functions
pub mod test_support {

    use std::{
        rc::Rc,
        sync::{
            RwLock,
        }
    };

    use crate::{
        game::{
            Alignment,
            Game,
            map::{
                MapData,
                Terrain,
            },
            obs::Obs,
            unit::{
                UnitID,
                UnitType,
            },
        },
        name::unit_namer,
        util::{
            Dims,
            Location,
            Wrap2d,
        },
    };

    pub fn test_propose_move_unit_by_id() {
        let src = Location{x:0, y:0};
        let dest = Location{x:1, y:0};

        let game = game_two_cities_two_infantry();

        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();

        {
            let unit = game.current_player_unit_by_id(unit_id).unwrap();
            assert_eq!(unit.loc, src);
        }

        let proposed_move = game.propose_move_unit_by_id(unit_id, dest).delta.unwrap();

        let component = proposed_move.components.get(0).unwrap();

        // Make sure the intended destination is now observed as containing this unit, and that no other observed tiles
        // are observed as containing it
        for located_obs in &component.observations_after_move {
            match located_obs.obs {
                Obs::Observed{ref tile, turn, current} =>{
                    if located_obs.loc == dest {
                        let unit = tile.unit.as_ref().unwrap();
                        assert_eq!(unit.id, unit_id);
                        assert_eq!(turn, 6);
                        assert!(current);
                    } else if let Some(unit) = tile.unit.as_ref() {
                        assert_ne!(unit.id, unit_id);
                    }
                },
                Obs::Unobserved => panic!("This should be observed")
            }
        }
    }

    /// 10x10 grid of land only with two cities:
    /// * Player 0's Machang at 0,0
    /// * Player 1's Zanzibar at 0,1
    fn map_two_cities(dims: Dims) -> MapData {
        let mut map = MapData::new(dims, |_loc| Terrain::Land);
        map.new_city(Location{x:0,y:0}, Alignment::Belligerent{player:0}, "Machang").unwrap();
        map.new_city(Location{x:0,y:1}, Alignment::Belligerent{player:1}, "Zanzibar").unwrap();
        map
    }

    #[cfg(test)]
    pub(crate) fn game1() -> Game {
        let players = 2;
        let fog_of_war = true;
 
        let map = map_two_cities(Dims::new(10, 10));
        let unit_namer = unit_namer();
        Game::new_with_map(map, players, fog_of_war, Some(Rc::new(RwLock::new(unit_namer))), Wrap2d::BOTH)
    }

    pub(crate) fn game_two_cities_dims(dims: Dims) -> Game {
        let players = 2;
        let fog_of_war = true;
 
        let map = map_two_cities(dims);
        let unit_namer = unit_namer();
        let mut game = Game::new_with_map(map, players, fog_of_war, Some(Rc::new(RwLock::new(unit_namer))), Wrap2d::BOTH);

        let loc: Location = game.production_set_requests().next().unwrap();

        // println!("Setting production at {:?} to infantry", loc);
        game.set_production_by_loc(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn().unwrap().current_player;
        assert_eq!(player, 1);

        let loc: Location = game.production_set_requests().next().unwrap();
        // println!("Setting production at {:?} to infantry", loc);
        game.set_production_by_loc(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn().unwrap().current_player;
        assert_eq!(player, 0);

        game
    }

    fn map_tunnel(dims: Dims) -> MapData {
        let mut map = MapData::new(dims, |_loc| Terrain::Land);
        map.new_city(Location::new(0, dims.height / 2), Alignment::Belligerent{player:0}, "City 0").unwrap();
        map.new_city(Location::new(dims.width - 1, dims.height / 2), Alignment::Belligerent{player:1}, "City 1").unwrap();
        map
    }

    pub fn game_tunnel(dims: Dims) -> Game {
        let players = 2;
        let fog_of_war = false;
        let map = map_tunnel(dims);
        let unit_namer = unit_namer();
        Game::new_with_map(map, players, fog_of_war, Some(Rc::new(RwLock::new(unit_namer))), Wrap2d::NEITHER)
    }

    // pub(crate) fn game_two_cities() -> Game {
    //     game_two_cities_dims(Dims::new(10, 10))
    // }

    // pub(crate) fn game_two_cities_big() -> Game {
    //     game_two_cities_dims(Dims::new(100, 100))
    // }

    pub fn game_two_cities_two_infantry_dims(dims: Dims) -> Game {
        let mut game = game_two_cities_dims(dims);

        for _ in 0..5 {
            let player = game.end_turn().unwrap().current_player;
            assert_eq!(player, 1);
            let player = game.end_turn().unwrap().current_player;
            assert_eq!(player, 0);
        }

        assert_eq!(game.end_turn(), Err(0));
        assert_eq!(game.end_turn(), Err(0));

        game
    }

    pub fn game_two_cities_two_infantry() -> Game {
        game_two_cities_two_infantry_dims(Dims::new(10, 10))
    }

    pub fn game_two_cities_two_infantry_big() -> Game {
        game_two_cities_two_infantry_dims(Dims::new(100, 100))
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{
            HashMap,
            HashSet,
        },
        convert::TryFrom,
        rc::Rc,
        sync::{
            RwLock,
        },
    };

    use rand::{
        Rng,
        thread_rng,
    };

    use crate::{
        game::{
            Alignment,
            Game,
            GameError,
            map::{
                MapData,
                Terrain,
            },
            move_::MoveError,
            test_support::{game_two_cities_two_infantry},
            unit::{
                TransportMode,
                Unit,
                UnitID,
                UnitType,
                orders::{
                    Orders,
                    OrdersStatus,
                },
            },
        },
        name::{
            Named,
            unit_namer,
        },
        
        util::{Dimensioned,Dims,Direction,Location,Vec2d,Wrap2d},
    };

    #[test]
    fn test_game() {
        let mut game = game_two_cities_two_infantry();

        for player in 0..2 {
            assert_eq!(game.unit_orders_requests().count(), 1);
            let unit_id: UnitID = game.unit_orders_requests().next().unwrap();
            let loc = game.current_player_unit_loc(unit_id).unwrap();
            let new_x = (loc.x + 1) % game.dims().width;
            let new_loc = Location{x:new_x, y:loc.y};
            println!("Moving unit from {} to {}", loc, new_loc);

            match game.move_toplevel_unit_by_loc(loc, new_loc) {
                Ok(move_result) => {
                    println!("{:?}", move_result);
                },
                Err(msg) => {
                    panic!("Error during move: {}", msg);
                }
            }
            
            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1-player);
        }
    }

    #[test]
    fn test_move_unit_by_id_far() {
        let mut map = MapData::new(Dims::new(180, 90), |_| Terrain::Water);
        let unit_id = map.new_unit(Location::new(0,0), UnitType::Fighter, Alignment::Belligerent{player:0}, "Han Solo").unwrap();

        let mut game = Game::new_with_map(map, 1, true, None, Wrap2d::BOTH);

        let mut rand = thread_rng();
        for _ in 0..10 {
            let mut delta = Vec2d::new(0,0);

            while delta.x==0 && delta.y==0 {
                delta = Vec2d::new(
                    rand.gen_range(-5, 6),
                    rand.gen_range(-5, 6),
                );
            }

            let unit_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
            let dest = game.wrapping().wrapped_add(game.dims(), unit_loc, delta).unwrap();
            let result = game.move_unit_by_id(unit_id, dest).unwrap();
            assert!(result.moved_successfully());
            assert_eq!(result.ending_loc(), Some(dest));

            game.force_end_turn();
        }
    }

    #[test]
    fn test_move_unit() {
        let map = MapData::try_from("--0-+-+-1--").unwrap();
        {
            let loc1 = Location{x:2, y:0};
            let loc2 = Location{x:8, y:0};

            let city1tile = map.tile(loc1).unwrap();
            let city2tile = map.tile(loc2).unwrap();
            assert_eq!(city1tile.terrain, Terrain::Land);
            assert_eq!(city2tile.terrain, Terrain::Land);

            let city1 = city1tile.city.as_ref().unwrap();
            let city2 = city2tile.city.as_ref().unwrap();
            assert_eq!(city1.alignment, Alignment::Belligerent{player:0});
            assert_eq!(city2.alignment, Alignment::Belligerent{player:1});
            assert_eq!(city1.loc, loc1);
            assert_eq!(city2.loc, loc2);
        }

        let mut game = Game::new_with_map(map, 2, false, Some(Rc::new(RwLock::new(unit_namer()))), Wrap2d::BOTH);
        assert_eq!(game.current_player(), 0);

        let productions = vec![UnitType::Armor, UnitType::Carrier];
        let players = vec![1, 0];

        for i in 0..2 {
            let loc: Location = game.production_set_requests().next().unwrap();
            assert_eq!(game.set_production_by_loc(loc, productions[i]), Ok(None));

            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, players[i]);
        }

        for _ in 0..11 {
            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1);

            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 0);
        }
        assert_eq!(game.end_turn(), Err(0));

        // Move the armor unit to the right until it attacks the opposing city
        for round in 0..3 {
            assert_eq!(game.unit_orders_requests().count(), 1);
            let unit_id: UnitID = game.unit_orders_requests().next().unwrap();
            let loc = {
                let unit = game.current_player_unit_by_id(unit_id).unwrap();
                assert_eq!(unit.type_, productions[0]);
                unit.loc
            };
            
            let dest_loc = Location{x: loc.x+2, y:loc.y};
            println!("Moving from {} to {}", loc, dest_loc);
            let move_result = game.move_toplevel_unit_by_loc(loc, dest_loc).unwrap();
            println!("Result: {:?}", move_result);
            
            assert_eq!(move_result.unit.type_, UnitType::Armor);
            assert_eq!(move_result.unit.alignment, Alignment::Belligerent{player:0});
            


            // Check the first move component
            assert_eq!(move_result.components.len(), 2);
            let move1 = move_result.components.get(0).unwrap();
            assert_eq!(move1.loc, Location{x:loc.x+1, y:loc.y});
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
                    let production_set_result = game.set_production_by_loc(conquered_city.loc, UnitType::Fighter);
                    assert_eq!(production_set_result, Ok(productions.get(1).cloned()));
                }

            } else {
                // The unit was destroyed
                assert_eq!(move_result.unit.moves_remaining(), 1);
            }

            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 1);

            let result = game.end_turn();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().current_player, 0);
        }
    }

    #[test]
    fn test_terrainwise_movement() {
        let mut map = MapData::try_from(" t-").unwrap();
        map.set_terrain(Location::new(1, 0), Terrain::Water).unwrap();

        let transport_id = map.toplevel_unit_by_loc(Location::new(1,0)).unwrap().id;

        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);

        game.move_unit_by_id_in_direction(transport_id, Direction::Left).unwrap();
        game.move_unit_by_id_in_direction(transport_id, Direction::Right).unwrap();

        assert_eq!(game.move_unit_by_id_in_direction(transport_id, Direction::Right), Err(MoveError::NoRoute {
            id: transport_id,
            src: Location::new(1, 0),
            dest: Location::new(2, 0),
        }));
    }

    #[test]
    fn test_unit_moves_onto_transport() {
        let map = MapData::try_from("---it   ").unwrap();
        let infantry_loc = Location{x: 3, y: 0};
        let transport_loc = Location{x: 4, y: 0};

        let transport_id: UnitID = map.toplevel_unit_id_by_loc(transport_loc).unwrap();

        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);
        let move_result = game.move_toplevel_unit_by_loc(infantry_loc, transport_loc).unwrap();
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
            
            let mut game = Game::new_with_map(map, 2, false, None, Wrap2d::NEITHER);
            
            // Load the infantry onto the transport
            let inf_move = game.move_unit_by_id_in_direction(infantry_id, Direction::Right).unwrap();
            assert!(inf_move.moved_successfully());
            assert_eq!(inf_move.ending_loc(), game.current_player_unit_loc(transport_id));
            assert_eq!(inf_move.ending_carrier(), Some(transport_id));

            // Attack the battleship with the transport
            let move_ = game.move_unit_by_id_in_direction(transport_id, Direction::Right).unwrap();
            if move_.moved_successfully() {
                victorious = true;

                assert!(game.current_player_units().any(|unit| unit.id==infantry_id));
                assert!(game.current_player_units().any(|unit| unit.id==transport_id));

                assert_eq!(game.current_player_unit_by_id(infantry_id).unwrap().loc, Location::new(2, 0));
                assert_eq!(game.current_player_unit_by_id(transport_id).unwrap().loc, Location::new(2, 0));

                assert_eq!(game.current_player_tile(Location::new(0, 0)).unwrap().unit.as_ref(), None);
                assert_eq!(game.current_player_tile(Location::new(1, 0)).unwrap().unit.as_ref(), None);
                {
                    let unit = game.current_player_tile(Location::new(2, 0)).unwrap().unit.as_ref().unwrap();
                    assert_eq!(unit.type_, UnitType::Transport);
                    assert_eq!(unit.id, transport_id);
                    assert!(unit.carried_units().any(|carried_unit| carried_unit.id == infantry_id));
                }

                game.force_end_turn();// ignore remaining moves

                assert!(!game.current_player_units().any(|unit| unit.id==battleship_id));
                assert!(!game.unit_orders_requests().any(|unit_id| unit_id==battleship_id));


            } else {
                defeated = true;

                assert!(!game.current_player_units().any(|unit| unit.id==infantry_id));
                assert!(!game.current_player_units().any(|unit| unit.id==transport_id));

                assert_eq!(game.current_player_unit_by_id(infantry_id), None);
                assert_eq!(game.current_player_unit_by_id(transport_id), None);

                assert_eq!(game.current_player_tile(Location::new(0, 0)).unwrap().unit.as_ref(), None);
                assert_eq!(game.current_player_tile(Location::new(1, 0)).unwrap().unit.as_ref(), None);
                assert_eq!(game.current_player_tile(Location::new(2, 0)).unwrap().unit.as_ref().unwrap().id, battleship_id);

                game.end_turn().unwrap();

                assert!(game.current_player_units().any(|unit| unit.id==battleship_id));
                assert!(game.unit_orders_requests().any(|unit_id| unit_id==battleship_id));
            }
        }
    }

    #[test]
    fn test_set_orders() {
        let map = MapData::try_from("i").unwrap();
        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);
        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();

        assert_eq!(game.current_player_unit_by_id(unit_id).unwrap().orders, None);
        assert_eq!(game.current_player_unit_by_id(unit_id).unwrap().name(), &String::from("Unit_0_0"));

        game.set_orders(unit_id, Orders::Sentry).unwrap();

        assert_eq!(game.current_player_unit_by_id(unit_id).unwrap().orders, Some(Orders::Sentry));
    }

     #[test]
    pub fn test_order_unit_explore() {
        let map = MapData::try_from("i--------------------").unwrap();
        let mut game = Game::new_with_map(map, 1, true, None, Wrap2d::NEITHER);

        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();
        
        let outcome = game.order_unit_explore(unit_id).unwrap();
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
        let mut traversable: HashMap<char,bool> = HashMap::new();
        traversable.insert(' ', false);//water
        traversable.insert('0', true);//friendly city
        traversable.insert('1', true);//enemy city
        traversable.insert('i', false);//friendly unit
        traversable.insert('I', true);//enemy unit

        for up_left in &possible {
            for up in &possible {
                for up_right in &possible {
                    for left in &possible {
                        for right in &possible {
                            for down_left in &possible {
                                for down in &possible {
                                    for down_right in &possible {

                                        let cs: Vec<char> = dirs.iter().map(|dir| match dir {
                                            Direction::UpLeft => up_left,
                                            Direction::Up => up,
                                            Direction::UpRight => up_right,
                                            Direction::Left => left,
                                            Direction::Right => right,
                                            Direction::DownLeft => down_left,
                                            Direction::Down => down,
                                            Direction::DownRight => down_right,
                                        }).cloned().collect();

                                        let s = format!("{}{}{}\n{}i{}\n{}{}{}",
                                            cs[0], cs[1], cs[2], cs[3], cs[4], cs[5], cs[6], cs[7]
                                        );

                                        let map = MapData::try_from(s.clone()).unwrap();
                                        assert_eq!(map.dims(), Dims::new(3, 3));

                                        let game = Game::new_with_map(map, 2, false, None, Wrap2d::BOTH);

                                        let id = game.current_player_toplevel_unit_by_loc(Location{x:1,y:1}).unwrap().id;

                                        let inclusions: Vec<bool> = cs.iter().map(|c| traversable.get(&c).unwrap()).cloned().collect();

                                        assert_eq!(cs.len(), inclusions.len());
                                        assert_eq!(cs.len(), dirs.len());

                                        let src = Location::new(1, 1);
                                        let dests: HashSet<Location> = game.current_player_unit_legal_one_step_destinations(id).unwrap();

                                        for (i, loc) in dirs.iter().map(|dir| {
                                            let v: Vec2d<i32> = (*dir).into();
                                            Location {
                                                x: ((src.x as i32) + v.x) as u16,
                                                y: ((src.y as i32) + v.y) as u16,
                                            }
                                        }).enumerate() {
                                            if inclusions[i] {
                                                assert!(dests.contains(&loc), "Erroneously omitted {:?} on \"{}\"", loc, s.replace("\n","\\n"));
                                            } else {
                                                assert!(!dests.contains(&loc), "Erroneously included {:?} on \"{}\"", loc, s.replace("\n","\\n"));
                                            }
                                        }
                                    }// down_right
                                }// down
                            }// down_left
                        }// right
                    }// left
                }// up_right
            }// up
        }// up_left
    }

    #[test]
    fn test_current_player_unit_legal_one_step_destinations_wrapping() {
        // Make sure the same destinations are found in these cases regardless of wrapping
        for wrapping in Wrap2d::values().iter().cloned() {
            {
                // 1x1
                let mut map = MapData::new(Dims::new(1,1), |_loc| Terrain::Land);
                let unit_id = map.new_unit(Location::new(0,0), UnitType::Infantry, Alignment::Belligerent{player:0}, "Eunice").unwrap();
                let game = Game::new_with_map(map, 1, false, None, wrapping);

                assert!(
                    game.current_player_unit_legal_one_step_destinations(unit_id).unwrap().is_empty()
                );
            }

            {
                // 2x1
                let mut map = MapData::new(Dims::new(2, 1), |_loc| Terrain::Land);
                let unit_id = map.new_unit(Location::new(0,0), UnitType::Infantry, Alignment::Belligerent{player:0}, "Eunice").unwrap();
                                let game = Game::new_with_map(map, 1, false, None, wrapping);

                let dests: HashSet<Location> = game.current_player_unit_legal_one_step_destinations(unit_id).unwrap();
                assert_eq!(dests.len(), 1, "Bad dests: {:?} with wrapping {:?}", dests, wrapping);
                assert!(dests.contains(&Location::new(1, 0)));
            }

            {
                // 3x1
                let mut map = MapData::new(Dims::new(3, 1), |_loc| Terrain::Land);
                let unit_id = map.new_unit(Location::new(1,0), UnitType::Infantry, Alignment::Belligerent{player:0}, "Eunice").unwrap();
                                let game = Game::new_with_map(map, 1, false, None, wrapping);

                let dests: HashSet<Location> = game.current_player_unit_legal_one_step_destinations(unit_id).unwrap();
                assert_eq!(dests.len(), 2, "Bad dests: {:?} with wrapping {:?}", dests, wrapping);
                assert!(dests.contains(&Location::new(0, 0)));
                assert!(dests.contains(&Location::new(2, 0)));
            }

            {
                // 3x1 with infantry in transport
                let mut map = MapData::try_from(".ti").unwrap();
                let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
                let inf_id = map.toplevel_unit_id_by_loc(Location::new(2, 0)).unwrap();
                map.carry_unit_by_id(transport_id, inf_id).unwrap();

                let game = Game::new_with_map(map, 1, false, None, wrapping);

                let dests: HashSet<Location> = game.current_player_unit_legal_one_step_destinations(inf_id).unwrap();
                assert_eq!(dests.len(), 2, "Bad dests: {:?} with wrapping {:?}", dests, wrapping);
                assert!(dests.contains(&Location::new(0, 0)));
                assert!(dests.contains(&Location::new(2, 0)));
            }
        }
    }

    #[test]
    pub fn test_one_step_routes() {
        let mut map = MapData::new(Dims::new(10, 10), |_loc| Terrain::Land);
        let unit_id = map.new_unit(Location::new(0,0), UnitType::Armor, Alignment::Belligerent{player:0}, "Forest Gump").unwrap();

        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);

        // let mut rand = thread_rng();

        for (i, src) in game.dims().iter_locs().enumerate() {
        // for _ in 0..1000 {
        //     let src = game.dims().sample(&mut rand);

            // // Recenter the unit on `src`
            // game.map.relocate_unit_by_id(unit_id, src).unwrap();

            // Recenter the unit on `src`
            if i > 0 {
                game.move_unit_by_id(unit_id, src).unwrap();
                game.order_unit_skip(unit_id).unwrap();
                game.end_turn().unwrap();
            }

            for dir in Direction::values().iter().cloned() {
                let src = game.current_player_unit_loc(unit_id).unwrap();
                let dest = game.wrapping.wrapped_add(game.dims(), src, dir.into()).unwrap();

                game.move_unit_by_id(unit_id, dest).expect(format!("Error moving unit with ID {:?} from {} to {}", unit_id, src, dest).as_str());
                assert_eq!(game.current_player_unit_loc(unit_id), Some(dest), "Wrong location after moving {:?} from {:?} to {:?}", dir, src, dest);

                game.move_unit_by_id(unit_id, src).expect(format!("Error moving unit with ID {:?} from {} to {}", unit_id, dest, src).as_str());
                game.end_turn().unwrap();

                game.move_unit_by_id_in_direction(unit_id, dir).unwrap();
                assert_eq!(game.current_player_unit_loc(unit_id), Some(dest), "Wrong location after moving {:?} from {:?} to {:?}", dir, src, dest);

                game.move_unit_by_id_in_direction(unit_id, dir.opposite()).unwrap();
                game.end_turn().unwrap();
            }
        }
    }

    #[test]
    pub fn test_order_unit_skip() {
        let mut map = MapData::new(Dims::new(10, 10), |_loc| Terrain::Land);
        let unit_id = map.new_unit(Location::new(0, 0), UnitType::Infantry, Alignment::Belligerent{player:0}, "Skipper").unwrap();
        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);

        game.move_unit_by_id_in_direction(unit_id, Direction::Right).unwrap();
        game.end_turn().unwrap();

        game.order_unit_skip(unit_id).unwrap();
        game.end_turn().unwrap();

        assert_eq!(game.unit_orders_requests().next(), Some(unit_id));

        game.current_player_unit_by_id(unit_id).unwrap();
    }

    #[test]
    pub fn test_movement_matches_carry_status() {
        let l1 = Location::new(0, 0);
        let l2 = Location::new(1,0);
        let a = Alignment::Belligerent{player:0};

        for type1 in UnitType::values().iter().cloned() {
            
            let u1 = Unit::new(UnitID::new(0), l2, type1, a, "u1");

            for type2 in UnitType::values().iter().cloned() {

                let u2 = Unit::new(UnitID::new(1), l2, type2, a, "u2");

                let mut map = MapData::new(Dims::new(2,1), |loc| {
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

                let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);
                let result = game.move_unit_by_id_in_direction(u1.id, Direction::Right);
                
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
        let unit_id = map.new_unit(loc, UnitType::Submarine,
            Alignment::Belligerent{player:0}, "K-19").unwrap();

        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);
        
        

        for _ in 0..9 {
            game.move_unit_by_id_in_direction(unit_id, Direction::Right).unwrap();
            loc = loc.shift_wrapped(Direction::Right, game.dims(), game.wrapping()).unwrap();

            let unit = game.current_player_toplevel_unit_by_loc(loc).unwrap();
            assert_eq!(unit.id, unit_id);

            game.force_end_turn();
        }

    }

    #[test]
    fn test_transport_moves_on_transport_unloaded() {
        
        let l1 = Location::new(0,0);
        let l2 = Location::new(1,0);

        let map = MapData::try_from("tt").unwrap();

        let t1_id = map.toplevel_unit_id_by_loc(l1).unwrap();

        {
            let mut game = Game::new_with_map(map.clone(), 1, false, None, Wrap2d::NEITHER);

            game.move_unit_by_id_in_direction(t1_id, Direction::Right)
                .expect_err("Transport should not be able to move onto transport");
        }


        let mut map2 = map.clone();
        map2.new_city(l2, Alignment::Belligerent{player:0}, "city").unwrap();

        let mut game = Game::new_with_map(map2, 1, false, None, Wrap2d::NEITHER);

        game.move_unit_by_id_in_direction(t1_id, Direction::Right)
            .expect_err("Transport should not be able to move onto transport");
    }

    #[test]
    fn test_transport_moves_on_transport_loaded() {
        
        let l1 = Location::new(1,0);
        let l2 = Location::new(2,0);

        let mut map = MapData::try_from(".tt.").unwrap();

        let t1_id = map.toplevel_unit_id_by_loc(l1).unwrap();
        let t2_id = map.toplevel_unit_id_by_loc(l2).unwrap();

        for i in 0..3 {
            println!("{}", i);
            let id = map.new_unit(Location::new(0,0), UnitType::Infantry, Alignment::Belligerent{player:0},
            format!("inf{}", i)).unwrap();
            map.carry_unit_by_id(t1_id, id).unwrap();
        }

        for i in 0..3 {
            let id = map.new_unit(Location::new(3,0), UnitType::Infantry, Alignment::Belligerent{player:0},
            format!("inf{}", i+100)).unwrap();
            map.carry_unit_by_id(t2_id, id).unwrap();
        }

        

        {
            let mut game = Game::new_with_map(map.clone(), 1, false, None, Wrap2d::NEITHER);

            game.move_unit_by_id_in_direction(t1_id, Direction::Right)
                .expect_err("Transport should not be able to move onto transport");

            game.move_unit_by_id_in_direction(t2_id, Direction::Left)
                .expect_err("Transport should not be able to move onto transport");

        }


        let mut map2 = map.clone();
        map2.new_city(l2, Alignment::Belligerent{player:0}, "city").unwrap();

        let mut game = Game::new_with_map(map2, 1, false, None, Wrap2d::NEITHER);

        game.move_unit_by_id_in_direction(t1_id, Direction::Right)
            .expect_err("Transport should not be able to move onto transport");
    }

    #[test]
    fn test_embark_disembark() {
        let map = MapData::try_from("at -").unwrap();
        let armor_id = map.toplevel_unit_id_by_loc(Location::new(0,0)).unwrap();
        let transport_id = map.toplevel_unit_id_by_loc(Location::new(1,0)).unwrap();
        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        // Embark
        game.move_unit_by_id_in_direction(armor_id, Direction::Right).unwrap();
        assert_eq!(game.current_player_unit_loc(armor_id), Some(Location::new(1,0)));
        assert_eq!(game.current_player_unit_loc(transport_id), Some(Location::new(1,0)));

        // Move transport
        game.move_unit_by_id_in_direction(transport_id, Direction::Right).unwrap();
        assert_eq!(game.current_player_unit_loc(armor_id), Some(Location::new(2,0)));
        assert_eq!(game.current_player_unit_loc(transport_id), Some(Location::new(2,0)));
        
        // Disembark
        game.move_unit_by_id_in_direction(armor_id, Direction::Right).unwrap();
        assert_eq!(game.current_player_unit_loc(armor_id), Some(Location::new(3,0)));
        assert_eq!(game.current_player_unit_loc(transport_id), Some(Location::new(2,0)));
    }

    #[test]
    fn test_embark_disembark_via_goto() {
        let map = MapData::try_from("at -").unwrap();
        let armor_id = map.toplevel_unit_id_by_loc(Location::new(0,0)).unwrap();
        let transport_id = map.toplevel_unit_id_by_loc(Location::new(1,0)).unwrap();
        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        // Embark
        game.order_unit_go_to(armor_id, Location::new(1,0)).unwrap();
        assert_eq!(game.current_player_unit_loc(armor_id), Some(Location::new(1,0)));
        assert_eq!(game.current_player_unit_loc(transport_id), Some(Location::new(1,0)));

        // Move transport
        game.order_unit_go_to(transport_id, Location::new(2, 0)).unwrap();
        assert_eq!(game.current_player_unit_loc(armor_id), Some(Location::new(2,0)));
        assert_eq!(game.current_player_unit_loc(transport_id), Some(Location::new(2,0)));
        
        // Disembark
        game.order_unit_go_to(armor_id, Location::new(3, 0)).unwrap();
        assert_eq!(game.current_player_unit_loc(armor_id), Some(Location::new(3,0)));
        assert_eq!(game.current_player_unit_loc(transport_id), Some(Location::new(2,0)));
    }

    #[test]
    fn test_shortest_paths_carrying() {
        let map = MapData::try_from("t t  ").unwrap();

        let mut game = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        game.move_toplevel_unit_by_loc(Location::new(0,0), Location::new(4, 0))
            .expect_err("Transports shouldn't traverse transports on their way somewhere");
    }

    #[test]
    fn test_valid_productions_conservative() {
        let map = MapData::try_from("...\n.0.\n...").unwrap();
        let game = Game::new_with_map(map, 1, false, None, Wrap2d::NEITHER);

        let city_loc = game.production_set_requests().next().unwrap();

        let prods: HashSet<UnitType> = game.valid_productions_conservative(city_loc).collect();

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
        let unit_id = map.new_unit(Location::new(0,0), UnitType::Fighter, Alignment::Belligerent{player:0}, "Han Solo").unwrap();

        let mut game = Game::new_with_map(map, 1, true, None, Wrap2d::BOTH);

        let unit_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
        let dest = game.wrapping().wrapped_add(game.dims(), unit_loc, Vec2d::new(5, 5)).unwrap();
        game.move_unit_by_id(unit_id, dest).unwrap();
    }

    #[test]
    fn test_disband_unit_by_id() {
        {
            let map = MapData::try_from("i").unwrap();
            let mut game = Game::new_with_map(map, 1, true, None, Wrap2d::NEITHER);
            let id = UnitID::new(0);

            let unit = game.current_player_unit_by_id(id).cloned().unwrap();

            assert!(game.unit_orders_requests().find(|unit_id| *unit_id == id ).is_some());

            assert_eq!(game.disband_unit_by_id(id), Ok(unit));

            let id2 = UnitID::new(1);

            assert_eq!(game.disband_unit_by_id(id2), Err(GameError::NoSuchUnit{id:id2}));

            assert!(game.unit_orders_requests().find(|unit_id| *unit_id == id ).is_none());
        }

        {
            let map2 = MapData::try_from("it ").unwrap();
            let infantry_id = map2.toplevel_unit_id_by_loc(Location::new(0,0)).unwrap();
            let transport_id = map2.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();

            let mut game2 = Game::new_with_map(map2, 1, true, None, Wrap2d::NEITHER);

            assert!(game2.unit_orders_requests().find(|unit_id| *unit_id==infantry_id).is_some());
            assert!(game2.unit_orders_requests().find(|unit_id| *unit_id==transport_id).is_some());

            game2.move_unit_by_id_in_direction(infantry_id, Direction::Right).unwrap();

            game2.force_end_turn();

            let infantry = game2.current_player_unit_by_id(infantry_id).cloned().unwrap();

            assert!(game2.unit_orders_requests().find(|unit_id| *unit_id==infantry_id).is_some());
            assert!(game2.unit_orders_requests().find(|unit_id| *unit_id==transport_id).is_some());

            assert_eq!(game2.disband_unit_by_id(infantry_id), Ok(infantry));

            assert!(game2.unit_orders_requests().find(|unit_id| *unit_id==infantry_id).is_none());
            assert!(game2.unit_orders_requests().find(|unit_id| *unit_id==transport_id).is_some());

            let transport = game2.current_player_unit_by_id(transport_id).cloned().unwrap();

            assert_eq!(game2.disband_unit_by_id(transport_id), Ok(transport));

            assert!(game2.unit_orders_requests().find(|unit_id| *unit_id==infantry_id).is_none());
            assert!(game2.unit_orders_requests().find(|unit_id| *unit_id==transport_id).is_none());
        }

        {

            let map = MapData::try_from("ii").unwrap();
            let a = map.toplevel_unit_id_by_loc(Location::new(0,0)).unwrap();
            let b = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();

            let mut game = Game::new_with_map(map, 1, true, None, Wrap2d::NEITHER);

            assert!(game.disband_unit_by_id(a).is_ok());

            assert!(game.current_player_unit_by_id(a).is_none());

            assert!(game.move_unit_by_id_in_direction(b, Direction::Left).is_ok());

        }
    }
}
