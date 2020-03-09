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

use crate::{
    game::{
        Game,
        unit::{
            UnitID,
            UnitType,
        },
    },
    util::Location,
};

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

pub trait Player {
    // fn move_unit(&mut self, unit_id: UnitID, game: &PlayerGameView) -> Direction;
    
    // fn set_production(&mut self, city_id: CityID, game: &PlayerGameView) -> UnitType;

    fn take_turn(&mut self, game: &mut Game);
}

pub(crate) mod test_support {
    pub(crate) use crate::game::test_support::{
        game1,
        game_two_cities,
        game_two_cities_two_infantry,
    };
}