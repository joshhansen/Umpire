//! Tool to train Umpire AI
//! 
//! Strategy:
//! First we bootstrap the AI by having it play against a random baseline.
//! Then we train it against itself.
//! These initial games should have small maps and only two players.
//! 
//! Once we have a simple AI, incorporate it into the UI.
use std::collections::{
    BinaryHeap,
    HashMap,
};

use rand::seq::SliceRandom;

use rsrl::{
    run, make_shared, Evaluation, SerialExperiment,
    control::td::QLearning,
    domains::{Domain, MountainCar},
    fa::{
        EnumerableStateActionFunction,
        linear::{basis::{Fourier, Projector}, optim::SGD, LFA}
    },
    logging,
    policies::{EnumerablePolicy, EpsilonGreedy, Greedy, Policy, Random},
    spaces::{
        BoundedSpace,
        Card,
        Dim,
        FiniteSpace,
        Interval,
        ProductSpace,
        Space,
        discrete::{
            // Interval,
            Ordinal,
        },
    },
};

use rsrl_domains::{
    Action,
    Observation,
    State,
    Transition,
};

use umpire::{
    game::{
        Game,
        PlayerNum,
        ai::RandomAI,
        city::CityID,
        combat::CombatCapable,
        map::terrain::Terrain,
        obs::ObsTracker,
        player::TurnTaker,
        test_support::game_two_cities_two_infantry_big,
        unit::{
            UnitID,
            UnitType,
        },
    },
    util::{
        Direction,
        Location,
    },
};

// pub enum Observation<S> {
//     Full(S),
//     Partial(S),
//     Terminal(S),
// }

// pub enum Card {
//     Finite(usize),
//     Infinite,
// }

// pub enum Dim {
//     Finite(usize),
//     Infinite,
// }

// /// Container class for data associated with a domain transition.
// #[derive(Clone, Copy, Debug)]
// pub struct Transition<S, A> {
//     /// State transitioned _from_, `s`.
//     pub from: Observation<S>,

//     /// Action taken to initiate the transition (control tasks).
//     pub action: A,

//     /// Reward obtained from the transition.
//     pub reward: f64,

//     /// State transitioned _to_, `s'`.
//     pub to: Observation<S>,
// }

// /// Trait for defining spaces with at least one finite bound.
// ///
// /// Note: If both `inf` and `sup` are well defined (i.e. are not None), then the interval is
// /// totally bounded and we have a compact space; this is true in `spaces` as bounds are treated as
// /// closed.
// pub trait BoundedSpace: Space where Self::Value: PartialOrd {
//     /// Returns the value of the dimension's infimum, if it exists.
//     fn inf(&self) -> Option<Self::Value>;

//     /// Returns the value of the dimension's supremum, if it exists.
//     fn sup(&self) -> Option<Self::Value>;

//     /// Returns true iff `val` lies within the dimension's bounds (closed).
//     fn contains(&self, val: Self::Value) -> bool;

//     /// Returns true iff `self` has a finite infimum.
//     fn is_left_bounded(&self) -> bool { self.inf().is_some() }

//     /// Returns true iff `self` has a finite supremum.
//     fn is_right_bounded(&self) -> bool { self.sup().is_some() }

//     /// Returns true iff `self` has finite bounds in both directions.
//     ///
//     /// Note: this trait assumed closedness, so compactness follows.
//     fn is_compact(&self) -> bool { self.is_left_bounded() && self.is_right_bounded() }
// }

// /// Trait for defining spaces containing a finite set of values.
// pub trait FiniteSpace: BoundedSpace where Self::Value: PartialOrd {
//     /// Returns the finite range of values contained by this space.
//     fn range(&self) -> ::std::ops::Range<Self::Value>;
// }

/// How important is a city in and of itself?
const CITY_INTRINSIC_SCORE: f64 = 40.0;
const VICTORY_SCORE: f64 = 999999.0;



// /// Represent the first player's game state as a vector
// fn game_to_vec(game: &Game) -> Vec<f64> {
//     // For every tile we add these f64's:
//     // is the tile observed or not?
//     // which player controls the tile (one hot encoded)
//     // is there a city or not?
//     // what is the unit type? (one hot encoded, could be none---all zeros)
//     // for each of the five potential carried units:
//     //   what is the unit type? (one hot encoded, could be none---all zeros)
//     // 
//     let mut x = Vec::new();

//     let observations = game.player_observations.get(&0).unwrap();

//     for obs in observations.iter() {
//         match obs {
//             Obs::Unobserved => {
//                 let n_zeros = 1// unobserved
//                     + game.num_players// which player controls the tile (nobody, one hot encoded)
//                     + 1//city or not
//                     + 6 * UnitType::values().len()// what is the unit type? (one hot encoded), for this unit and any
//                                                     // carried units. Could be none (all zeros)
//                 ;
//                 x.extend_from_slice(&vec![0.0; n_zeros]);
//             },
//             Obs::Observed{tile,..} => {

//                 x.push(1.0);// observed
//                 for p in 0..self.num_players {// which player controls the tile (one hot encoded)
//                     x.push(if let Some(Alignment::Belligerent{player}) = tile.alignment_maybe() {
//                         if player==p {
//                             1.0
//                         } else {
//                             0.0
//                         }
//                     } else {
//                         0.0
//                     });
//                 }

//                 x.push(if tile.city.is_some() { 1.0 } else { 0.0 });// city or not

//                 let mut units_unaccounted_for = 6;

//                 if let Some(ref unit) = tile.unit {
//                     units_unaccounted_for -= 1;
//                     for t in UnitType::values().iter() {
//                         x.push(if unit.type_ == *t { 1.0 } else { 0.0 });
//                     }

//                     for carried_unit in unit.carried_units() {
//                         units_unaccounted_for -= 1;
//                         for t in UnitType::values().iter() {
//                             x.push(if carried_unit.type_ == *t { 1.0 } else { 0.0 });
//                         }
//                     }
//                 }

//                 x.extend_from_slice(&vec![0.0; 6 * units_unaccounted_for]);// fill in zeros for any missing
//                                                                                     // units


//             }
//         }
//     }

//     x
// }




pub struct UmpireStateSpace {
    // num_tiles: usize,
    // num_players: PlayerNum,
    // unit_types: usize,
    space: ProductSpace<Interval<f64>>,
}

impl UmpireStateSpace {
    // fn ordinal_product_space_from_game_state(game: &Game) -> ProductSpace<Ordinal> {
    //     let players = game.num_players();
    //     let units = UnitType::values().len();

    //     let mut dims: Vec<Ordinal> = vec![
    //         // Is the tile observed or not? x 2
    //         Ordinal::new(2),
    //         // Interval::new(Some(0.0), Some(1.0)),

    //         // Is there no city (0), a neutral city (1) or the city of any of the players (2:2+players)? x (2 + players)
    //         Ordinal::new(2 + players),
    //         // Interval::new(Some(0.0), Some(1.0 + players)),

    //         // Is the city production n/a (0), none (1), or any of the unit types (2:2+units)? x (2 + units)
    //         Ordinal::new(2 + units),
    //         // Interval::new(Some(0.0), Some(1.0 + units)),

    //         // Is there no unit, or a unit of a particular type belong to a particular player? (1 + players*units)
    //         Ordinal::new(1 + players*units),
    //         // Interval::new(Some(0.0), Some(players*units)),

    //         // Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
    //         Ordinal::new(9),
    //         // Interval::new(Some(0.0), Some(8.0)),
    //     ];

    //     for _carrying_space_slot in 0..=5 {
    //         // Is there no unit, or a unit of any unit type with the same alignment as the carrier? (1 + units)
    //         dims.push(Ordinal::new(1 + units));
    //         // dims.push(Interval::new(Some(0.0), Some(units)));

    //         // Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
    //         dims.push(Ordinal::new(9));
    //         // dims.push(Interval::new(Some(0.0), Some(8.0)));
    //     }

    //     ProductSpace::new(dims)
    // }

    // fn interval_product_space_from_game_state(game: &Game) -> ProductSpace<Interval> {
    //     let players = game.num_players() as f64;
    //     let units = UnitType::values().len() as f64;

    //     let mut dims: Vec<Interval> = vec![
    //         // Is the tile observed or not? x 2
    //         // Ordinal::new(2),
    //         Interval::new(Some(0.0), Some(1.0)),

    //         // Is there no city (0), a neutral city (1) or the city of any of the players (2:2+players)? x (2 + players)
    //         // Ordinal::new(2 + players),
    //         Interval::new(Some(0.0), Some(1.0 + players)),

    //         // Is the city production n/a (0), none (1), or any of the unit types (2:2+units)? x (2 + units)
    //         // Ordinal::new(2 + units),
    //         Interval::new(Some(0.0), Some(1.0 + units)),

    //         // Is there no unit, or a unit of a particular type belong to a particular player? (1 + players*units)
    //         // Ordinal::new(1 + players*units),
    //         Interval::new(Some(0.0), Some(players*units)),

    //         // Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
    //         // Ordinal::new(9),
    //         Interval::new(Some(0.0), Some(8.0)),
    //     ];

    //     for _carrying_space_slot in 0..=5 {
    //         // Is there no unit, or a unit of any unit type with the same alignment as the carrier? (1 + units)
    //         // dims.push(Ordinal::new(1 + units));
    //         dims.push(Interval::new(Some(0.0), Some(units)));

    //         // Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
    //         // dims.push(Ordinal::new(9));
    //         dims.push(Interval::new(Some(0.0), Some(8.0)));
    //     }

    //     ProductSpace::new(dims)
    // }

    fn from_game_state(game: &Game) -> Self {
        // For every tile:
        // is the tile observed or not?
        // which player controls the tile (one hot encoded)
        // is there a city or not?
        // what is the unit type? (one hot encoded, could be none---all zeros)
        // for each of the five potential carried units:
        //   what is the unit type? (one hot encoded, could be none---all zeros)
        // 
        
        let players = game.num_players();
        let units = UnitType::values().len();

        let mut dims: Vec<Interval<f64>> = vec![Interval::new(Some(0.0), Some(1.0))];// is the tile observed or not?
        for _ in 0..players {
            dims.push(Interval::new(Some(0.0), Some(1.0)));// which player controls the tile (one hot encoded)
        }

        dims.push(Interval::new(Some(0.0), Some(1.0)));// is there a city or not?

        for _ in 0..units {// what is the unit type (one hot encoded, all zeros if no unit)
            dims.push(Interval::new(Some(0.0), Some(1.0)));
        }

        // for each of the five potential carried units:
        for _ in 0..5 {
            for _ in 0..units {// what is the unit type (one hot encoded, all zeros if no unit)
                dims.push(Interval::new(Some(0.0), Some(1.0)));
            }
        }


        // let mut dims: Vec<Interval<f64>> = vec![
        //     // Is the tile observed or not? x 2
        //     // Ordinal::new(2),
        //     Interval::new(Some(0.0), Some(1.0)),

        //     // Is there no city (0), a neutral city (1) or the city of any of the players (2:2+players)? x (2 + players)
        //     // Ordinal::new(2 + players),
        //     Interval::new(Some(0.0), Some(1.0 + players)),

        //     // Is the city production n/a (0), none (1), or any of the unit types (2:2+units)? x (2 + units)
        //     // Ordinal::new(2 + units),
        //     Interval::new(Some(0.0), Some(1.0 + units)),

        //     // Is there no unit, or a unit of a particular type belong to a particular player? (1 + players*units)
        //     // Ordinal::new(1 + players*units),
        //     Interval::new(Some(0.0), Some(players*units)),

        //     // Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
        //     // Ordinal::new(9),
        //     Interval::new(Some(0.0), Some(8.0)),
        // ];

        // for _carrying_space_slot in 0..=5 {
        //     // Is there no unit, or a unit of any unit type with the same alignment as the carrier? (1 + units)
        //     // dims.push(Ordinal::new(1 + units));
        //     dims.push(Interval::new(Some(0.0), Some(units)));

        //     // Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
        //     // dims.push(Ordinal::new(9));
        //     dims.push(Interval::new(Some(0.0), Some(8.0)));
        // }

        Self {
            space: ProductSpace::new(dims)
        }
    }
}

impl Space for UmpireStateSpace {
    type Value = Game;
    // type Value = ObsTracker;
    // type Value = Vec<f64>;
    

    fn dim(&self) -> Dim {
        // let per_tile_dim = 15;
        // Dim::Finite(self.num_tiles * per_tile_dim)
        self.space.dim()
    }

    fn card(&self) -> Card {
        // The cardinality of the game state space will be a function of the map
        self.space.card()

        // For each tile:
        //   Is the tile observed or not (2)
        //   Is there no city, a neutral city, or the city of any of the players? (2 + players)
        //   Is the city production n/a, none, or any of the unit types? (2 + units)
        //   Is there no unit, or a unit a particular type belonging to any of the players? (1 + players*units)
        //   Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
        //   For each of five carrying space slots, is there no unit, or a unit of a particular type matching the carrier's
        //     alignment? 5 * (1 + units)
        //   For each of five carrying space slots, the same unit hp calculation again 5 * (9)

        // let per_tile_cardinality = 2
        //     * (2 + self.num_players)
        //     * (2 + self.unit_types)
        //     * (1 + self.num_players*self.unit_types)
        //     * 9
        //     * 5 * (1 + self.unit_types)
        //     * 5 * 9;
        
        // Card::Finite(self.num_tiles * per_tile_cardinality)


        // // Every tile could contain every possible combination of player units, as well as being empty
        // // Every carrier unit could contain every possible combination of its owner's units
        // // Every city could have every possible combination of owners, and every possible production assignment (or
        // // none at all).

        // // Every unit and every city could have any combination of its hitpoints from 1..=max
        
        // // Non-essentials like the names of the units are ignored---they're there for decorative purposes but are not
        // // combinatorially relevant.

        // let mut cardinality = 0;

        // for (terrain,count) in self.terrain_counts.iter() {

        // }

        // Card::Finite(cardinality)
    }
}


// #[derive(Clone)]
// enum UmpireActionScenario {
//     UnitOrdersRequest {
//         unit_id: UnitID,
//     },
//     ProductionSetRequest {
//         city_id: CityID,
//     }
// }

#[derive(Clone,Copy,Eq,Ord,PartialEq,PartialOrd)]
enum UmpireAction {
    // SetProduction{city_loc: Location, unit_type: UnitType},
    // MoveUnit{unit_id: UnitID, direction: Direction}
    SetNextCityProduction{unit_type: UnitType},
    MoveNextUnit{direction: Direction},
}

impl UmpireAction {
    fn legal_actions(game: &Game) -> BinaryHeap<Self> {
        let mut a = BinaryHeap::new();

        //TODO Possibly consider actions for all cities instead of just the next one that isn't set yet
        if let Some(city_loc) = game.production_set_requests().next() {
            for unit_type in game.valid_productions(city_loc) {
                a.push(UmpireAction::SetNextCityProduction{unit_type});
            }
        }

        //TODO Possibly consider actions for all units instead of just the next one that needs orders
        if let Some(unit_id) = game.unit_orders_requests().next() {
            for direction in game.current_player_unit_legal_directions(unit_id).unwrap() {
                a.push(UmpireAction::MoveNextUnit{direction});
            }
        }

        a
    }

    /// The number of possible actions in the abstract
    fn possible_actions() -> usize {
        UnitType::values().len() + Direction::values().len()
    }

    fn to_idx(&self) -> usize {
        match self {
            UmpireAction::SetNextCityProduction{unit_type} => {
                let types = UnitType::values();
                for i in 0..types.len() {
                    if types[i] == *unit_type {
                        return i;
                    }
                }
                unreachable!()
            },
            UmpireAction::MoveNextUnit{direction} => {
                let dirs = Direction::values();
                for i in 0..dirs.len() {
                    if dirs[i] == *direction {
                        return UnitType::values().len() + i;
                    }
                }
                unreachable!()
            }
        }
    }

    fn from_idx(mut idx: usize) -> Result<Self,()> {
        let unit_types = UnitType::values();
        if unit_types.len() > idx {
            return Ok(UmpireAction::SetNextCityProduction{unit_type: unit_types[idx]});
        }

        idx -= unit_types.len();

        let dirs = Direction::values();
        if dirs.len() > idx {
            return Ok(UmpireAction::MoveNextUnit{direction: dirs[idx]});
        }

        Err(())
    }
}

struct UmpireActionSpace;// {
//     actions: Vec<UmpireAction>,
// }

// impl UmpireActionSpace {
//     fn from_game_state(game: &Game) -> Self {
//         Self {
//             actions: UmpireAction::legal_actions(game).into_sorted_vec()
//         }
//     }
// }

impl Space for UmpireActionSpace {
    // type Value = UmpireActionScenario;
    type Value = usize;

    fn dim(&self) -> Dim {
        Dim::one()
    }
    fn card(&self) -> Card {
        Card::Finite(UmpireAction::possible_actions())
    }
}

// impl BoundedSpace for UmpireActionSpace {
//     /// Returns the value of the dimension's infimum, if it exists.
//     fn inf(&self) -> Option<Self::Value> {
//         self.actions.get(0).cloned()
//     }

//     /// Returns the value of the dimension's supremum, if it exists.
//     fn sup(&self) -> Option<Self::Value> {
//         self.actions.get(self.actions.len() - 1).cloned()
//     }

//     /// Returns true iff `val` lies within the dimension's bounds (closed).
//     fn contains(&self, val: Self::Value) -> bool {
//         self.actions.contains(&val)
//     }
// }

// /// Trait for defining spaces containing a finite set of values.
// impl FiniteSpace for UmpireActionSpace {
//     /// Returns the finite range of values contained by this space.
//     fn range(&self) -> ::std::ops::Range<Self::Value>;
// }

/// The domain of the game of Umpire being played by player 0 against an AI opponent
struct UmpireDomain {
    /// The game state
    game: Game,

    /// Our formidable foe
    random_ai: RandomAI,
}

impl UmpireDomain {
//     fn new() -> Self {
//         let game = game_two_cities_two_infantry_big();

//         Self { game, random_ai: RandomAI::new() }
//     }


    fn update_state(&mut self, action: UmpireAction) {
        match action {
            UmpireAction::SetNextCityProduction{unit_type} => {
                let city_loc = self.game.production_set_requests().next().unwrap();
                self.game.set_production_by_loc(city_loc, unit_type).unwrap();
            },
            UmpireAction::MoveNextUnit{direction} => {
                let unit_id = self.game.unit_orders_requests().next().unwrap();
                self.game.move_unit_by_id_in_direction(unit_id, direction).unwrap();
            },
        }

        if self.game.turn_is_done() {
            self.game.end_turn().unwrap();

            let mut ctrl = self.game.player_turn_control(1);
            self.random_ai.take_turn(&mut ctrl);
        }
    }

    fn current_player_score(&self) -> f64 {
        let mut score = 0.0;

        for unit in self.game.current_player_units() {
            // The cost of the unit scaled by the unit's current hitpoints relative to maximum
            score += (unit.type_.cost() as f64) * (unit.hp() as f64) / (unit.max_hp() as f64);
        }

        for city in self.game.current_player_cities() {
            // The city's intrinsic value plus any progress it's made toward producing its unit
            score += CITY_INTRINSIC_SCORE + city.production_progress as f64;
        }

        if let Some(victor) = self.game.victor() {
            if victor == self.game.current_player() {
                score += VICTORY_SCORE;
            }
        }

        score
    }
}

impl Default for UmpireDomain {
    fn default() -> Self {
        let game = game_two_cities_two_infantry_big();

        Self { game, random_ai: RandomAI::new() }
    }
}

impl Domain for UmpireDomain {
    /// State space representation type class.
    type StateSpace = UmpireStateSpace;
    // type StateSpace = ProductSpace<Interval>;

    /// Action space representation type class.
    type ActionSpace = UmpireActionSpace;

    /// Emit an observation of the current state of the environment.
    fn emit(&self) -> Observation<State<Self>> {
        // let v = self.game.to_feature_vec();
        let v = self.game.clone();
        if self.game.victor().is_some() {
            Observation::Terminal(v)
        } else {
            // Partial unless we happen to be observing every tile in the current turn, which we'll assume doesn't happen
            Observation::Partial(v)
        }
    }

    /// Transition the environment forward a single step given an action, `a`.
    fn step(&mut self, action_idx: usize) -> Transition<State<Self>, Action<Self>> {
        let start_score = self.current_player_score();
        let from = self.emit();

        let action = UmpireAction::from_idx(action_idx).unwrap();

        self.update_state(action);

        let end_score = self.current_player_score();
        let to = self.emit();

        let reward = end_score - start_score;

        Transition {
            from,
            action: action_idx,
            reward,
            to,
        }
    }

    /// Returns an instance of the state space type class.
    fn state_space(&self) -> Self::StateSpace {
        UmpireStateSpace::from_game_state(&self.game)
        // UmpireStateSpace::interval_product_space_from_game_state(&self.game)
    }

    /// Returns an instance of the action space type class.
    fn action_space(&self) -> Self::ActionSpace {
        // UmpireActionSpace::from_game_state(&self.game)
        UmpireActionSpace
    }
}


// use crate::policies::{EnumerablePolicy, Policy};
use rand::{distributions::{Distribution, Uniform}, Rng};

// #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
// #[derive(Clone, Debug)]
pub struct UmpireRandom;

// impl Random {
//     pub fn new(n_actions: usize) -> Self { Random(n_actions) }
// }

// impl RandomActionPolicy {
//     fn permitted_actions(state: &Game) -> Vec<UmpireAction> {

//     }
// }

impl Policy<Game> for UmpireRandom {
    type Action = usize;

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R, state: &Game) -> usize {
        // Uniform::new(0, self.0).sample(rng)
        // let mut rng = rand::thread_rng();
        let legal = UmpireAction::legal_actions(state).into_vec();
        // legal.choose(rng).cloned().unwrap()
        let indices: Vec<usize> = (0..legal.len()).collect();
        indices.choose(rng).cloned().unwrap()


    }

    fn probability(&self, state: &Game, action: &Self::Action) -> f64 {
        let legal_indices: Vec<usize> = UmpireAction::legal_actions(state).iter().map(|action| action.to_idx()).collect();
        if legal_indices.contains(action) {
            1.0 / legal_indices.len() as f64
        } else {
            0.0
        }

        // let n_legal = UmpireAction::legal_actions(state).len();
        // 1.0 / n_legal as f64
    }
}

// impl<S> EnumerablePolicy<S> for RandomActionPolicy {
//     fn n_actions(&self) -> usize {
//         UmpireAction::legal_actions()
//     }

//     fn probabilities(&self, _: &S) -> Vec<f64> { vec![1.0 / self.0 as f64; self.0].into() }
// }

// struct UmpireEpsilonGreedy {

// }

// // #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
// // #[derive(Clone, Debug)]
// struct UmpireGreedy<Q>(Q);

// impl<Q> UmpireGreedy<Q> {
//     pub fn new(q_func: Q) -> Self { Self(q_func) }

//     pub fn argmax_qs(qs: &[f64]) -> usize {
//         // argmaxima(qs).1[0]

//     }
// }

// impl<S, Q: EnumerableStateActionFunction<S>> Policy<S> for UmpireGreedy<Q> {
//     type Action = usize;

//     fn mpa(&self, s: &S) -> Self::Action {

//         let qs = self.0.evaluate_all(s);

//         // Self::<Q>::argmax_qs(&self.0.evaluate_all(s))
//     }

//     fn probability(&self, s: &S, a: &Self::Action) -> f64 { self.probabilities(s)[*a] }
// }

// impl<S, Q: EnumerableStateActionFunction<S>> EnumerablePolicy<S> for UmpireGreedy<Q> {
//     fn n_actions(&self) -> usize { self.0.n_actions() }

//     fn probabilities(&self, s: &S) -> Vec<f64> {
//         let qs = self.0.evaluate_all(s);
//         let mut ps = vec![0.0; qs.len()];

//         let (_, maxima) = argmaxima(&qs);

//         let p = 1.0 / maxima.len() as f64;
//         for i in maxima {
//             ps[i] = p;
//         }

//         ps.into()
//     }
// }






struct UmpireEpsilonGreedy<Q> {
    greedy: Greedy<Q>,
    random: UmpireRandom,

    pub epsilon: f64,
}

impl<Q> UmpireEpsilonGreedy<Q> {
    pub fn new(greedy: Greedy<Q>, random: UmpireRandom, epsilon: f64) -> Self {
        Self {
            greedy,
            random,

            epsilon,
        }
    }

    #[allow(non_snake_case)]
    pub fn from_Q<S>(q_func: Q, epsilon: f64) -> Self where Q: EnumerableStateActionFunction<S> {
        let greedy = Greedy::new(q_func);
        // let random = Random::new(greedy.n_actions());
        let random = UmpireRandom;

        Self::new(greedy, random, epsilon)
    }
}

impl<Q: EnumerableStateActionFunction<Game>> Policy<Game> for UmpireEpsilonGreedy<Q> {
    type Action = usize;

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R, s: &Game) -> Self::Action {
        if rng.gen_bool(self.epsilon) {
            self.random.sample(rng, s)
        } else {
            self.greedy.sample(rng, s)
        }
    }

    fn mpa(&self, s: &Game) -> Self::Action { self.greedy.mpa(s) }

    fn probability(&self, s: &Game, a: &Self::Action) -> f64 { self.probabilities(s)[*a] }
}

impl<Q: EnumerableStateActionFunction<Game>> EnumerablePolicy<Game> for UmpireEpsilonGreedy<Q> {
    fn n_actions(&self) -> usize { self.greedy.n_actions() }

    fn probabilities(&self, s: &Game) -> Vec<f64> {
        let prs = self.greedy.probabilities(s);
        let pr = self.epsilon / prs.len() as f64;

        prs.into_iter().map(|p| pr + p * (1.0 - self.epsilon)).collect()
    }
}



fn main() {
    // let domain = MountainCar::default();
    let domain = UmpireDomain::default();

    let mut agent = {
        let n_actions: usize = domain.action_space().card().into();

        println!("# actions: {}", n_actions);
        // lfa::basis::stack::Stacker<lfa::basis::fourier::Fourier, lfa::basis::constant::Constant>
        // let basis = Fourier::from_space(5, domain.state_space()).with_constant();
        let basis = Fourier::from_space(1, domain.state_space().space).with_constant();
        let lfa = LFA::vector(basis, SGD(1.0), n_actions);
        let q_func = make_shared(lfa);

        // let policy = EpsilonGreedy::new(
        //     Greedy::new(q_func.clone()),
        //     Random::new(n_actions),
        //     0.2
        // );

        let policy = UmpireEpsilonGreedy::new(
            Greedy::new(q_func.clone()),
            UmpireRandom,
            0.2
        );

        QLearning::new(q_func, policy, 0.01, 1.0)
    };

    let logger = logging::root(logging::stdout());

    //FIXME Construct UmpireDomain instead of MountainCar
    // let domain_builder = Box::new(MountainCar::default);
    let domain_builder = Box::new(UmpireDomain::default);

    // Training phase:
    let _training_result = {
        // Start a serial learning experiment up to 1000 steps per episode.
        let e = SerialExperiment::new(&mut agent, domain_builder.clone(), 1000);

        // Realise 1000 episodes of the experiment generator.
        run(e, 1000, Some(logger.clone()))
    };

    // Testing phase:
    let testing_result = Evaluation::new(&mut agent, domain_builder).next().unwrap();

    println!("solution: {:?}", testing_result);
}
