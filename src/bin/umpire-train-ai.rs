//! Tool to train Umpire AI
//! 
//! Strategy:
//! First we bootstrap the AI by having it play against a random baseline.
//! Then we train it against itself.
//! These initial games should have small maps and only two players.
//! 
//! Once we have a simple AI, incorporate it into the UI.
#![forbid(unsafe_code)]
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    io::Write,
    rc::Rc,
    sync::{
        Arc,
        RwLock,
    }, path::Path, fs::File,
};

use clap::{Arg};

use rand::{
    Rng,
    seq::SliceRandom,
};

use rsrl::{
    Evaluation,
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
        StateFunction,
        linear::{
            basis::{
                Polynomial,
            },
            optim::SGD,
            LFA,
            VectorFunction,
        },
    },
    logging,
    policies::{EnumerablePolicy, Policy},
    spaces::{
        BoundedSpace,
        Card,
        Dim,
        Interval,
        ProductSpace,
        Space,
    },
};

use rsrl_domains::{
    Action,
    Observation,
    State,
    Transition,
};

use umpire::{
    cli,
    conf,
    game::{
        Game,
        ai::{
            RandomAI,
            rl::{
                UmpireAction, find_legal_max,
            }, RL_AI,
        },
        combat::CombatCapable,
        player::{LimitedTurnTaker},
        // test_support::{
        //     game_two_cities_two_infantry,
        //     game_two_cities_two_infantry_big,
        //     game_two_cities_two_infantry_dims,
        //     game_tunnel,
        // },
        unit::{
            UnitType,
        },
    },
    name::IntNamer,
    util::{
        Dims,
        Wrap2d,
    },
};


/// How important is a city in and of itself?
const CITY_INTRINSIC_SCORE: f64 = 1000.0;
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



struct UmpireActionSpace {
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
struct UmpireDomain {
    /// The game state
    game: Game,

    /// Our formidable foe
    random_ai: RandomAI,

    verbose: bool,
}

impl UmpireDomain {
    fn new(map_dims: Dims, verbose: bool) -> Self {
        let city_namer = IntNamer::new("city");
        let unit_namer = IntNamer::new("unit");
    
        let game = Game::new(
            map_dims,
            city_namer,
            2,
            false,
            Arc::new(RwLock::new(unit_namer)),
            Wrap2d::BOTH,
        );

        Self {
            // game: game_two_cities_two_infantry_dims(dims),
            // game: game_tunnel(dims),
            game,
            random_ai: RandomAI::new(verbose),
            verbose,
        }
    }

    #[cfg(test)]
    fn from_game(game: Game, verbose: bool) -> Self {
        Self {
            game,
            random_ai: RandomAI::new(verbose),
            verbose,
        }
    }

    fn update_state(&mut self, action: UmpireAction) {

        debug_assert_eq!(self.game.current_player(), 0);
        debug_assert!(!self.game.turn_is_done());

        action.take(&mut self.game);

        if self.verbose {
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

        // Run AI turns until the human player has something to do
        while self.game.victor().is_none() && self.game.turn_is_done() {
            self.game.end_turn().unwrap();

            let mut ctrl = self.game.player_turn_control(1);
            LimitedTurnTaker::take_turn(&mut self.random_ai, &mut ctrl);
            // self.random_ai.take_turn(&mut ctrl);
            // Turn gets ended when ctrl goes out of scope
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

        if self.verbose {
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

    debug_assert!(!ixs.is_empty(), "Found no legal argmaxima. vals: {:?}, legal_indices: {:?}, max: {}, ixs: {:?}", vals, legal_indices, max, ixs);

    (max, ixs)
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

        debug_assert!(!argmaxima.is_empty());

        argmaxima[0]
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

struct UmpireConstant<V>(pub V);

impl<V: Clone> StateFunction<Game> for UmpireConstant<V> {
    type Output = V;

    fn evaluate(&self, _state: &Game) -> Self::Output {
         self.0.clone()
    }

    fn update(&mut self, _: &Game, _: Self::Output) {}
}




struct UmpireAgent<Q,P> {
    q: QLearning<Q,P>,
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
        // self.q.q_func.find_max(s).0
        // self.find_legal_max(s).0
        find_legal_max(&self.q.q_func, s).0
    }

    fn sample_behaviour(&self, rng: &mut impl Rng, s: &Game) -> P::Action {
        self.q.sample_behaviour(rng, s)
    }
}



fn get_bounds(d: &Interval) -> (f64, f64) {
    match (d.inf(), d.sup()) {
        (Some(lb), Some(ub)) => (lb, ub),
        (Some(_), None) => panic!("Dimension {} is missing an upper bound (sup).", d),
        (None, Some(_)) => panic!("Dimension {} is missing a lower bound (inf).", d),
        (None, None) => panic!("Dimension {} must be bounded.", d),
    }
}

fn agent(domain_builder: &dyn Fn() -> UmpireDomain, verbose: bool) ->
        UmpireAgent<Shared<Shared<LFA<Polynomial,SGD,VectorFunction>>>,
            UmpireEpsilonGreedy<Shared<LFA<Polynomial, SGD, VectorFunction>>>>{

    let n_actions = UmpireAction::possible_actions().len();
    // let n_actions: usize = domain.action_space().card().into();

    if verbose {
        println!("# actions: {}", n_actions);

        let limits: Vec<(f64,f64)> = domain_builder().state_space().space.iter().map(get_bounds).collect();
        println!("Limits: {}", limits.len());
    }
    
    // lfa::basis::stack::Stacker<lfa::basis::fourier::Fourier, lfa::basis::constant::Constant>
    // let basis = Fourier::from_space(5, domain.state_space()).with_constant();

    // let basis = Fourier::from_space(2, domain_builder().state_space().space).with_constant();
    // let basis = Constant::new(5.0);
    let basis = Polynomial::new(1, 1);
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

    UmpireAgent{q:QLearning::new(q_func, policy, 0.01, 1.0)}
}

fn trained_agent(domain_builder: Box<dyn Fn() -> UmpireDomain>, episodes: usize, steps: u64, verbose: bool) ->
        UmpireAgent<Shared<Shared<LFA<Polynomial,SGD,VectorFunction>>>,
            UmpireEpsilonGreedy<Shared<LFA<Polynomial, SGD, VectorFunction>>>>{
                

    let logger = logging::root(logging::stdout());

    let mut agent = agent(&*domain_builder, verbose);


    // Start a serial learning experiment up to 1000 steps per episode.
    let e = SerialExperiment::new(&mut agent, domain_builder, steps);

    // Realise 1000 episodes of the experiment generator.
    run(e, episodes, Some(logger.clone()));

    agent
}



fn main() {
    let matches = cli::app("Umpire AI Trainer", "HWv")
    .version(conf::APP_VERSION)
    .author("Josh Hansen <hansen.joshuaa@gmail.com>")
    .arg(
        Arg::with_name("episodes")
        .short("e")
        .long("episodes")
        .takes_value(true)
        .default_value("1000")
        .validator(|s| {
            let episodes: Result<usize,_> = s.trim().parse();
            episodes.map(|_n| ()).map_err(|_e| format!("Invalid episodes '{}'", s))
        })
    )
    .arg(
        Arg::with_name("out")
        // .short("o")
        // .long("out")
        .help("Output path")
        // .takes_value(true)
        .required(true)
    )
    // .arg(
    //     Arg::with_name("rel_height")
    //     .short("h")
    //     .long("rel_height")
    //     .help("Height of relative feature window")
    //     .takes_value(true)
    //     .default_value("10")
    //     .validator(|s| {
    //         let width: Result<usize,_> = s.trim().parse();
    //         width.map(|_n| ()).map_err(|_e| format!("Invalid feature window height '{}'", s))
    //     })
    // )
    // .arg(
    //     Arg::with_name("rel_width")
    //     .short("w")
    //     .long("rel_width")
    //     .help("Width of relative feature window")
    //     .takes_value(true)
    //     .default_value("10")
    //     .validator(|s| {
    //         let width: Result<usize,_> = s.trim().parse();
    //         width.map(|_n| ()).map_err(|_e| format!("Invalid feature window width '{}'", s))
    //     })
    // )
    .arg(
        Arg::with_name("steps")
        .short("s")
        .long("steps")
        .takes_value(true)
        .default_value("5000")
        .validator(|s| {
            let steps: Result<u64,_> = s.trim().parse();
            steps.map(|_n| ()).map_err(|_e| format!("Invalid steps '{}'", s))
        })
    )
    .arg(
        Arg::with_name("evaluate")
        .short("E")
        .long("evaluate")
        .help("Evaluate the trained policy")
    )
    .get_matches();

    let episodes: usize = matches.value_of("episodes").unwrap().parse().unwrap();
    let evaluate = matches.is_present("evaluate");
    let map_height: u16 = matches.value_of("map_height").unwrap().parse().unwrap();
    let map_width: u16 = matches.value_of("map_width").unwrap().parse().unwrap();
    // let rel_height: usize = matches.value_of("rel_height").unwrap().parse().unwrap();
    // let rel_width: usize = matches.value_of("rel_width").unwrap().parse().unwrap();
    let output_path = matches.value_of("out").unwrap();
    let steps: u64 = matches.value_of("steps").unwrap().parse().unwrap();
    let verbose = matches.is_present("verbose");

    println!("Training Umpire AI.");

    let qf = {
        let mut agent = {
            let domain_builder = Box::new(move || UmpireDomain::new(Dims::new(map_width, map_height), verbose));
            trained_agent(domain_builder, episodes, steps, verbose)
        };

        let domain_builder = Box::new(move || UmpireDomain::new(Dims::new(map_width, map_height), false));

        if evaluate {
        
            let testing_result = Evaluation::new(&mut agent, domain_builder).next().unwrap();

            println!("solution: {:?}", testing_result);
        }

        agent.q.q_func.0
    };

    // Pry the q function loose
    let qfd = Rc::try_unwrap(qf).unwrap().into_inner();
    let qfdd = Rc::try_unwrap(qfd.0).unwrap().into_inner();

    let rl_ai = RL_AI::new(qfdd);

    let data = bincode::serialize(&rl_ai).unwrap();

    let path = Path::new(output_path);
    let display = path.display();

    let mut file = match File::create(&path) {
        Err(why) => panic!("couldn't create {}: {}", display, why),
        Ok(file) => file,
    };

    match file.write_all(&data) {
        Err(why) => panic!("couldn't write to {}: {}", display, why),
        Ok(_) => println!("successfully wrote to {}", display),
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{
            HashMap,
            HashSet,
        },
        sync::{
            Arc,
            RwLock
        }
    };

    use rand::{
        thread_rng,
    };

    use rsrl::{
        control::Controller,
    };

    use rsrl_domains::Domain;

    use umpire::{
        game::{
            Alignment,
            map::{
                MapData,
                Terrain,
            },
            unit::UnitType, Game,
        },
        name::IntNamer,
        util::{
            Dims,
            Direction,
            Location,
            Wrap2d,
        },
    };

    use super::{
        UmpireAction,
        UmpireDomain,
        trained_agent,
    };

    #[test]
    fn test_ai_movement() {
        let n = 100000;

        let domain_builder = Box::new(move || UmpireDomain::new(Dims::new(10, 10), false));
        let agent = trained_agent(domain_builder, 10, 50, false);


        let mut map = MapData::new(Dims::new(10, 10), |_| Terrain::Land);
        let _unit_id = map.new_unit(Location::new(5,5), UnitType::Infantry,
            Alignment::Belligerent{player:0}, "Aragorn").unwrap();
        let unit_namer = IntNamer::new("unit");
        let game = Game::new_with_map(map, 1, false, Arc::new(RwLock::new(unit_namer)), Wrap2d::BOTH);

        let mut directions: HashSet<Direction> = Direction::values().iter().cloned().collect();

        let mut counts: HashMap<UmpireAction,usize> = HashMap::new();

        let mut domain = UmpireDomain::from_game(game, false);

        let mut rng = thread_rng();
        for _ in 0..n {
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