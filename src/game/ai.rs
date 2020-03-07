use std::collections::HashMap;

use rand::seq::SliceRandom;

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
        Interval,
        ProductSpace,
        Space,
        discrete::Ordinal,
    },
};

use rsrl_domains::{
    Action,
    Observation,
    State,
    Transition,
};

use crate::{
    game::{
        Alignment,
        Game,
        PlayerNum,
        city::CityID,
        map::{
            terrain::Terrain,
            tile::Tile,
        },
        unit::{
            UnitEssentials,
            UnitID,
            UnitType,
        },
    },
    util::{
        Direction,
        Location,
    },
};

// pub enum Alignment {
//     Neutral,
//     Belligerent { player: PlayerNum }
//     // active neutral, chaotic, etc.
// }
struct AlignmentSpace {
    num_players: PlayerNum,
}
impl AlignmentSpace {
    fn new(num_players: PlayerNum) -> Self {
        Self { num_players }
    }
}
impl Space for AlignmentSpace {
    type Value = Alignment;

    fn dim(&self) -> Dim {
        Dim::Finite(2)//?
    }

    fn card(&self) -> Card {
        Card::Finite(1 + self.num_players)
    }
}

struct UnitTypeSpace {
    
}
impl Space for UnitTypeSpace {
    type Value = UnitType;

    fn dim(&self) -> Dim {
        Dim::Finite(1)
    }

    fn card(&self) -> Card {
        Card::Finite(UnitType::values().len())
    }
}

// pub type_: UnitType,
// pub alignment: Alignment,
// hp: u16,
// max_hp: u16,
// moves_remaining: u16,
// // pub orders: Option<Orders>,
// carrying_space: Option<CarryingSpaceEssentials>,

fn unit_space_for_type(num_players: PlayerNum, type_: UnitType) -> ProductSpace<Ordinal> {
    // let type_ = Ordinal::new(UnitType::values().len());
    let alignment = Ordinal::new(num_players + 1);
    let hp = Ordinal::new(type_.max_hp() as usize);
    let moves_remaining = Ordinal::new(type_.movement_per_turn() as usize);

    ProductSpace::new(vec![alignment, hp, moves_remaining])
}

fn unit_space_for_type_without_carrying_space() -> () {

}


// struct TileEssentials {
//     pub terrain: Terrain,
//     pub unit: Option<UnitEssentials>,
//     pub city: Option<CityEssentials>,
//     // pub loc: Location
// }

// struct TileSpace;

// impl Space for TileSpace {
//     type Value = TileEssentials;

//     fn dim(&self) -> Dim { Dim::one() }

//     fn card(&self) -> Card { Card::Finite(2) }
// }


struct PlayerGameControl<'a> {
    game: &'a mut Game,
}
impl <'a> PlayerGameControl<'a> {
    fn unit_orders_requests<'b>(&'b self) -> impl Iterator<Item=UnitID> + 'b {
        self.game.unit_orders_requests()
    }

    fn production_set_requests<'b>(&'b self) -> impl Iterator<Item=Location> + 'b {
        self.game.production_set_requests()
    }

    fn set_production(&mut self, loc: Location, production: UnitType) -> Result<(),String> {
        self.game.set_production(loc, production)
    }
}


trait AI {
    // fn move_unit(&mut self, unit_id: UnitID, game: &PlayerGameView) -> Direction;
    
    // fn set_production(&mut self, city_id: CityID, game: &PlayerGameView) -> UnitType;

    fn take_turn(&mut self, game: &mut PlayerGameControl);
}

struct RandomAI {
    unit_type_vec: Vec<UnitType>,
}
impl RandomAI {
    fn new() -> Self {
        Self {
            unit_type_vec: UnitType::values().to_vec(),
        }
    }
}
impl AI for RandomAI {
    fn take_turn(&mut self, game: &mut PlayerGameControl) {
        while game.production_set_requests().next().is_some() {
            let city_loc = game.production_set_requests().next().unwrap();
            let unit_type = self.unit_type_vec.choose(&mut rand::thread_rng()).unwrap();
            game.set_production(city_loc, *unit_type).unwrap();
        }

        while game.unit_orders_requests().next().is_some() {
            let unit_id = game.unit_orders_requests().next().unwrap();


            
        }
    }
}