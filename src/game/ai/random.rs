use rand::seq::SliceRandom;


use crate::{
    game::{
        player::{
            PlayerTurnControl,
            LimitedTurnTaker,
        },
        unit::{
            Unit,
            UnitType,
        },
    },
    util::{
        Location,
    },
};

pub struct RandomAI {
    unit_type_vec: Vec<UnitType>,
    verbose: bool,
}
impl RandomAI {
    pub fn new(verbose: bool) -> Self {
        Self {
            unit_type_vec: UnitType::values().to_vec(),
            verbose,
        }
    }
}
impl LimitedTurnTaker for RandomAI {
    fn take_turn(&mut self, game: &mut PlayerTurnControl) {
        let mut rng = rand::thread_rng();

        if self.verbose {
            println!("Random:\n{:?}", game.current_player_observations());
            println!("Random cities: {}", game.current_player_cities().count());
            println!("Random units: {}", game.current_player_units().count());
        }

        let production_set_requests: Vec<Location> = game.production_set_requests().collect();
        for city_loc in production_set_requests {
            let unit_type = self.unit_type_vec.choose(&mut rng).unwrap();

            if self.verbose {
                println!("{:?} -> {:?}", city_loc, unit_type);
            }

            game.set_production_by_loc(city_loc, *unit_type).unwrap();
        }

        // let unit_orders_requests: Vec<UnitID> = game.unit_orders_requests().collect();
        let units_with_orders_requests: Vec<Unit> = game.units_with_orders_requests().cloned().collect();
        // for unit_id in unit_orders_requests {
        for unit in units_with_orders_requests {
            let unit_id = unit.id;

            let possible: Vec<Location> = match game.current_player_unit_legal_one_step_destinations(unit_id) {
                Ok(it) => it.collect(),
                Err(e) => {
                    let tile = game.current_player_tile(unit.loc);
                    panic!("Error getting destinations for unit with orders request: {}\nunit: {:?}\ntile: {:?}", e, unit, tile)
                }
            };

            // let possible: Vec<Location> = game.current_player_unit_legal_one_step_destinations(unit_id).unwrap().collect();
            if let Some(dest) = possible.choose(&mut rng) {
                // println!("dest: {:?}", dest);
                if self.verbose {
                    let src = game.current_player_unit_loc(unit_id).unwrap();
                    println!("{:?} {} -> {}", unit_id, src, dest);
                }
                let result = game.move_unit_by_id(unit_id, *dest).unwrap();
                if self.verbose && !result.moved_successfully() {
                    println!("**DESTROYED");
                }

                if self.verbose {
                    println!("{:?}", game.current_player_observations());
                }

            } else {
                if self.verbose {
                    println!("skip");
                }
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
            UnitID,
            map::{
                MapData,
                gen::generate_map,
                terrain::Terrain,
            },
            player::LimitedTurnTaker,
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

        
        let mut ai = RandomAI::new(false);
        
        {
            let mut map = MapData::new(Dims::new(100, 100), |_loc| Terrain::Land);
            // let unit_id = map.new_unit(Location::new(0,0), UnitType::Armor, Alignment::Belligerent{player:0}, "Forest Gump").unwrap();
            map.new_city(Location::new(0,0), Alignment::Belligerent{player:0}, "Hebevund").unwrap();

            let unit_namer = IntNamer::new("unit");
            let mut game = Game::new_with_map(map, 1, true, Arc::new(RwLock::new(unit_namer)), Wrap2d::BOTH);
            let mut ctrl = game.player_turn_control(0);

            for _ in 0..1000 {
                ai.take_turn(&mut ctrl);
            }
        }

        let mut ai = RandomAI::new(true);

        for r in 0..100 {
            let players = 2;
            let mut city_namer = IntNamer::new("city");
            let map = generate_map(&mut city_namer, Dims::new(5, 5), players);
            let unit_namer = IntNamer::new("unit");
            let mut game = Game::new_with_map(map, players, true, Arc::new(RwLock::new(unit_namer)), Wrap2d::BOTH);

            for i in 0..300 {
                for player in 0..=1 {
                    let mut ctrl = game.player_turn_control(player);
                    ai.take_turn(&mut ctrl);
                
                    let orders_requests: Vec<UnitID> = ctrl.unit_orders_requests().collect();

                    for rqst_unit_id in orders_requests.iter().cloned() {
                        // Assert that all orders requests correspond to units still present and that the IDs still
                        // match
                        let unit = ctrl.current_player_unit_by_id(rqst_unit_id)
                                              .expect(format!("Unit not found in iteration {}, round {}", i, r).as_str());

                        assert_eq!(unit.id, rqst_unit_id);
                    }

                }

                if game.victor().is_some() {
                    break;
                }
            }
        }
    }
}