use std::{
    collections::{
        BTreeSet,
        HashSet,
    },
};

use crate::{
    game::{
        Game,
        GameError,
        ProposedAction,
        TurnNum,
        TurnStart,
        city::{
            City,
            CityID,
        },
        map::tile::Tile,
        move_::{
            MoveError,
            MoveResult,
            ProposedMove,
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
                ProposedSetAndFollowOrders,
            },
        },
    },
    util::{
        Dims,
        Direction,
        Location,
        Wrap2d,
    },
};


pub type PlayerNum = usize;

#[derive(Clone)]
pub enum PlayerType {
    Human,
    Random,
}

impl PlayerType {
    pub fn values() -> [Self; 2] {
        [Self::Human, Self::Random]
    }

    pub fn desc(&self) -> &str {
        match self {
            Self::Human => "human",
            Self::Random => "random",
        }
    }

    /// The character used to specify this variant on the command line
    pub fn spec_char(&self) -> char {
        match self {
            Self::Human => 'h',
            Self::Random => 'r',
        }
    }

    pub fn from_spec_char(c: char) -> Result<Self,String> {
        match c {
            'h' => Ok(Self::Human),
            'r' => Ok(Self::Random),
            _ => Err(format!("'{}' does not correspond to a player type", c))
        }
    }
}


// pub struct PlayerGameControl<'a> {
//     game: &'a mut Game,
// }
// impl <'a> PlayerGameControl<'a> {
//     fn unit_orders_requests<'b>(&'b self) -> impl Iterator<Item=UnitID> + 'b {
//         self.game.unit_orders_requests()
//     }

//     fn production_set_requests<'b>(&'b self) -> impl Iterator<Item=Location> + 'b {
//         self.game.production_set_requests()
//     }

//     fn set_production(&mut self, loc: Location, production: UnitType) -> Result<(),String> {
//         self.game.set_production(loc, production)
//     }
// }

// pub trait Player {
//     // fn move_unit(&mut self, unit_id: UnitID, game: &PlayerGameView) -> Direction;
    
//     // fn set_production(&mut self, city_id: CityID, game: &PlayerGameView) -> UnitType;

//     // fn take_turn(&mut self, game: &mut Game);

//     fn take_turn(&mut self, game: &Game, tx: &Sender<PlayerCommand>);
// }

pub struct ProposedActionWrapper<T:ProposedAction> {
    pub item: T,
}
impl <T:ProposedAction> ProposedActionWrapper<T> {
    fn new(item: T) -> Self {
        Self { item }
    }

    pub fn take(self, ctrl: &mut PlayerTurnControl) -> T::Outcome {
        self.item.take(ctrl.game)
    }
}

pub struct PlayerTurnControl<'a> {
    game: &'a mut Game,

    /// Which player is this control shim representing? A copy of `Game::current_player`'s result. Shouldn't get stale
    /// since we lock down anything that would change who the current player is. We do this for convenience.
    pub player: PlayerNum,
}
impl <'a> PlayerTurnControl<'a> {
    pub fn new(game: &'a mut Game) -> Self {
        let player = game.current_player();
        Self { game, player }
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

    pub fn propose_move_unit_by_id(&self, id: UnitID, dest: Location) -> Result<ProposedActionWrapper<ProposedMove>,MoveError> {
        self.game.propose_move_unit_by_id(id, dest)
            .map(ProposedActionWrapper::new)
    }

    pub fn move_unit_by_id_avoiding_combat(&mut self, id: UnitID, dest: Location) -> MoveResult {
        self.game.move_unit_by_id_avoiding_combat(id, dest)
    }

    pub fn propose_move_unit_by_id_avoiding_combat(&self, id: UnitID, dest: Location) -> Result<ProposedActionWrapper<ProposedMove>,MoveError> {
        self.game.propose_move_unit_by_id_avoiding_combat(id, dest)
            .map(ProposedActionWrapper::new)
    }

    /// Sets the production of the current player's city at location `loc` to `production`.
    /// 
    /// Returns GameError::NoCityAtLocation if no city belonging to the current player exists at that location.
    pub fn set_production_by_loc(&mut self, loc: Location, production: UnitType) -> Result<(),GameError> {
        self.game.set_production_by_loc(loc, production)
    }

    /// Sets the production of the current player's city with ID `city_id` to `production`.
    /// 
    /// Returns GameError::NoCityAtLocation if no city with the given ID belongs to the current player.
    pub fn set_production_by_id(&mut self, city_id: CityID, production: UnitType) -> Result<(),GameError> {
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
    pub fn valid_productions(&self, loc: Location) -> BTreeSet<UnitType> {
        self.game.valid_productions(loc)
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
    pub fn propose_order_unit_go_to(&mut self, unit_id: UnitID, dest: Location) -> ProposedActionWrapper<ProposedSetAndFollowOrders> {
        ProposedActionWrapper::new(self.game.propose_order_unit_go_to(unit_id, dest))
    }

    pub fn order_unit_explore(&mut self, unit_id: UnitID) -> OrdersResult {
        self.game.order_unit_explore(unit_id)
    }

    /// Simulate ordering the specified unit to explore.
    pub fn propose_order_unit_explore(&mut self, unit_id: UnitID) -> ProposedActionWrapper<ProposedSetAndFollowOrders> {
        ProposedActionWrapper::new(self.game.propose_order_unit_explore(unit_id))
    }

    /// If a unit at the location owned by the current player exists, activate it and any units it carries
    pub fn activate_unit_by_loc(&mut self, loc: Location) -> Result<(),GameError> {
        self.game.activate_unit_by_loc(loc)
    }

    pub fn propose_end_turn(&self) -> Result<TurnStart,PlayerNum> {
        let mut game = self.game.clone();
        game.end_turn()
    }

    // ----- Consuming methods -----
    fn end_turn(self) -> Result<TurnStart,PlayerNum> {
        self.game.end_turn()
    }
}

/// If for whatever reason a careless user fails to end the turn, we do it for them so the game continues to advance.
/// 
/// This forces the turn to end regardless of the state of production and orders requests.
impl <'a> Drop for PlayerTurnControl<'a> {
    fn drop(&mut self) {
        self.game.force_end_turn();
    }
}

pub trait TurnTaker {
    fn take_turn(&mut self, ctrl: &mut PlayerTurnControl);
}

pub trait Player: Send {
    fn play(&mut self, game: PlayerTurnControl);
}

/// A player whose action is specified turn by turn.
/// 
/// A shim over game is given which allows appropriate read-only actions and a limited number of mutations to enable
/// gameplay.
pub trait TurnPlayer: Send {
    fn take_turn(&mut self, game: &mut PlayerTurnControl);
}

impl <P:TurnPlayer> Player for P {
    fn play(&mut self, mut game: PlayerTurnControl) {

        loop {
            self.take_turn(&mut game);

            if let Ok(_) = game.propose_end_turn() {
                break;
            }
        }
        game.end_turn().unwrap();
    }
}

/// A few reified commands a player can use to take their turn
///
/// Meant to be passed over a channel to the game engine.
#[derive(Clone)]
pub enum PlayerCommand {
    SetProduction {
        city_id: CityID,
        production: UnitType,
    },
    OrderUnitMoveInDirection {
        unit_id: UnitID,
        direction: Direction,
    },
    OrderUnitGoTo {
        unit_id: UnitID,
        dest: Location,
    },
    OrderUnitExplore {
        unit_id: UnitID,
    },
    OrderUnitSentry {
        unit_id: UnitID,
    },
    OrderUnitMove {
        unit_id: UnitID,
        dest: Location,
    },
}