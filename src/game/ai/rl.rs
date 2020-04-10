//! Reinforcement learning-based AI

use std::collections::HashSet;

use serde::{Deserialize,Serialize};

use rsrl::{
    fa::EnumerableStateActionFunction,
};

use crate::{
    game::{
        Game,
        player::TurnTaker, unit::UnitType,
    }, util::Direction,
};

#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd)]
pub enum UmpireAction {
    SetNextCityProduction{unit_type: UnitType},
    MoveNextUnit{direction: Direction},
    SkipNextUnit,
}

impl UmpireAction {
    pub fn legal_actions(game: &Game) -> HashSet<Self> {
        let mut a = HashSet::new();

        debug_assert!(!game.turn_is_done());


        //TODO Possibly consider actions for all cities instead of just the next one that isn't set yet
        if let Some(city_loc) = game.production_set_requests().next() {
            for unit_type in game.valid_productions_conservative(city_loc) {
                a.insert(UmpireAction::SetNextCityProduction{unit_type});
            }
        }

        //TODO Possibly consider actions for all units instead of just the next one that needs orders
        if let Some(unit_id) = game.unit_orders_requests().next() {
            for direction in game.current_player_unit_legal_directions(unit_id).unwrap() {
                a.insert(UmpireAction::MoveNextUnit{direction});
            }
            a.insert(UmpireAction::SkipNextUnit);
        }

        debug_assert!(!a.is_empty());

        a
    }

    // UnitType::Infantry,    0
    // UnitType::Armor,       1
    // UnitType::Fighter,     2
    // UnitType::Bomber,      3
    // UnitType::Transport,   4
    // UnitType::Destroyer,   5
    // UnitType::Submarine,   6
    // UnitType::Cruiser,     7
    // UnitType::Battleship,  8
    // UnitType::Carrier      9
    // Direction::Up,         10
    // Direction::Down,       11
    // Direction::Left,       12
    // Direction::Right,      13
    // Direction::UpLeft,     14
    // Direction::UpRight,    15
    // Direction::DownLeft,   16
    // Direction::DownRight,  17
    // SkipNextTurn           18
    pub fn possible_actions() -> Vec<Self> {
        let mut a = Vec::new();
        for unit_type in UnitType::values().iter().cloned() {
            a.push(UmpireAction::SetNextCityProduction{unit_type});
        }
        for direction in Direction::values().iter().cloned() {
            a.push(UmpireAction::MoveNextUnit{direction});
        }
        a.push(UmpireAction::SkipNextUnit);

        a
    }

    pub fn from_idx(mut idx: usize) -> Result<Self,()> {
        let unit_types = UnitType::values();
        if unit_types.len() > idx {
            return Ok(UmpireAction::SetNextCityProduction{unit_type: unit_types[idx]});
        }

        idx -= unit_types.len();

        let dirs = Direction::values();
        if dirs.len() > idx {
            return Ok(UmpireAction::MoveNextUnit{direction: dirs[idx]});
        }
        
        idx -= dirs.len();

        if idx == 0 {
            return Ok(UmpireAction::SkipNextUnit);
        }

        Err(())
    }

    pub fn to_idx(&self) -> usize {
        Self::possible_actions().into_iter().position(|a| *self == a).unwrap()
    }

    pub fn take(&self, game: &mut Game) {
        match *self {
            UmpireAction::SetNextCityProduction{unit_type} => {
                let city_loc = game.production_set_requests().next().unwrap();
                game.set_production_by_loc(city_loc, unit_type).unwrap();
            },
            UmpireAction::MoveNextUnit{direction} => {
                let unit_id = game.unit_orders_requests().next().unwrap();
                debug_assert!({
                    let legal: HashSet<Direction> = game.current_player_unit_legal_directions(unit_id).unwrap()
                                                             .collect();

                    // println!("legal moves: {}", legal.len());
                    
                    legal.contains(&direction)
                });

                game.move_unit_by_id_in_direction(unit_id, direction).unwrap();
            },
            UmpireAction::SkipNextUnit => {
                let unit_id = game.unit_orders_requests().next().unwrap();
                game.order_unit_skip(unit_id).unwrap();
            }
        }
    }
}

pub fn find_legal_max<Q:EnumerableStateActionFunction<Game>>(q_func: &Q, state: &Game, avoid_skip: bool) -> (usize, f64) {

    let mut legal = UmpireAction::legal_actions(state);

    let possible = UmpireAction::possible_actions();

    let mut qs = q_func.evaluate_all(state);

    if legal.contains(&UmpireAction::SkipNextUnit) && legal.len() > 1 && avoid_skip {
        legal.remove(&UmpireAction::SkipNextUnit);
        qs.remove(UmpireAction::SkipNextUnit.to_idx());
    }

    qs.into_iter().enumerate()
        .filter(|(i,_x)| legal.contains(possible.get(*i).unwrap()))
        .max_by(|a,b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap()
}

#[derive(Deserialize, Serialize)]
pub struct RL_AI<Q> {
    q_func: Q,
    avoid_skip: bool,
}
impl <Q: EnumerableStateActionFunction<Game>> RL_AI<Q> {
    pub fn new(q_func: Q, avoid_skip: bool) -> Self {
        Self { q_func, avoid_skip }
    }

    fn _take_turn_unended(&mut self, game: &mut Game) {
        while !game.turn_is_done() {
            let action_idx = find_legal_max(&self.q_func, game, self.avoid_skip).0;
            let action = UmpireAction::from_idx(action_idx).unwrap();
            action.take(game);
        }
    }
}
impl <Q: EnumerableStateActionFunction<Game>> TurnTaker for RL_AI<Q> {
    fn take_turn(&mut self, game: &mut Game) {
        self._take_turn_unended(game);

        game.end_turn().unwrap();
    }

    fn take_turn_clearing(&mut self, game: &mut Game) {
        self._take_turn_unended(game);

        game.end_turn_clearing().unwrap();
    }
}