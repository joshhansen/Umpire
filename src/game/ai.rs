use rand::seq::SliceRandom;


use crate::{
    game::{
        player::{
            PlayerTurnControl,
            TurnTaker,
        },
        unit::{
            UnitID,
            UnitType,
        },
    },
    util::{
        Location,
    },
};

pub struct RandomAI {
    unit_type_vec: Vec<UnitType>,
}
impl RandomAI {
    pub fn new() -> Self {
        Self {
            unit_type_vec: UnitType::values().to_vec(),
        }
    }
}
impl TurnTaker for RandomAI {
    fn take_turn(&mut self, game: &mut PlayerTurnControl) {
        let mut rng = rand::thread_rng();

        let production_set_requests: Vec<Location> = game.production_set_requests().collect();
        for city_loc in production_set_requests {
            let unit_type = self.unit_type_vec.choose(&mut rng).unwrap();
            game.set_production_by_loc(city_loc, *unit_type).unwrap();
        }

        let unit_orders_requests: Vec<UnitID> = game.unit_orders_requests().collect();
        for unit_id in unit_orders_requests {

            println!("{:?}", game.current_player_observations());

            let possible: Vec<Location> = game.current_player_unit_legal_one_step_destinations(unit_id).unwrap().collect();
            if let Some(dest) = possible.choose(&mut rng) {
                // println!("dest: {:?}", dest);
                println!("{:?} -> {:?}", unit_id, dest);
                game.move_unit_by_id(unit_id, *dest).unwrap();

            } else {
                println!("skip");
                game.order_unit_skip(unit_id).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::{
        Arc,
        RwLock,
    };

    use crate::{
        game::{
            Alignment,
            Game,
            map::{
                MapData,
                terrain::Terrain,

            },
            player::TurnTaker,
        },
        name::IntNamer,
        util::{
            Dims,
            Location,
            Wrap2d,
        },
    };

    use super::RandomAI;

    #[test]
    pub fn test_random_ai() {
        let mut map = MapData::new(Dims::new(100, 100), |_loc| Terrain::Land);
        // let unit_id = map.new_unit(Location::new(0,0), UnitType::Armor, Alignment::Belligerent{player:0}, "Forest Gump").unwrap();
        map.new_city(Location::new(0,0), Alignment::Belligerent{player:0}, "Hebevund").unwrap();

        let unit_namer = IntNamer::new("unit");
        let mut game = Game::new_with_map(map, 1, true, Arc::new(RwLock::new(unit_namer)), Wrap2d::BOTH);
        let mut ctrl = game.player_turn_control(0);

        let mut ai = RandomAI::new();

        for _ in 0..1000 {
            ai.take_turn(&mut ctrl);
        }
    }
}