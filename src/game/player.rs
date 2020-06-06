use std::{
    collections::HashSet,
    convert::TryFrom,
};

use crate::{
    game::{
        Game,
        GameError,
        Proposed,
        TurnNum,
        TurnStart,
        city::{
            City,
            CityID,
        },
        map::tile::Tile,
        move_::{
            Move,
            MoveError,
            MoveResult,
        },
        obs::{
            Obs,
            ObsTracker,
        },
        unit::{
            Unit,
            UnitID,
            UnitType,
            orders::{
                OrdersResult,
            },
        },
    },
    util::{
        Dims,
        Direction,
        Location,
        Wrap2d,
    }, cli::Specified,
};
use super::ai::AISpec;


pub type PlayerNum = usize;

#[derive(Clone,Debug,Eq,Hash,PartialEq)]
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

    // /// The text description of this type of player
    // pub fn desc(&self) -> String {
    //     match self {
    //         Self::Human => String::from("human"),
    //         Self::AI(ai_type) => ai_type.desc(),
    //     }
    // }

    // pub fn spec(&self) -> String {
    //     match self {
    //         Self::Human => "h".to_string(),
    //         Self::AI(ai_type) => ai_type.spec()
    //     }
    // }

    // /// The character used to specify this variant on the command line
    // pub fn spec_char(&self) -> char {
    //     match self {
    //         Self::Human => 'h',
    //         Self::Random => 'r',
    //         Self::AI(level) => std::char::from_digit(*level as u32, 10).unwrap(),
    //     }
    // }

    // pub fn from_spec_char(c: char) -> Result<Self,String> {
    //     match c {
    //         'h' => Ok(Self::Human),
    //         c => AIType::try_from(c).map(Self::AI)
    //     }
    // }
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
            Self::AI(ai_type) => ai_type.spec()
        }
    }
}

impl TryFrom<String> for PlayerType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "h" | "human" => Ok(Self::Human),
            _ => AISpec::try_from(value).map(Self::AI)
        }
    }
}

impl Into<String> for PlayerType {
    fn into(self) -> String {
        match self {
            Self::Human => "h".to_string(),
            Self::AI(ai_type) => ai_type.into()
        }
    }
}

// impl Into<Rc<RefCell<dyn TurnTaker>>> for PlayerType {
//     fn into(self) -> Rc<RefCell<dyn TurnTaker>> {

//     }
// }

// impl Into<String> for PlayerType {
//     fn into(self) -> String {

//     }
// }

pub struct PlayerTurnControl<'a> {
    game: &'a mut Game,

    /// Which player is this control shim representing? A copy of `Game::current_player`'s result. Shouldn't get stale
    /// since we lock down anything that would change who the current player is. We do this for convenience.
    pub player: PlayerNum,

    clear_completed_productions: bool,

    end_turn_on_drop: bool,
}
impl <'a> PlayerTurnControl<'a> {
    pub fn new(game: &'a mut Game, end_turn_on_drop: bool, clear_completed_productions: bool) -> Self {
        let player = game.current_player();
        Self { game, player, clear_completed_productions, end_turn_on_drop }
    }

    pub fn new_clearing(game: &'a mut Game) -> Self {
        let player = game.current_player();
        Self { game, player, clear_completed_productions: true, end_turn_on_drop: true }
    }

    pub fn num_players(&self) -> PlayerNum {
        self.game.num_players()
    }

    pub fn turn_is_done(&self) -> bool {
        self.game.turn_is_done()
    }

    /// The victor---if any---meaning the player who has defeated all other players.
    /// 
    /// It is the user's responsibility to check for a victor---the game will continue to function even when somebody
    /// has won.
    pub fn victor(&self) -> Option<PlayerNum> {
        self.game.victor()
    }

    pub fn current_player_unit_legal_one_step_destinations(&self, unit_id: UnitID) -> Result<HashSet<Location>,GameError> {
        self.game.current_player_unit_legal_one_step_destinations(unit_id)
    }

    /// The current player's most recent observation of the tile at location `loc`, if any
    pub fn current_player_tile(&self, loc: Location) -> Option<&Tile> {
        self.game.current_player_tile(loc)
    }

    /// The current player's observation at location `loc`
    pub fn current_player_obs(&self, loc: Location) -> &Obs {
        self.game.current_player_obs(loc)
    }

    pub fn current_player_observations(&self) -> &ObsTracker {
        self.game.current_player_observations()
    }

    /// Every city controlled by the current player
    pub fn current_player_cities(&self) -> impl Iterator<Item=&City> {
        self.game.current_player_cities()
    }

    /// All cities controlled by the current player which have a production target set
    pub fn current_player_cities_with_production_target(&self) -> impl Iterator<Item=&City> {
        self.game.current_player_cities_with_production_target()
    }

    /// The number of cities controlled by the current player which either have a production target or are NOT set to be ignored when requesting productions to be set
    /// 
    /// This basically lets us make sure a player doesn't set all their cities' productions to none since right now the UI has no way of getting out of that situation
    /// 
    /// FIXME Get rid of this and just make the UI smarter
    #[deprecated]
    pub fn player_cities_producing_or_not_ignored(&self) -> usize {
        self.game.player_cities_producing_or_not_ignored()
    }

    /// Every unit controlled by the current player
    pub fn current_player_units(&self) -> impl Iterator<Item=&Unit> {
        self.game.current_player_units()
    }

    /// If the current player controls a city at location `loc`, return it
    pub fn current_player_city_by_loc(&self, loc: Location) -> Option<&City> {
        self.game.current_player_city_by_loc(loc)
    }

    /// If the current player controls a city with ID `city_id`, return it
    pub fn current_player_city_by_id(&self, city_id: CityID) -> Option<&City> {
        self.game.current_player_city_by_id(city_id)
    }

    /// If the current player controls a unit with ID `id`, return it
    pub fn current_player_unit_by_id(&self, id: UnitID) -> Option<&Unit> {
        self.game.current_player_unit_by_id(id)
    }

    /// If the current player controls a unit with ID `id`, return its location
    pub fn current_player_unit_loc(&self, id: UnitID) -> Option<Location> {
        self.game.current_player_unit_loc(id)
    }

    /// If the current player controls the top-level unit at location `loc`, return it
    pub fn current_player_toplevel_unit_by_loc(&self, loc: Location) -> Option<&Unit> {
        self.game.current_player_toplevel_unit_by_loc(loc)
    }

    pub fn production_set_requests(&'a self) -> impl Iterator<Item=Location> + 'a {
        self.game.production_set_requests()
    }

    /// Which if the current player's units need orders?
    /// 
    /// In other words, which of the current player's units have no orders and have moves remaining?
    pub fn unit_orders_requests(&'a self) -> impl Iterator<Item=UnitID> + 'a {
        self.game.unit_orders_requests()
    }

    /// Which if the current player's units need orders?
    /// 
    /// In other words, which of the current player's units have no orders and have moves remaining?
    pub fn units_with_orders_requests(&'a self) -> impl Iterator<Item=&Unit> + 'a {
        self.game.units_with_orders_requests()
    }

    pub fn units_with_pending_orders(&'a self) -> impl Iterator<Item=UnitID> + 'a {
        self.game.units_with_pending_orders()
    }


    // Movement-related methods

    pub fn move_toplevel_unit_by_id(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        self.game.move_toplevel_unit_by_id(unit_id, dest)
    }

    pub fn move_toplevel_unit_by_id_avoiding_combat(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        self.game.move_toplevel_unit_by_id_avoiding_combat(unit_id, dest)
    }

    pub fn move_toplevel_unit_by_loc(&mut self, src: Location, dest: Location) -> MoveResult {
        self.game.move_toplevel_unit_by_loc(src, dest)
    }

    pub fn move_toplevel_unit_by_loc_avoiding_combat(&mut self, src: Location, dest: Location) -> MoveResult {
        self.game.move_toplevel_unit_by_loc_avoiding_combat(src, dest)
    }

    pub fn move_unit_by_id_in_direction(&mut self, id: UnitID, direction: Direction) -> MoveResult {
        self.game.move_unit_by_id_in_direction(id, direction)
    }

    pub fn move_unit_by_id(&mut self, unit_id: UnitID, dest: Location) -> MoveResult {
        self.game.move_unit_by_id(unit_id, dest)
    }

    pub fn propose_move_unit_by_id(&self, id: UnitID, dest: Location) -> Proposed<Result<Move,MoveError>> {
        self.game.propose_move_unit_by_id(id, dest)
    }

    pub fn move_unit_by_id_avoiding_combat(&mut self, id: UnitID, dest: Location) -> MoveResult {
        self.game.move_unit_by_id_avoiding_combat(id, dest)
    }

    pub fn propose_move_unit_by_id_avoiding_combat(&self, id: UnitID, dest: Location) -> Proposed<MoveResult> {
        self.game.propose_move_unit_by_id_avoiding_combat(id, dest)
    }

    pub fn disband_unit_by_id(&mut self, id: UnitID) -> Result<Unit,GameError> {
        self.game.disband_unit_by_id(id)
    }

    /// Sets the production of the current player's city at location `loc` to `production`.
    /// 
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    pub fn set_production_by_loc(&mut self, loc: Location, production: UnitType) -> Result<Option<UnitType>,GameError> {
        self.game.set_production_by_loc(loc, production)
    }

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    /// 
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    pub fn set_production_by_id(&mut self, city_id: CityID, production: UnitType) -> Result<Option<UnitType>,GameError> {
        self.game.set_production_by_id(city_id, production)
    }

    //FIXME Restrict to current player cities
    pub fn clear_production_without_ignoring(&mut self, loc: Location) -> Result<(),String> {
        self.game.clear_production_without_ignoring(loc)
    }

    //FIXME Restrict to current player cities
    pub fn clear_production_and_ignore(&mut self, loc: Location) -> Result<(),String> {
        self.game.clear_production_and_ignore(loc)
    }

    pub fn turn(&self) -> TurnNum {
        self.game.turn()
    }

    pub fn current_player(&self) -> PlayerNum {
        self.game.current_player()
    }

    /// The logical dimensions of the game map
    pub fn dims(&self) -> Dims {
        self.game.dims()
    }

    pub fn wrapping(&self) -> Wrap2d {
        self.game.wrapping()
    }

    /// Units that could be produced by a city located at the given location
    pub fn valid_productions(&'a self, loc: Location) -> impl Iterator<Item=UnitType> + 'a {
        self.game.valid_productions(loc)
    }

    /// Units that could be produced by a city located at the given location, allowing only those which can actually
    /// leave the city (rather than attacking neighbor cities, potentially not occupying them)
    pub fn valid_productions_conservative<'b>(&'b self, loc: Location) -> impl Iterator<Item=UnitType> + 'b {
        self.game.valid_productions_conservative(loc)
    }

    /// If the current player controls a unit with ID `id`, order it to sentry
    pub fn order_unit_sentry(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_sentry(unit_id)
    }

    pub fn order_unit_skip(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_skip(unit_id)
    }

    pub fn order_unit_go_to(&mut self, unit_id: UnitID, dest: Location) -> OrdersResult {
        self.game.order_unit_go_to(unit_id, dest)
    }

    /// Simulate ordering the specified unit to go to the given location
    pub fn propose_order_unit_go_to(&self, unit_id: UnitID, dest: Location) -> Proposed<OrdersResult> {
        self.game.propose_order_unit_go_to(unit_id, dest)
    }

    pub fn order_unit_explore(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_explore(unit_id)
    }

    /// Simulate ordering the specified unit to explore.
    pub fn propose_order_unit_explore(&self, unit_id: UnitID) -> Proposed<OrdersResult> {
        self.game.propose_order_unit_explore(unit_id)
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    pub fn activate_unit_by_loc(&mut self, loc: Location) -> Result<(),GameError> {
        self.game.activate_unit_by_loc(loc)
    }

    pub fn propose_end_turn(&self) -> (Game,Result<TurnStart,PlayerNum>) {
        let mut game = self.game.clone();
        let result = game.end_turn();
        (game, result)
    }

    pub fn apply_proposal<T>(&mut self, proposal: Proposed<T>) -> T {
        proposal.apply(self.game)
    }

    // // ----- Consuming methods -----
    // fn end_turn(self) -> Result<TurnStart,PlayerNum> {
    //     self.game.end_turn()
    // }
}

/// If for whatever reason a careless user fails to end the turn, we do it for them so the game continues to advance.
/// 
/// This forces the turn to end regardless of the state of production and orders requests.
impl <'a> Drop for PlayerTurnControl<'a> {
    fn drop(&mut self) {
        if self.end_turn_on_drop {
            if self.clear_completed_productions {
                self.game.force_end_turn_clearing();
            } else {
                self.game.force_end_turn();
            }
        }
    }
}

/// Take a turn with only the knowledge of game state an individual player should have
/// This is the main thing to use
pub trait LimitedTurnTaker {
    fn take_turn(&mut self, ctrl: &mut PlayerTurnControl);
}

/// Take a turn with full knowledge of the game state
/// 
/// This is a kludgey escape hatch for an issue in AI training where we need the whole state. It is crucial for
/// implementors to guarantee that the player's turn is ended (and only the player's turn---no further turns) by the
/// end of the `take_turn` function call.
pub trait TurnTaker {
    fn take_turn_not_clearing(&mut self, game: &mut Game);

    fn take_turn_clearing(&mut self, game: &mut Game);

    fn take_turn(&mut self, game: &mut Game, clear_at_end_of_turn: bool) {
        if clear_at_end_of_turn {
            self.take_turn_clearing(game);
        } else {
            self.take_turn_not_clearing(game);
        }
    }
}

impl <T:LimitedTurnTaker> TurnTaker for T {
    fn take_turn_not_clearing(&mut self, game: &mut Game) {
        let mut ctrl = game.player_turn_control(game.current_player());
        loop {
            <Self as LimitedTurnTaker>::take_turn(self, &mut ctrl);

            if ctrl.turn_is_done() {
                break;
            }
        }
    }

    fn take_turn_clearing(&mut self, game: &mut Game) {
        let mut ctrl = game.player_turn_control_clearing(game.current_player());
        loop {
            <Self as LimitedTurnTaker>::take_turn(self, &mut ctrl);

            if ctrl.turn_is_done() {
                break;
            }
        }
    }
}