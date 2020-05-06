//! Reinforcement learning-based AI

use std::{
    borrow::Borrow,
    cell::RefCell,
    collections::{
        HashMap,
        HashSet,
    },
    convert::TryFrom,
    fmt,
    fs::File,
    io::{
        stdout,
        Write,
    },
    rc::Rc,
    sync::Arc,
    path::Path,
};

use crossterm::{
    execute,
    cursor::MoveTo,
};

use serde::{Deserialize,Deserializer,Serialize, Serializer};

use rsrl::{
    OnlineLearner,
    SerialExperiment,
    Shared,
    run,
    make_shared,
    control::{
        Controller,
        td::QLearning,
    },
    domains::Domain,
    fa::{
        EnumerableStateActionFunction,
        linear::{
            basis::{
                Constant,
                // Polynomial,
            },
            optim::SGD,
            LFA,
            VectorFunction,
        },
    },
    logging,
    policies::{EnumerablePolicy, Policy},
    spaces::{
        Card,
        Dim,
        Interval,
        ProductSpace,
        Space,
    },
};

use crate::{
    game::{
        Game,
        ai::AISpec,
        player::{
            TurnTaker,
        },
        unit::{
            UnitType,
        },
    },
    name::IntNamer,
    util::{
        Dims,
        Direction,
        Wrap2d,
    },
};

use rand::{
    Rng,
    seq::SliceRandom,
    thread_rng,
};

use rsrl_domains::{
    Action,
    Observation,
    State,
    Transition,
};


use super::{dnn::DNN, RandomAI, Loadable, Storable, AI};

pub type Basis = Constant;
// pub type Basis = Polynomial;

pub type LFA_ = LFA<Basis,SGD,VectorFunction>;
type FA = LFA_;
// type FA = DNN;
type Agent = UmpireAgent<Shared<Shared<AI>>,UmpireEpsilonGreedy<Shared<AI>>>;

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

    pub fn to_idx(self) -> usize {
        Self::possible_actions().into_iter().position(|a| self == a).unwrap()
    }

    pub fn take(self, game: &mut Game) {
        match self {
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



pub struct UmpireActionSpace {
    legal_actions: HashSet<UmpireAction>,
}

impl UmpireActionSpace {
    fn from_game_state(game: &Game) -> Self {
        Self {
            legal_actions: UmpireAction::legal_actions(game)
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
pub struct UmpireDomain {
    /// The game state
    game: Game,

    /// Our formidable foes
    opponents: Vec<Rc<RefCell<AI>>>,

    verbosity: usize,

    fix_output_loc: bool,
}

impl UmpireDomain {
    // fn new(game: Game, opponent: Rc<RefCell<dyn TurnTaker>>, verbose: bool) -> Self {
    //     Self { game, opponent, verbose }
    // }

    fn new_from_path(map_dims: Dims, opponents: Vec<Rc<RefCell<AI>>>, fix_output_loc: bool, fog_of_war: bool, verbosity: usize) -> Self {
        let city_namer = IntNamer::new("city");
    
        let game = Game::new(
            map_dims,
            city_namer,
            2,
            fog_of_war,
            None,
            Wrap2d::BOTH,
        );

        // let opponent = instantiate_opponent(ai_model_path, fix_output_loc, verbosity)?;

        Self {
            game,
            opponents,
            fix_output_loc,
            verbosity,
        }
    }

    #[cfg(test)]
    fn from_game(game: Game, opponents: Vec<Rc<RefCell<AI>>>, fix_output_loc: bool, verbosity: usize) -> Self {
        // let opponent = instantiate_opponent(ai_model_path, fix_output_loc, verbosity)?;
        Self {
            game,
            opponents,
            fix_output_loc,
            verbosity,
        }
    }

    fn update_state(&mut self, action: UmpireAction) {

        debug_assert_eq!(self.game.current_player(), 0);
        debug_assert!(!self.game.turn_is_done());

        action.take(&mut self.game);

        if self.verbosity > 1 {
            if self.fix_output_loc {
                execute!(stdout(), MoveTo(0,0)).unwrap();
            }

            println!("{:?}", action);
            let loc = if let Some(unit_id) = self.game.unit_orders_requests().next() {
                self.game.current_player_unit_loc(unit_id)
            } else {
                self.game.production_set_requests().next()
            };

            println!("AI:\n{:?}", self.game.current_player_observations());
            if let Some(loc) = loc {
                println!("Loc: {:?}", loc);
            }
            
            println!("AI Cities: {}", self.game.current_player_cities().count());
            println!("AI Units: {}", self.game.current_player_units().count());
            println!("AI Score: {}", self.current_player_score());
        }

        // If the user's turn is done, end it and take a complete turn for the other player until there's something
        // for this user to do or the game is over
        while self.game.victor().is_none() && self.game.turn_is_done() {
            // End this user's turn
            self.game.end_turn_clearing().unwrap();

            // Play the other player's turn to completion
            // let mut ctrl = self.game.player_turn_control_clearing(1);
            // while ctrl.victor().is_none() && !ctrl.turn_is_done() {
            //     // LimitedTurnTaker::take_turn(&mut self.random_ai, &mut ctrl);
            //     TurnTaker::take_turn_clearing(self.opponent.as_mut(), &mut ctrl);
            // }

            for opponent in &mut self.opponents {
                (*opponent).borrow_mut().take_turn_clearing(&mut self.game);
            }

            // TurnTaker::take_turn_clearing(self.opponent.borrow_mut(), &mut self.game);

            debug_assert_eq!(self.game.current_player(), 0);

            // while self.game.victor().is_none() && !self.game.turn_is_done() {
            //     TurnTaker::take_turn_clearing(self.opponent.as_mut(), &mut self.game);
            // }
        }

        

        

        // // Run AI turns until the human player has something to do
        // while self.game.victor().is_none() && self.game.turn_is_done() {

        //     // Clear productions for cities that complete units so the AI can update---keeps it from getting stuck
        //     // in a state where it could never win the game (e.g. producing only fighters)
        //     let result = self.game.end_turn().unwrap().unwrap();

        //     for prod in result.production_outcomes {
        //         if let UnitProductionOutcome::UnitProduced{city, ..} = prod {
        //             self.game.clear_production_without_ignoring(city.loc).unrwap();
        //         }
        //     }

        //     let mut ctrl = self.game.player_turn_control(1);
        //     LimitedTurnTaker::take_turn(&mut self.random_ai, &mut ctrl);

        //     if ctrl.turn_is_done() {
        //         let result = ctrl.end_turn().unwrap().unwrap();

        //         for prod in result.production_outcomes {
        //             if let UnitProductionOutcome::UnitProduced{city, ..} = prod {
        //                 productions_to_clear.push(city.loc);
        //             }
        //         }
        //     }
        //     // Turn gets ended when ctrl goes out of scope
        // }
    }

    /// For our purposes, the player's score is their own inherent score minus all other players' scores.
    fn current_player_score(&self) -> f64 {
        let scores = self.game.player_scores();
        scores[self.game.current_player()]

        // let mut score = 0.0;

        // for player in 0..self.game.num_players() {
        //     if player == self.game.current_player() {
        //         score += scores[player];
        //     } else {
        //         score -= scores[player];
        //     }
        // }

        // score
    }
}

impl fmt::Debug for UmpireDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.game.fmt(f)
    }
}

impl Domain for UmpireDomain {
    /// State space representation type class.
    type StateSpace = UmpireStateSpace;

    /// Action space representation type class.
    type ActionSpace = UmpireActionSpace;

    /// Emit an observation of the current state of the environment.
    fn emit(&self) -> Observation<State<Self>> {
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

        debug_assert_eq!(self.game.current_player(), 0);

        let start_score = self.current_player_score();
        let from = self.emit();

        let action = UmpireAction::from_idx(action_idx).unwrap();

        // println!("Action: {:?}", action);

        

        self.update_state(action);

        debug_assert_eq!(self.game.current_player(), 0);

        let end_score = self.current_player_score();
        let to = self.emit();

        let reward = end_score - start_score;

        if self.verbosity > 1 {
            println!("AI Reward: {}", reward);
        }

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
    }

    /// Returns an instance of the action space type class.
    fn action_space(&self) -> Self::ActionSpace {
        UmpireActionSpace::from_game_state(&self.game)
    }
}



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
        let legal = UmpireAction::legal_actions(state);

        debug_assert!(!legal.is_empty());

        legal.iter().map(|action| {
            self.possible_indices.get(action).cloned().unwrap()
        }).collect()
    }
}

impl Policy<Game> for UmpireRandom {
    type Action = usize;
    

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R, state: &Game) -> usize {
        debug_assert!(!state.turn_is_done(), "It makes no sense to sample actions for a game whose current turn is
                                              already done");

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

pub struct UmpireGreedy<Q>(Q);

impl<Q> UmpireGreedy<Q> {
    pub fn new(q_func: Q) -> Self { Self(q_func) }

    pub fn legal_argmax_qs(qs: &[f64], state: &Game) -> usize {
        debug_assert!(!state.turn_is_done(), "It makes no sense to sample actions for a game whose current turn is
                                              already done");

        debug_assert!(!qs.is_empty());

        let legal = UmpireRandom::new().canonical_legal_indices(state);

        debug_assert!(!legal.is_empty());

        let argmaxima = legal_argmaxima(qs, &legal).1;

        // debug_assert!(!argmaxima.is_empty());
        if argmaxima.is_empty() {
            println!("No argmaximum in qs {:?} legal {:?}; choosing randomly", qs, legal);
            let mut rand = thread_rng();
            *legal.choose(&mut rand).unwrap()
        } else {
            argmaxima[0]
        }
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

        ps
    }
}

pub struct UmpireEpsilonGreedy<Q> {
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
            // println!("RANDOM");
            self.random.sample(rng, state)
        } else {
            // println!("GREEDY");
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

/// A Q-Learning agent for the game of Umpire
///
/// Basically a wrapper around `QLearning` which only selects among actions that are legal given the current game state
///
/// # Type Parameters
/// * Q: the q-function approximator
/// * P: the learning policy
pub struct UmpireAgent<Q,P> {
    pub q: QLearning<Q,P>,
    avoid_skip: bool,
}

impl<Q, P> OnlineLearner<Game, P::Action> for UmpireAgent<Q, P>
where
    Q: EnumerableStateActionFunction<Game>,
    P: EnumerablePolicy<Game>,
{
    fn handle_transition(&mut self, t: &Transition<Game, P::Action>) {
        self.q.handle_transition(t)
    }
}

impl<Q, P> Controller<Game, P::Action> for UmpireAgent<Q, P>
where
    Q: EnumerableStateActionFunction<Game>,
    P: EnumerablePolicy<Game>,
{
    fn sample_target(&self, _: &mut impl Rng, s: &Game) -> P::Action {
        find_legal_max(&self.q.q_func, s, self.avoid_skip).0
    }

    fn sample_behaviour(&self, rng: &mut impl Rng, s: &Game) -> P::Action {
        self.q.sample_behaviour(rng, s)
    }
}

// impl From<RL_AI> for UmpireAgent<Shared<Shared<FunctionApproximator>>,UmpireEpsilonGreedy<Shared<FunctionApproximator>>> {
//     fn from(rl_ai: RL_AI) -> Self {

//         let q_func: Shared<FunctionApproximator> = make_shared(rl_ai.q_func);
    
//         let policy: UmpireEpsilonGreedy<Shared<FunctionApproximator>> = UmpireEpsilonGreedy::new(
//             UmpireGreedy::new(q_func.clone()),
//             UmpireRandom::new(),
//             0.2
//         );

//         let q: QLearning<Shared<Shared<FunctionApproximator>>,UmpireEpsilonGreedy<Shared<FunctionApproximator>>> = QLearning::new(q_func, policy, 0.01, 0.8);
    
//         Self{q, avoid_skip: rl_ai.avoid_skip}
//     }
// }

// /// The lightweight, serializable counterpart to `UmpireAgent`. 
// #[allow(non_camel_case_types)]
// pub struct RL_AI {
//     q_func: FunctionApproximator,
//     avoid_skip: bool,
// }
// impl RL_AI {
//     pub fn new(q_func: FunctionApproximator, avoid_skip: bool) -> Self {
//         Self { q_func, avoid_skip }
//     }

//     fn _take_turn_unended(&mut self, game: &mut Game) {
//         while !game.turn_is_done() {
//             // let q_func: &Q = *self.q_func.borrow();


//             // let refcell: &RefCell<Q> = self.q_func.borrow();
//             // let ref_ = refcell.borrow();
//             // let q_func = ref_.borrow();
//             // let q_func: &Q = <RefCell<Q> as Borrow<Q>>::borrow(refcell);
//             let action_idx = find_legal_max(&self.q_func, game, self.avoid_skip).0;
//             let action = UmpireAction::from_idx(action_idx).unwrap();
//             action.take(game);
//         }
//     }

//     pub fn to_agent(self) -> UmpireAgent<Shared<Shared<FunctionApproximator>>,UmpireEpsilonGreedy<Shared<FunctionApproximator>>> {
//         let agent: UmpireAgent<Shared<Shared<FunctionApproximator>>,UmpireEpsilonGreedy<Shared<FunctionApproximator>>> = self.into();
//         agent
//     }
// }
// impl TurnTaker for RL_AI {
//     fn take_turn_not_clearing(&mut self, game: &mut Game) {
//         self._take_turn_unended(game);

//         game.end_turn().unwrap();
//     }

//     fn take_turn_clearing(&mut self, game: &mut Game) {
//         self._take_turn_unended(game);

//         game.end_turn_clearing().unwrap();
//     }
// }

// impl Loadable for RL_AI {
//     fn load(path: &Path) ->  Result<Self,String> {
//         FunctionApproximator::load(path).map(|q_func| Self {
//             q_func: q_func,
//             avoid_skip: true
//         })//NOTE assumes avoid_skip
//     }
// }

// impl Storable for RL_AI {
//     fn store(self, path: &Path) -> Result<(),String> {
//         self.q_func.store(path)
//         // let refcell = Rc::try_unwrap(self.q_func)
//         //                 .map_err(|_| String::from("Could not unwrap q_func from Rc"))?;
//         // let q = refcell.into_inner();
//         // q.store(path)
//     }
// }

// impl <'de, Q:Deserialize<'de>> Deserialize<'de> for RL_AI<Q> {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
//         Q::deserialize(deserializer)
//         .map(|q| Self {
//             q_func: Rc::new(RefCell::new(q)),
//             avoid_skip: true,
//         })
//     }
// }

// impl <Q:Serialize> Serialize for RL_AI<Q> {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
//         let refcell: &RefCell<Q> = self.q_func.borrow();
//         let q = refcell.borrow();
//         // let q: &RefCell<Q> = <Rc<RefCell<Q>> as Borrow<Q>>::borrow(&self.q_func);
//         // let q: &Q = self.q_func.borrow();

//         q.serialize(serializer)
//     }
// }

fn agent(deep: bool, avoid_skip: bool) -> Result<Agent,String> {

    let n_actions = UmpireAction::possible_actions().len();
    
    // lfa::basis::stack::Stacker<lfa::basis::fourier::Fourier, lfa::basis::constant::Constant>
    // let basis = Fourier::from_space(5, domain.state_space()).with_constant();


    // let fa = DNN::load(Path::new("ai/umpire_regressor")).unwrap();
    // let fa_ai = AI::DNN(fa);
    // let q_func = make_shared(fa_ai);

    let fa_ai = if deep {
        let fa = DNN::load(Path::new("ai/umpire_regressor"))?;
        AI::DNN(fa)
    } else {
        // let basis = Fourier::from_space(2, domain_builder().state_space().space).with_constant();
        let basis = Constant::new(5.0);
        // let basis = Polynomial::new(2, 1);
        let fa = LFA::vector(basis, SGD(0.001), n_actions);
        AI::LFA(fa)
    };

    let q_func = make_shared(fa_ai);


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

    Ok(UmpireAgent{q:QLearning::new(q_func, policy, 0.01, 0.8), avoid_skip})
}

pub fn trained_agent(
    deep: bool,
    opponent_specs: Vec<AISpec>,
    dims: Vec<Dims>,
    episodes: usize,
    steps: u64,
    avoid_skip: bool,
    fix_output_loc: bool,
    fog_of_war: bool,
    verbosity: usize,
) -> Result<Agent,String> {

    if opponent_specs.len() > 3 {
        return Err(format!("Cannot train agent against {} opponents; max is 3", opponent_specs.len()));
    }

    let mut agent = agent(deep, avoid_skip)?;

    for dims in dims {

        // let opponent_model_path = opponent_model_path.clone();

        if verbosity > 0 {
            println!("Training {}", dims);
        }

        // let opponent_ai = Rc::new(RefCell::new(AI::from(opponent_spec.clone())));
        let opponents: Vec<Rc<RefCell<AI>>> = opponent_specs.iter()
                                                            .cloned()
                                                            .map(AI::from)
                                                            .map(RefCell::new)
                                                            .map(Rc::new)
                                                            .collect();

        let domain_builder = Box::new(move || {
            UmpireDomain::new_from_path(dims, opponents.clone(), fix_output_loc, fog_of_war, verbosity)
        });

        // Start a serial learning experiment up to 1000 steps per episode.
        let e = SerialExperiment::new(&mut agent, domain_builder, steps);

        // Realise 1000 episodes of the experiment generator.
        run(e, episodes,
            if verbosity > 0 {
                let logger = logging::root(logging::stdout());
                Some(logger)
            } else {
                None
            }
        );
    }

    Ok(agent)
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



fn legal_argmaxima(vals: &[f64], legal_indices: &[usize]) -> (f64, Vec<usize>) {
    debug_assert!(!vals.is_empty());
    debug_assert!(!legal_indices.is_empty());

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

    // debug_assert!(!ixs.is_empty(), "Found no legal argmaxima. vals: {:?}, legal_indices: {:?}, max: {}, ixs: {:?}", vals, legal_indices, max, ixs);

    (max, ixs)
}

// pub fn instantiate_opponent(ai_model_path: Option<&String>, fix_output_loc: bool, verbosity: usize) -> Result<Rc<RefCell<dyn TurnTaker>>,String> {
//     let ai_type = AIType::try_from(ai_model_path)?;
//     // let player_type: PlayerType = ai_type.into();
//     let opponent: Rc<RefCell<dyn TurnTaker>> = ai_type.into();

//     //FIXME somehow set these verbosity settings


//     Ok(opponent)
// }


// pub fn instantiate_opponent(ai_model_path: Option<&String>, verbosity: usize) -> Rc<RefCell<dyn TurnTaker>> {
//     let opponent: Rc<RefCell<dyn TurnTaker>> = if let Some(ai_model_path) = ai_model_path {

//         let path = Path::new(ai_model_path.as_str());

//         if path.is_dir() {
//             // A TensorFlow model
//             let rl_dnn_ai: DNN = DNN::load(path).unwrap();
//             Rc::new(RefCell::new(rl_dnn_ai))
//         } else {
//             let f = File::open(path).unwrap();
//             let rl_ai: RL_AI<LFA_> = bincode::deserialize_from(f).unwrap();
//             Rc::new(RefCell::new(rl_ai))
//         }
//     } else {
//         Rc::new(RefCell::new(RandomAI::new(verbosity)))
//     };
//     opponent
// }

// pub fn instantiate_no_default(ai_model_path: &Path) -> RL_AI<Box<dyn EnumerableStateActionFunction<Game>>> {
//     if ai_model_path.is_dir() {
//         // A TensorFlow model
//         let dnn = DNN::load(ai_model_path).unwrap();
//         let rl_ai = RL_AI::new(Box::new(dnn), true);//NOTE assuming avoid_skip
//         rl_ai
//     } else {
//         let f = File::open(ai_model_path).unwrap();
//         let rl_ai: RL_AI<LFA_> = bincode::deserialize_from(f).unwrap();
//         rl_ai
//     }
// }



// /// Instantiate an RL AI from a path
// /// 
// /// If a file is given, deserialize it using serde's bincode
// /// 
// /// If a directory is given, load it as a TensorFlow SavedModel
// pub fn instantiate_opponent_no_default<F:Loadable>(ai_model_path: &Path) -> Result<RL_AI<F>,Box<dyn std::error::Error>> {
//     F::load(ai_model_path).map(|)
//     // if ai_model_path.is_dir() {
//     //     // A TensorFlow model
//     //     let dnn = DNN::load(ai_model_path).unwrap();
//     //     let rl_ai: RL_AI<F> = RL_AI::new(dnn, true);//NOTE assuming avoid_skip
//     //     rl_ai
//     // } else {
//     //     let f = File::open(ai_model_path).unwrap();
//     //     let rl_ai: RL_AI<LFA_> = bincode::deserialize_from(f).unwrap();
//     //     rl_ai
//     // }
// }

#[cfg(test)]
mod test {
    use std::{
        cell::RefCell,
        collections::{
            HashMap,
            HashSet,
        },
        rc::Rc,
    };

    use rand::{
        thread_rng,
    };

    use rsrl::{
        control::Controller,
    };

    use rsrl_domains::Domain;

    use crate::{
        game::{
            Alignment,
            ai::{RandomAI, rl::{
                UmpireAction,
                UmpireDomain,
                trained_agent,
            }, AI, AISpec},
            map::{
                MapData,
                Terrain,
            },
            unit::UnitType, Game,
        },
        util::{
            Dims,
            Direction,
            Location,
            Wrap2d,
        },
    };

    #[test]
    fn test_ai_movement() {
        let n = 10_000;

        let opponent: Rc<RefCell<AI>> = Rc::new(RefCell::new(AI::random(0)));

        // let domain_builder = Box::new(move || UmpireDomain::new_from_path(Dims::new(10, 10), None, false));
        let agent = trained_agent(false, vec![AISpec::Random], vec![Dims::new(10,10)], 10, 50, false, false, true, 0).unwrap();


        let mut map = MapData::new(Dims::new(10, 10), |_| Terrain::Land);
        let _unit_id = map.new_unit(Location::new(5,5), UnitType::Infantry,
            Alignment::Belligerent{player:0}, "Aragorn").unwrap();
        

        let mut directions: HashSet<Direction> = Direction::values().iter().cloned().collect();

        let mut counts: HashMap<UmpireAction,usize> = HashMap::new();


        let game = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);
        
        let mut domain = UmpireDomain::from_game(game.clone(), vec![opponent.clone()], false, 0);

        let mut rng = thread_rng();
        for _ in 0..n {

            // Reinitialize when somebody wins
            if domain.game.victor().is_some() {
                domain = UmpireDomain::from_game(game.clone(), vec![opponent.clone()], false, 0);
            }

            let idx = agent.sample_behaviour(&mut rng, domain.emit().state());

            domain.step(idx);

            let action = UmpireAction::from_idx(idx).unwrap();

            println!("Action: {:?}", action);

            *counts.entry(action).or_insert(0) += 1;

            if let UmpireAction::MoveNextUnit{direction} = action {
                directions.remove(&direction);
            }
        }

        assert!(directions.is_empty(), "AI is failing to explore in these directions over {} steps: {}\nCounts: {}",
            n,
            directions.iter().map(|dir| format!("{:?} ", dir)).collect::<String>(),
            counts.iter().map(|(k,v)| format!("{:?}:{} ", k, v)).collect::<String>()
        );
    }
}
