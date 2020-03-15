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

use rsrl::{
    run, make_shared, Evaluation, SerialExperiment,
    control::td::QLearning,
    domains::{Domain, MountainCar},
    fa::linear::{basis::{Fourier, Projector}, optim::SGD, LFA},
    logging,
    policies::{EpsilonGreedy, Greedy, Random},
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



pub struct UmpireStateSpace {
    // num_tiles: usize,
    // num_players: PlayerNum,
    // unit_types: usize,
    space: ProductSpace<Interval<f64>>,
}

impl UmpireStateSpace {
    fn from_game_state(game: &Game) -> Self {
        let players = game.num_players() as f64;
        let units = UnitType::values().len() as f64;

        let mut dims: Vec<Interval<f64>> = vec![
            // Is the tile observed or not? x 2
            // Ordinal::new(2),
            Interval::new(Some(0.0), Some(1.0)),

            // Is there no city (0), a neutral city (1) or the city of any of the players (2:2+players)? x (2 + players)
            // Ordinal::new(2 + players),
            Interval::new(Some(0.0), Some(1.0 + players)),

            // Is the city production n/a (0), none (1), or any of the unit types (2:2+units)? x (2 + units)
            // Ordinal::new(2 + units),
            Interval::new(Some(0.0), Some(1.0 + units)),

            // Is there no unit, or a unit of a particular type belong to a particular player? (1 + players*units)
            // Ordinal::new(1 + players*units),
            Interval::new(Some(0.0), Some(players*units)),

            // Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
            // Ordinal::new(9),
            Interval::new(Some(0.0), Some(8.0)),
        ];

        for _carrying_space_slot in 0..=5 {
            // Is there no unit, or a unit of any unit type with the same alignment as the carrier? (1 + units)
            // dims.push(Ordinal::new(1 + units));
            dims.push(Interval::new(Some(0.0), Some(units)));

            // Are the unit's hitpoints n/a, or between 1 and max hp for all units? (1 + 8 = 9)
            // dims.push(Ordinal::new(9));
            dims.push(Interval::new(Some(0.0), Some(8.0)));
        }

        Self {
            space: ProductSpace::new(dims)
        }
    }
}

impl Space for UmpireStateSpace {
    type Value = Game;
    // type Value = ObsTracker;
    

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
}

struct UmpireActionSpace {
    actions: Vec<UmpireAction>,
}

impl UmpireActionSpace {
    fn from_game_state(game: &Game) -> Self {
        Self {
            actions: UmpireAction::legal_actions(game).into_sorted_vec()
        }
    }
}

impl Space for UmpireActionSpace {
    // type Value = UmpireActionScenario;
    type Value = UmpireAction;

    fn dim(&self) -> Dim {
        Dim::one()
    }
    fn card(&self) -> Card {
        Card::Finite(self.actions.len())
    }
}

impl BoundedSpace for UmpireActionSpace {
    /// Returns the value of the dimension's infimum, if it exists.
    fn inf(&self) -> Option<Self::Value> {
        self.actions.get(0).cloned()
    }

    /// Returns the value of the dimension's supremum, if it exists.
    fn sup(&self) -> Option<Self::Value> {
        self.actions.get(self.actions.len() - 1).cloned()
    }

    /// Returns true iff `val` lies within the dimension's bounds (closed).
    fn contains(&self, val: Self::Value) -> bool {
        self.actions.contains(&val)
    }
}

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

    /// Action space representation type class.
    type ActionSpace = UmpireActionSpace;

    /// Emit an observation of the current state of the environment.
    fn emit(&self) -> Observation<State<Self>> {
        if self.game.victor().is_some() {
            Observation::Terminal(self.game.clone())
        } else {
            // Partial unless we happen to be observing every tile in the current turn, which we'll assume doesn't happen
            Observation::Partial(self.game.clone())
        }
    }

    /// Transition the environment forward a single step given an action, `a`.
    fn step(&mut self, action: UmpireAction) -> Transition<State<Self>, Action<Self>> {
        let start_score = self.current_player_score();
        let from = self.emit();

        self.update_state(action);

        let end_score = self.current_player_score();
        let to = self.emit();

        let reward = end_score - start_score;

        Transition {
            from,
            action,
            reward,
            to,
        }
    }

    /// Returns an instance of the state space type class.
    fn state_space(&self) -> Self::StateSpace {
        UmpireStateSpace::from_game_state(&self.game)
    }

    /// Returns an instance of the action space type class.
    fn action_space(&self) -> Self::ActionSpace {
        UmpireActionSpace::from_game_state(&self.game)
    }
}

fn main() {
    // let domain = MountainCar::default();
    let domain = UmpireDomain::default();

    let mut agent = {
        let n_actions: usize = domain.action_space().card().into();

        // let basis = Fourier::from_space(5, domain.state_space()).with_constant();
        let basis = Fourier::from_space(1, domain.state_space().space).with_constant();
        let q_func = make_shared(LFA::vector(basis, SGD(1.0), n_actions));

        let policy = EpsilonGreedy::new(
            Greedy::new(q_func.clone()),
            Random::new(n_actions),
            0.2
        );

        QLearning::new(q_func, policy, 0.01, 1.0)
    };

    let logger = logging::root(logging::stdout());

    //FIXME Construct UmpireDomain instead of MountainCar
    let domain_builder = Box::new(MountainCar::default);
    // let domain_builder = Box::new(UmpireDomain::default);

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