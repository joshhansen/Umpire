//! Reinforcement learning-based AI

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    fmt,
    fs::{File, OpenOptions},
    io::{stdout, Write},
    path::Path,
    sync::Arc,
};

use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{size, Clear, ClearType},
};

use rsrl::{
    control::{td::QLearning, Controller},
    domains::{Action, Domain, Observation, State, Transition},
    fa::{
        linear::{basis::Constant, optim::SGD, VectorFunction, LFA},
        EnumerableStateActionFunction,
    },
    logging, make_shared,
    policies::{EnumerablePolicy, Policy},
    run,
    spaces::{Card, Dim, Interval, ProductSpace, Space},
    OnlineLearner, SerialExperiment, Shared,
};

use rand::{seq::SliceRandom, thread_rng, Rng};

use serde::{Deserialize, Serialize};

use common::{
    game::{
        action::AiPlayerAction,
        ai::{fX, player_features},
        unit::UnitType,
        Game, PlayerNum, PlayerSecret,
    },
    name::IntNamer,
    util::{Dims, Rect, Vec2d, Wrap2d},
};

use crate::{
    color::{palette16, Palette},
    ui::{Component, Draw, Map},
};

use super::{dnn::DNN, GameWithSecrets, AI};

pub type Basis = Constant;
// pub type Basis = Polynomial;

pub type LFA_ = LFA<Basis, SGD, VectorFunction>;
type Agent = UmpireAgent<Shared<Shared<AI>>, UmpireEpsilonGreedy<Shared<AI>>>;

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

        let mut dims: Vec<Interval<f64>> = vec![Interval::new(Some(0.0), Some(1.0))]; // is the tile observed or not?
        for _ in 0..players {
            dims.push(Interval::new(Some(0.0), Some(1.0))); // which player controls the tile (one hot encoded)
        }

        dims.push(Interval::new(Some(0.0), Some(1.0))); // is there a city or not?

        for _ in 0..units {
            // what is the unit type (one hot encoded, all zeros if no unit)
            dims.push(Interval::new(Some(0.0), Some(1.0)));
        }

        // for each of the five potential carried units:
        for _ in 0..5 {
            for _ in 0..units {
                // what is the unit type (one hot encoded, all zeros if no unit)
                dims.push(Interval::new(Some(0.0), Some(1.0)));
            }
        }

        Self {
            space: ProductSpace::new(dims),
        }
    }
}

impl Space for UmpireStateSpace {
    type Value = GameWithSecrets;

    fn dim(&self) -> Dim {
        self.space.dim()
    }

    fn card(&self) -> Card {
        self.space.card()
    }
}

pub struct UmpireActionSpace {
    legal_actions: HashSet<AiPlayerAction>,
}

impl UmpireActionSpace {
    fn from_game_state(game: &Game) -> Self {
        Self {
            legal_actions: AiPlayerAction::legal_actions(game),
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

/// Basically a light form of `Transition` to serialize to disk as part of the memory pool
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Memory {
    from: Vec<fX>,
    action: usize,
    reward: f64,
    to: Vec<fX>,
}

/// The domain of the game of Umpire being played by player 0 against an AI opponent
pub struct UmpireDomain {
    /// The game state
    game: Game,

    player_secrets: Vec<PlayerSecret>,

    verbosity: u8,

    fix_output_loc: bool,

    memory_file: Option<File>,

    memory_prob: f64,

    map: Map,

    palette: Palette,
}

impl UmpireDomain {
    fn new(
        map_dims: Dims,
        num_players: PlayerNum,
        fix_output_loc: bool,
        fog_of_war: bool,
        wrapping: Wrap2d,
        verbosity: u8,
        memory_path: Option<&Path>,
        memory_prob: f64,
    ) -> Result<Self, std::io::Error> {
        let city_namer = IntNamer::new("city");

        let (game, player_secrets) = Game::new(
            map_dims,
            city_namer,
            num_players,
            fog_of_war,
            None,
            wrapping,
        );

        Self::from_game(
            game,
            player_secrets,
            fix_output_loc,
            verbosity,
            memory_path,
            memory_prob,
        )
    }

    fn from_game(
        game: Game,
        secrets: Vec<PlayerSecret>,
        fix_output_loc: bool,
        verbosity: u8,
        memory_path: Option<&Path>,
        memory_prob: f64,
    ) -> Result<Self, std::io::Error> {
        let memory_file = if let Some(memory_path) = memory_path {
            let memory_file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(memory_path)?;
            Some(memory_file)
        } else {
            None
        };

        // Color palette is needed for terminal output when verbosity > 1
        let palette = palette16(game.num_players()).unwrap();

        let mut map = Map::new(
            Rect::new(0, 0, game.dims().width, game.dims().height),
            game.dims(),
            false,
        );
        map.set_viewport_offset(Vec2d::new(0, 0));

        Ok(Self {
            game,
            player_secrets: secrets,
            fix_output_loc,
            verbosity,
            memory_file,
            memory_prob,
            map,
            palette,
        })
    }

    fn update_state(&mut self, action: AiPlayerAction) {
        debug_assert!(!self.game.turn_is_done());

        let player_secret = self.player_secrets[self.game.current_player()];

        action.take(&mut self.game, player_secret).unwrap();

        if self.verbosity > 1 {
            let loc = if let Some(unit_id) = self
                .game
                .player_unit_orders_requests(player_secret)
                .unwrap()
                .next()
            {
                self.game.player_unit_loc(player_secret, unit_id).unwrap()
            } else {
                self.game
                    .player_production_set_requests(player_secret)
                    .unwrap()
                    .next()
            };

            if self.fix_output_loc {
                let mut stdout = stdout();
                {
                    let (ctrl, _turn_start) = self
                        .game
                        .player_turn_control_nonending(player_secret)
                        .unwrap();
                    self.map.draw(&ctrl, &mut stdout, &self.palette);
                }
                execute!(stdout, MoveTo(0, self.map.rect().bottom() + 1)).unwrap();
            } else {
                println!("{:?}", self.game.player_observations(player_secret));
            }

            println!(
                "Player: {} | Turn: {} | Score: {}     ",
                self.game.current_player(),
                self.game.turn(),
                self.game.current_player_score()
            );
            println!(
                "Cities: {} | Units: {}     ",
                self.game.player_cities(player_secret).unwrap().count(),
                self.game.player_units(player_secret).unwrap().count()
            );
            println!(
                "Considering move for: {}     ",
                loc.map_or(String::from(""), |loc| format!("{:?}", loc))
            );
            println!("Action taken: {:?}                         ", action);
        }

        // If the user's turn is done, end it and take a complete turn for the other player until there's something
        // for this user to do or the game is over
        while self.game.victor().is_none() && self.game.turn_is_done() {
            // End this user's turn
            self.game.end_turn(player_secret).unwrap();
        }
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
        let v = GameWithSecrets {
            game: self.game.clone(),
            secrets: self.player_secrets.clone(),
        };

        if self.game.victor().is_some() {
            Observation::Terminal(v)
        } else {
            // Partial unless we happen to be observing every tile in the current turn, which we'll assume doesn't happen
            Observation::Partial(v)
        }
    }

    /// Transition the environment forward a single step given an action, `a`.
    fn step(&mut self, action_idx: usize) -> Transition<State<Self>, Action<Self>> {
        let player_secret = self.player_secrets[self.game.current_player()];
        let start_score = self.game.player_score(player_secret).unwrap();
        let from = self.emit();

        let action = AiPlayerAction::from_idx(action_idx).unwrap();

        self.update_state(action);

        let end_score = self.game.player_score(player_secret).unwrap();
        let to = self.emit();

        let reward = end_score - start_score;

        if self.verbosity > 1 {
            println!("AI Reward: {}     ", reward);
        }

        if let Some(ref mut memory_file) = self.memory_file {
            // With a specified probability, serialize this transition to the memory
            let x: f64 = thread_rng().gen();
            if x <= self.memory_prob {
                let from_state = from.state();
                let to_state = to.state();
                let memory = Memory {
                    from: player_features(&from_state.game, player_secret).unwrap(),
                    action: action_idx,
                    reward,
                    to: player_features(&to_state.game, player_secret).unwrap(),
                };

                let bytes = bincode::serialize(&memory).unwrap();
                memory_file.write_all(&bytes[..]).unwrap();
                memory_file.flush().unwrap();
            }
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
    possible_indices: HashMap<AiPlayerAction, usize>,
}

impl UmpireRandom {
    fn new() -> Self {
        Self {
            possible_indices: AiPlayerAction::possible_actions()
                .iter()
                .enumerate()
                .map(|(i, action)| (*action, i))
                .collect(),
        }
    }

    /// The indices of all legal actions for a given game state, given in a consistent manner regardless of which (if
    /// any) are actually present.
    fn canonical_legal_indices(&self, state: &GameWithSecrets) -> Vec<usize> {
        let legal = AiPlayerAction::legal_actions(&state.game);

        debug_assert!(!legal.is_empty());

        legal
            .iter()
            .map(|action| self.possible_indices.get(action).cloned().unwrap())
            .collect()
    }
}

impl Policy<GameWithSecrets> for UmpireRandom {
    type Action = usize;

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R, state: &GameWithSecrets) -> usize {
        debug_assert!(
            !state.game.turn_is_done(),
            "It makes no sense to sample actions for a game whose current turn is
                                              already done"
        );

        self.canonical_legal_indices(state)
            .choose(rng)
            .cloned()
            .unwrap()
    }

    fn probability(&self, state: &GameWithSecrets, action: &Self::Action) -> f64 {
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
    pub fn new(q_func: Q) -> Self {
        Self(q_func)
    }

    pub fn legal_argmax_qs(qs: &[f64], state: &GameWithSecrets) -> usize {
        debug_assert!(
            !state.game.turn_is_done(),
            "It makes no sense to sample actions for a game whose current turn is
                                              already done"
        );

        debug_assert!(!qs.is_empty());

        let legal = UmpireRandom::new().canonical_legal_indices(state);

        debug_assert!(!legal.is_empty());

        let argmaxima = legal_argmaxima(qs, &legal).1;

        if argmaxima.is_empty() {
            println!(
                "No argmaximum in qs {:?} legal {:?}; choosing randomly",
                qs, legal
            );
            let mut rand = thread_rng();
            *legal.choose(&mut rand).unwrap()
        } else {
            argmaxima[0]
        }
    }
}

impl<Q: EnumerableStateActionFunction<GameWithSecrets>> Policy<GameWithSecrets>
    for UmpireGreedy<Q>
{
    type Action = usize;

    fn mpa(&self, state: &GameWithSecrets) -> usize {
        Self::legal_argmax_qs(&self.0.evaluate_all(state), state)
    }

    fn probability(&self, s: &GameWithSecrets, a: &usize) -> f64 {
        self.probabilities(s)[*a]
    }
}

impl<Q: EnumerableStateActionFunction<GameWithSecrets>> EnumerablePolicy<GameWithSecrets>
    for UmpireGreedy<Q>
{
    fn n_actions(&self) -> usize {
        self.0.n_actions()
    }

    fn probabilities(&self, state: &GameWithSecrets) -> Vec<f64> {
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
    epsilon: Cell<f64>,
    epsilon_decay: f64,
    decay_prob: f64,
    min_epsilon: f64,
}

impl<Q> UmpireEpsilonGreedy<Q> {
    pub fn new(
        greedy: UmpireGreedy<Q>,
        random: UmpireRandom,
        epsilon: f64,
        epsilon_decay: f64,
        decay_prob: f64,
        min_epsilon: f64,
    ) -> Self {
        Self {
            greedy,
            random,

            epsilon: Cell::new(epsilon),
            epsilon_decay,
            decay_prob,
            min_epsilon,
        }
    }
}

impl<Q: EnumerableStateActionFunction<GameWithSecrets>> Policy<GameWithSecrets>
    for UmpireEpsilonGreedy<Q>
{
    type Action = usize;

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R, state: &GameWithSecrets) -> Self::Action {
        let epsilon = self.epsilon.get();
        let action = if rng.gen_bool(epsilon) {
            // println!("RANDOM");
            self.random.sample(rng, state)
        } else {
            // println!("GREEDY");
            self.greedy.sample(rng, state)
        };

        if epsilon > self.min_epsilon && rng.gen_bool(self.decay_prob) {
            let epsilon = (epsilon * self.epsilon_decay).max(self.min_epsilon);
            self.epsilon.set(epsilon);
            println!("Epsilon: {}", epsilon);
        }

        action
    }

    fn mpa(&self, s: &GameWithSecrets) -> Self::Action {
        self.greedy.mpa(s)
    }

    fn probability(&self, s: &GameWithSecrets, a: &Self::Action) -> f64 {
        self.probabilities(s)[*a]
    }
}

impl<Q: EnumerableStateActionFunction<GameWithSecrets>> EnumerablePolicy<GameWithSecrets>
    for UmpireEpsilonGreedy<Q>
{
    fn n_actions(&self) -> usize {
        self.greedy.n_actions()
    }

    fn probabilities(&self, s: &GameWithSecrets) -> Vec<f64> {
        let prs = self.greedy.probabilities(s);
        let epsilon = self.epsilon.get();
        let pr = epsilon / prs.len() as f64;

        prs.into_iter().map(|p| pr + p * (1.0 - epsilon)).collect()
    }
}

/// A Q-Learning agent for the game of Umpire
///
/// Basically a wrapper around `QLearning` which only selects among actions that are legal given the current game state
///
/// # Type Parameters
/// * Q: the q-function approximator
/// * P: the learning policy
pub struct UmpireAgent<Q, P> {
    pub q: QLearning<Q, P>,
    avoid_skip: bool,
}

impl<Q, P> OnlineLearner<GameWithSecrets, P::Action> for UmpireAgent<Q, P>
where
    Q: EnumerableStateActionFunction<GameWithSecrets>,
    P: EnumerablePolicy<GameWithSecrets>,
{
    fn handle_transition(&mut self, t: &Transition<GameWithSecrets, P::Action>) {
        self.q.handle_transition(t)
    }
}

impl<Q, P> Controller<GameWithSecrets, P::Action> for UmpireAgent<Q, P>
where
    Q: EnumerableStateActionFunction<GameWithSecrets>,
    P: EnumerablePolicy<GameWithSecrets>,
{
    fn sample_target(&self, _: &mut impl Rng, s: &GameWithSecrets) -> P::Action {
        find_legal_max(&self.q.q_func, s, self.avoid_skip).0
    }

    fn sample_behaviour(&self, rng: &mut impl Rng, s: &GameWithSecrets) -> P::Action {
        self.q.sample_behaviour(rng, s)
    }
}

fn agent(
    initialize_from: AI,
    deep: bool,
    alpha: f64,
    gamma: f64,
    epsilon: f64,
    epsilon_decay: f64,
    decay_prob: f64,
    min_epsilon: f64,
    dnn_learning_rate: f32,
    avoid_skip: bool,
) -> Result<Agent, String> {
    let n_actions = AiPlayerAction::possible_actions().len();

    let q_func = match initialize_from {
        AI::Random(_) => {
            let fa_ai = if deep {
                AI::DNN(DNN::new(dnn_learning_rate)?)
            } else {
                // let basis = Fourier::from_space(2, domain_builder().state_space().space).with_constant();
                let basis = Constant::new(5.0);
                // let basis = Polynomial::new(2, 1);
                let fa = LFA::vector(basis, SGD(0.001), n_actions);
                AI::LFA(fa)
            };
            fa_ai
        }
        other => other,
    };

    let q_func = make_shared(q_func);

    let policy = UmpireEpsilonGreedy::new(
        UmpireGreedy::new(q_func.clone()),
        UmpireRandom::new(),
        epsilon,
        epsilon_decay,
        decay_prob,
        min_epsilon,
    );

    Ok(UmpireAgent {
        q: QLearning::new(q_func, policy, alpha, gamma),
        avoid_skip,
    })
}

pub fn trained_agent(
    initialize_from: AI,
    deep: bool,
    training_players: PlayerNum,
    dims: Vec<Dims>,
    wrappings: Vec<Wrap2d>,
    episodes: usize,
    steps: u64,
    alpha: f64,
    gamma: f64,
    epsilon: f64,
    epsilon_decay: f64,
    decay_prob: f64,
    min_epsilon: f64,
    dnn_learning_rate: f32,
    avoid_skip: bool,
    fix_output_loc: bool,
    fog_of_war: bool,
    verbosity: u8,
    memory_path: Option<&'static Path>,
    memory_prob: f64,
) -> Result<Agent, String> {
    if training_players > 4 {
        return Err(format!(
            "Max players in training game is 4 but {} was specified",
            training_players
        ));
    }

    let mut agent = agent(
        initialize_from,
        deep,
        alpha,
        gamma,
        epsilon,
        epsilon_decay,
        decay_prob,
        min_epsilon,
        dnn_learning_rate,
        avoid_skip,
    )?;

    let episode = Arc::new(RefCell::new(1_usize));
    let domain_builder = Box::new(move || {
        let mut rng = thread_rng();

        let dims = dims.choose(&mut rng).unwrap();
        let wrapping = wrappings.choose(&mut rng).unwrap();

        if fix_output_loc {
            let (term_width, term_height) = size().unwrap();
            let mut stdout = stdout();
            execute!(stdout, MoveTo(term_width, term_height - 8)).unwrap();
            execute!(stdout, Clear(ClearType::FromCursorUp)).unwrap();

            execute!(stdout, MoveTo(0, term_height - 13)).unwrap();
            println!("Episode: {}", episode.borrow());
            println!("Map Dimensions: {:?}", dims);
            println!("Wrapping: {:?}", wrapping);
            println!("Fog of War: {}", if fog_of_war { "on" } else { "off" });
            *episode.borrow_mut() += 1;
        }

        UmpireDomain::new(
            *dims,
            training_players,
            fix_output_loc,
            fog_of_war,
            *wrapping,
            verbosity,
            memory_path,
            memory_prob,
        )
        .unwrap()
    });

    // Start a serial learning experiment up to 1000 steps per episode.
    let e = SerialExperiment::new(&mut agent, domain_builder, steps);

    // Realise 1000 episodes of the experiment generator.
    run(
        e,
        episodes,
        if verbosity > 0 {
            let logger = logging::root(logging::stdout());
            Some(logger)
        } else {
            None
        },
    );

    Ok(agent)
}

pub fn find_legal_max<Q: EnumerableStateActionFunction<GameWithSecrets>>(
    q_func: &Q,
    state: &GameWithSecrets,
    avoid_skip: bool,
) -> (usize, f64) {
    let mut legal = AiPlayerAction::legal_actions(&state.game);

    let possible = AiPlayerAction::possible_actions();

    let mut qs = q_func.evaluate_all(state);

    if legal.contains(&AiPlayerAction::SkipNextUnit) && legal.len() > 1 && avoid_skip {
        legal.remove(&AiPlayerAction::SkipNextUnit);
        qs.remove(AiPlayerAction::SkipNextUnit.to_idx());
    }

    qs.into_iter()
        .enumerate()
        .filter(|(i, _x)| legal.contains(possible.get(*i).unwrap()))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
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

#[cfg(test)]
mod test {
    use std::collections::{HashMap, HashSet};

    use rand::thread_rng;

    use rsrl::{control::Controller, domains::Domain};

    use common::{
        game::{
            alignment::Alignment,
            map::{MapData, Terrain},
            unit::UnitType,
            Game,
        },
        util::{Dims, Direction, Location, Wrap2d},
    };

    use crate::game::ai::{
        rl::{trained_agent, AiPlayerAction, UmpireDomain},
        AI,
    };

    #[test]
    fn test_ai_movement() {
        let n = 10_000;

        let mut map = MapData::new(Dims::new(10, 10), |_| Terrain::Land);
        let _unit_id = map
            .new_unit(
                Location::new(5, 5),
                UnitType::Infantry,
                Alignment::Belligerent { player: 0 },
                "Aragorn",
            )
            .unwrap();

        let mut directions: HashSet<Direction> = Direction::values().iter().cloned().collect();

        let mut counts: HashMap<AiPlayerAction, usize> = HashMap::new();

        let (game, secrets) = Game::new_with_map(map, 1, false, None, Wrap2d::BOTH);

        let agent = trained_agent(
            AI::random(0, false),
            false,
            secrets.len(),
            vec![Dims::new(10, 10)],
            vec![Wrap2d::BOTH],
            10,
            50,
            0.05,
            0.90,
            0.05,
            0.999,
            0.0001,
            0.2,
            0.001,
            false,
            false,
            true,
            0,
            None,
            std::f64::NAN,
        )
        .unwrap();

        let mut domain =
            UmpireDomain::from_game(game.clone(), secrets.clone(), false, 0, None, std::f64::NAN)
                .unwrap();

        let mut rng = thread_rng();
        for _ in 0..n {
            // Reinitialize when somebody wins
            if domain.game.victor().is_some() {
                domain = UmpireDomain::from_game(
                    game.clone(),
                    secrets.clone(),
                    false,
                    0,
                    None,
                    std::f64::NAN,
                )
                .unwrap();
            }

            let idx = agent.sample_behaviour(&mut rng, domain.emit().state());

            domain.step(idx);

            let action = AiPlayerAction::from_idx(idx).unwrap();

            println!("Action: {:?}", action);

            *counts.entry(action).or_insert(0) += 1;

            if let AiPlayerAction::MoveNextUnit { direction } = action {
                directions.remove(&direction);
            }
        }

        assert!(
            directions.is_empty(),
            "AI is failing to explore in these directions over {} steps: {}\nCounts: {}",
            n,
            directions
                .iter()
                .map(|dir| format!("{:?} ", dir))
                .collect::<String>(),
            counts
                .iter()
                .map(|(k, v)| format!("{:?}:{} ", k, v))
                .collect::<String>()
        );
    }
}
