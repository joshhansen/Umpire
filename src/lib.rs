#![allow(clippy::cognitive_complexity)]
#![allow(clippy::let_and_return)]
#![allow(clippy::too_many_arguments)]

pub mod color;
pub mod conf;
pub mod game;
pub mod log;
pub mod name;
pub mod ui;
pub mod util;

// use std::sync::mpsc::{
//     Sender,
//     channel,
// };

// use crate::{
//     game::{
//         Game,
//         GameError,
//         city::CityID,
//         move_::MoveResult,
//         unit::{
//             UnitID,
//             UnitType,
//             orders::{
//                 Orders,
//                 OrdersResult,
//             },
//         },
//     },
//     util::{
//         Direction,
//         Location,
//     },
// };



use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        RwLock,
    },
};

use self::game::Game;



pub use self::game::player::{
    PlayerTurnControl,
    TurnPlayer,
};

pub use self::game::player::Player;

// struct GamePlayerFixture<'a> {
//     game: &'a mut Game,
//     players: Vec<Box<RefCell<dyn Player>>>,
// }
// impl <'a> GamePlayerFixture<'a> {
//     fn new(game: &'a mut Game, players: Vec<Box<RefCell<dyn Player>>>) -> Self {
//         Self { game, players }
//     }

//     fn play()
// }


// pub fn play(game: Arc<RwLock<Game>>, players: Vec<Box<RefCell<dyn Player>>>) {
//     while {
//         let game = game.read();
//         game.unwrap().victor().is_none()
//     } {
//         for player in &players {
//             let mut game = game.write();
//             let game = game.as_mut().unwrap();
//             player.borrow_mut().play(game);
//         }
//     }
// }

pub(crate) mod test_support {
    pub(crate) use crate::game::test_support::{
        game1,
        game_two_cities,
        game_two_cities_two_infantry,
    };
}