//! Tool to train Umpire AI
//! 
//! Strategy:
//! First we bootstrap the AI by having it play against a random baseline.
//! Then we train it against itself.
//! These initial games should have small maps and only two players.
//! 
//! Once we have a simple AI, incorporate it into the UI.
use std::{
    collections::{
        BinaryHeap,
        HashMap,
    },
};

use rand::{
    Rng,
    seq::SliceRandom,
};

use rsrl::{
    run, make_shared, Evaluation, SerialExperiment,
    control::td::QLearning,
    domains::{Domain, MountainCar},
    fa::{
        EnumerableStateActionFunction,
        StateFunction,
        linear::{basis::{Constant, Fourier, Projector}, optim::SGD, LFA}
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
        ai::RandomAI,
        combat::CombatCapable,
        player::TurnTaker,
        test_support::game_two_cities_two_infantry_big,
        unit::{
            UnitType,
        },
    },
    util::{
        Direction,
    },
};


/// How important is a city in and of itself?
const CITY_INTRINSIC_SCORE: f64 = 40.0;
const VICTORY_SCORE: f64 = 999999.0;




pub struct UmpireStateSpace {
    space: ProductSpace<Interval<f64>>,
}

impl UmpireStateSpace {

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

        Self {
            space: ProductSpace::new(dims)
        }
    }
}

impl Space for UmpireStateSpace {
    type Value = Game;    

    fn dim(&self) -> Dim {
        self.space.dim()
    }

    fn card(&self) -> Card {
        self.space.card()
    }
}

#[derive(Clone,Copy,Eq,Hash,Ord,PartialEq,PartialOrd)]
pub enum UmpireAction {
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

    fn possible_actions() -> BinaryHeap<Self> {
        let mut a = BinaryHeap::new();
        for unit_type in UnitType::values().iter().cloned() {
            a.push(UmpireAction::SetNextCityProduction{unit_type});
        }
        for direction in Direction::values().iter().cloned() {
            a.push(UmpireAction::MoveNextUnit{direction});
        }
        a
    }

    // /// The number of possible actions in the abstract
    // fn n_possible_actions() -> usize {
    //     UnitType::values().len() + Direction::values().len()
    // }

    // fn to_idx(&self) -> usize {
    //     match self {
    //         UmpireAction::SetNextCityProduction{unit_type} => {
    //             let types = UnitType::values();
    //             for i in 0..types.len() {
    //                 if types[i] == *unit_type {
    //                     return i;
    //                 }
    //             }
    //             unreachable!()
    //         },
    //         UmpireAction::MoveNextUnit{direction} => {
    //             let dirs = Direction::values();
    //             for i in 0..dirs.len() {
    //                 if dirs[i] == *direction {
    //                     return UnitType::values().len() + i;
    //                 }
    //             }
    //             unreachable!()
    //         }
    //     }
    // }

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

struct UmpireActionSpace {
    legal_actions: Vec<UmpireAction>,
}

impl UmpireActionSpace {
    fn from_game_state(game: &Game) -> Self {
        Self {
            legal_actions: UmpireAction::legal_actions(game).into_sorted_vec()
        }
    }
}

impl Space for UmpireActionSpace {
    // type Value = UmpireActionScenario;
    type Value = usize;

    fn dim(&self) -> Dim {
        Dim::one()
    }
    fn card(&self) -> Card {
        // Card::Finite(UmpireAction::possible_actions())
        Card::Finite(self.legal_actions.len())
    }
}

/// The domain of the game of Umpire being played by player 0 against an AI opponent
struct UmpireDomain {
    /// The game state
    game: Game,

    /// Our formidable foe
    random_ai: RandomAI,
}

impl UmpireDomain {
    fn update_state(&mut self, action: UmpireAction) {

        debug_assert_eq!(self.game.current_player(), 0);
        debug_assert!(!self.game.turn_is_done());

        match action {
            UmpireAction::SetNextCityProduction{unit_type} => {
                //FIXME We can't assume the request is available since the system is still selecting invalid actions

                if self.game.production_set_requests().next().is_some() {
                    let city_loc = self.game.production_set_requests().next().unwrap();
                    self.game.set_production_by_loc(city_loc, unit_type).unwrap();
                }
            },
            UmpireAction::MoveNextUnit{direction} => {
                //FIXME We can't assume the request is available since the system is still selecting invalid actions

                if self.game.unit_orders_requests().next().is_some() {
                    let unit_id = self.game.unit_orders_requests().next().unwrap();
                    self.game.move_unit_by_id_in_direction(unit_id, direction).unwrap();
                }
            },
        }

        // Run AI turns until the human player has something to do
        while self.game.turn_is_done() {
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
        UmpireActionSpace::from_game_state(&self.game)
    }
}

// #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
// #[derive(Clone, Debug)]
pub struct UmpireRandom {
    possible_indices: HashMap<UmpireAction,usize>,
}

impl UmpireRandom {
    fn new() -> Self {
        Self {
            possible_indices: UmpireAction::possible_actions().iter().enumerate().map(|(i,action)| (*action,i)).collect()
        }
    }

    /// The indices of all legal actions for a given game state, given in a consistent manner regardless of which (if
    /// any) are actually present.
    fn canonical_legal_indices(&self, state: &Game) -> Vec<usize> {
        UmpireAction::legal_actions(state).iter().map(|action| {
            self.possible_indices.get(action).cloned().unwrap()
        }).collect()
    }
}

impl Policy<Game> for UmpireRandom {
    type Action = usize;
    

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R, state: &Game) -> usize {
        self.canonical_legal_indices(state).choose(rng).cloned().unwrap()
    }

    fn probability(&self, state: &Game, action: &Self::Action) -> f64 {
        let legal_indices = self.canonical_legal_indices(state);
        if legal_indices.contains(action) {
            1.0 / legal_indices.len() as f64
        } else {
            0.0
        }
    }
}

fn legal_argmaxima(vals: &[f64], legal_indices: &[usize]) -> (f64, Vec<usize>) {
    let mut max = std::f64::MIN;
    let mut ixs = vec![];

    for i in legal_indices.iter().cloned() {
        let v = vals[i];
        if (v - max).abs() < 1e-7 {
            ixs.push(i);
        } else if v > max {
            max = v;
            ixs.clear();
            ixs.push(i);
        }
    }

    (max, ixs)
}

// #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
// #[derive(Clone, Debug)]
pub struct UmpireGreedy<Q>(Q);

impl<Q> UmpireGreedy<Q> {
    pub fn new(q_func: Q) -> Self { Self(q_func) }

    pub fn legal_argmax_qs(qs: &[f64], state: &Game) -> usize {
        let legal = UmpireRandom::new().canonical_legal_indices(state);
        legal_argmaxima(qs, &legal).1[0]
    }
}

impl<Q: EnumerableStateActionFunction<Game>> Policy<Game> for UmpireGreedy<Q> {
    type Action = usize;

    fn mpa(&self, state: &Game) -> usize {
        Self::legal_argmax_qs(&self.0.evaluate_all(state), state)
    }

    fn probability(&self, s: &Game, a: &usize) -> f64 { self.probabilities(s)[*a] }
}

impl<Q: EnumerableStateActionFunction<Game>> EnumerablePolicy<Game> for UmpireGreedy<Q> {
    fn n_actions(&self) -> usize { self.0.n_actions() }

    fn probabilities(&self, state: &Game) -> Vec<f64> {
        let qs = self.0.evaluate_all(state);
        let mut ps = vec![0.0; qs.len()];

        let legal_indices = UmpireRandom::new().canonical_legal_indices(state);
        debug_assert!(!legal_indices.is_empty());

        let (_, legal_maxima) = legal_argmaxima(&qs, &legal_indices);

        let p = 1.0 / legal_maxima.len() as f64;
        for i in legal_maxima {
            ps[i] = p;
        }

        ps.into()
    }
}



struct UmpireEpsilonGreedy<Q> {
    greedy: UmpireGreedy<Q>,
    random: UmpireRandom,
    pub epsilon: f64,
}

impl<Q> UmpireEpsilonGreedy<Q> {
    pub fn new(greedy: UmpireGreedy<Q>, random: UmpireRandom, epsilon: f64) -> Self {
        Self {
            greedy,
            random,

            epsilon,
        }
    }
}

impl<Q: EnumerableStateActionFunction<Game>> Policy<Game> for UmpireEpsilonGreedy<Q> {
    type Action = usize;

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R, state: &Game) -> Self::Action {
        if rng.gen_bool(self.epsilon) {
            self.random.sample(rng, state)
        } else {
            self.greedy.sample(rng, state)
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


// Constant:
// #[derive(Clone, Debug, Serialize, Deserialize)]
struct UmpireConstant<V>(pub V);

impl<V: Clone> StateFunction<Game> for UmpireConstant<V> {
    type Output = V;

    fn evaluate(&self, _state: &Game) -> Self::Output {
         self.0.clone()
    }

    fn update(&mut self, _: &Game, _: Self::Output) {}
}

fn get_bounds(d: &Interval) -> (f64, f64) {
    match (d.inf(), d.sup()) {
        (Some(lb), Some(ub)) => (lb, ub),
        (Some(_), None) => panic!("Dimension {} is missing an upper bound (sup).", d),
        (None, Some(_)) => panic!("Dimension {} is missing a lower bound (inf).", d),
        (None, None) => panic!("Dimension {} must be bounded.", d),
    }
}



fn main() {
    println!("Training Umpire AI.");


    // let domain = MountainCar::default();
    let domain = UmpireDomain::default();

    let mut agent = {

        let n_actions = UmpireAction::possible_actions().len();
        // let n_actions: usize = domain.action_space().card().into();

        println!("# actions: {}", n_actions);
        // lfa::basis::stack::Stacker<lfa::basis::fourier::Fourier, lfa::basis::constant::Constant>
        // let basis = Fourier::from_space(5, domain.state_space()).with_constant();

        let limits: Vec<(f64,f64)> = domain.state_space().space.iter().map(get_bounds).collect();
        println!("Limits: {}", limits.len());

        // let basis = Fourier::from_space(order, domain.state_space().space).with_constant();
        let basis = Constant::new(5.0);
        let lfa = LFA::vector(basis, SGD(1.0), n_actions);
        let q_func = make_shared(lfa);

        // let policy = EpsilonGreedy::new(
        //     Greedy::new(q_func.clone()),
        //     Random::new(n_actions),
        //     0.2
        // );

        let policy = UmpireEpsilonGreedy::new(
            UmpireGreedy::new(q_func.clone()),
            UmpireRandom::new(),
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
