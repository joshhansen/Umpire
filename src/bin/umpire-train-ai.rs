//! Tool to train Umpire AI
//! 
//! Strategy:
//! First we bootstrap the AI by having it play against a random baseline.
//! Then we train it against itself.
//! These initial games should have small maps and only two players.
//! 
//! Once we have a simple AI, incorporate it into the UI.
use std::collections::HashMap;

use rsrl::{
    run, make_shared, Evaluation, SerialExperiment,
    control::td::QLearning,
    domains::{Domain, MountainCar},
    fa::linear::{basis::{Fourier, Projector}, optim::SGD, LFA},
    logging,
    policies::{EpsilonGreedy, Greedy, Random},
    spaces::{
        Card,
        Dim,
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
    game::{
        Game,
        PlayerNum,
        city::CityID,
        map::terrain::Terrain,
        unit::UnitID,
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

// pub struct Transition<S, A> {
//     pub from: Observation<S>,
//     pub action: A,
//     pub reward: f64,
//     pub to: Observation<S>,
// }


// pub struct UmpireStateSpace {
//     terrain_counts: HashMap<Terrain,usize>,
// }

// impl Space for UmpireStateSpace {
//     type Value = Game;
    

//     fn dim(&self) -> Dim {

//     }

//     fn card(&self) -> Card {
//         // The cardinality of the game state space will be a function of the map

//         // Every tile could contain every possible combination of player units, as well as being empty
//         // Every carrier unit could contain every possible combination of its owner's units
//         // Every city could have every possible combination of owners, and every possible production assignment (or
//         // none at all).

//         // Every unit and every city could have any combination of its hitpoints from 1..=max
        
//         // Non-essentials like the names of the units are ignored---they're there for decorative purposes but are not
//         // combinatorially relevant.

//         let mut cardinality = 0;

//         for (terrain,count) in self.terrain_counts.iter() {

//         }

//         Card::Finite(cardinality)
//     }
// }


// #[derive(Clone)]
// enum UmpireActionScenario {
//     UnitOrdersRequest {
//         unit_id: UnitID,
//     },
//     ProductionSetRequest {
//         city_id: CityID,
//     }
// }

// struct UmpireActionSpace {

// }

// impl Space for UmpireActionSpace {
//     type Value = UmpireActionScenario;

//     fn dim(&self) -> Dim {

//     }
//     fn card(&self) -> Card {

//     }
// }

// struct UmpirePlayerTurn {
//     /// The player controlled by this AI.
//     player: PlayerNum,

//     // The game state
//     game: Game,
// }

// impl UmpirePlayerTurn {
//     fn new(player: PlayerNum, game: Game) -> Self {
//         Self { player, game }
//     }
// }

// impl Domain for UmpirePlayerTurn {
//     /// State space representation type class.
//     type StateSpace = UmpireStateSpace;

//     /// Action space representation type class.
//     type ActionSpace = UmpireActionSpace;

//     /// Emit an observation of the current state of the environment.
//     fn emit(&self) -> Observation<State<Self>> {
//         Observation::Partial(self.game.clone())
//     }

//     /// Transition the environment forward a single step given an action, `a`.
//     fn step(&mut self, a: Action<Self>) -> Transition<State<Self>, Action<Self>> {

//     }

//     /// Returns an instance of the state space type class.
//     fn state_space(&self) -> Self::StateSpace {

//     }

//     /// Returns an instance of the action space type class.
//     fn action_space(&self) -> Self::ActionSpace {

//     }
// }

fn main() {
    let domain = MountainCar::default();



    // let domain = UmpirePlayerTurn::new(0);
    let mut agent = {
        let n_actions = domain.action_space().card().into();

        let basis = Fourier::from_space(5, domain.state_space()).with_constant();
        let q_func = make_shared(LFA::vector(basis, SGD(1.0), n_actions));

        let policy = EpsilonGreedy::new(
            Greedy::new(q_func.clone()),
            Random::new(n_actions),
            0.2
        );

        QLearning::new(q_func, policy, 0.01, 1.0)
    };

    let logger = logging::root(logging::stdout());
    let domain_builder = Box::new(MountainCar::default);

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