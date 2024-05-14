use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
};

use rand::Rng;

use crate::{
    game::{
        map::{MapData, Terrain},
        move_::MoveError,
        obs::Obs,
        test_support::game_two_cities_two_infantry,
        unit::{
            orders::{Orders, OrdersStatus},
            Fuel, TransportMode, Unit, UnitID, UnitType,
        },
        Alignment, Game, GameError, TurnNum,
    },
    name::{unit_namer, Named},
    util::{Dimensioned, Dims, Direction, Location, Vec2d, Wrap2d},
};

use super::ai::TrainingFocus;

#[test]
fn test_game() {
    let (mut game, secrets) = game_two_cities_two_infantry();

    for player in 0..2 {
        assert_eq!(game.current_player_unit_orders_requests().count(), 1);
        let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();
        let loc = game.current_player_unit_loc(unit_id).unwrap();
        let new_x = (loc.x + 1) % game.dims().width;
        let new_loc = Location { x: new_x, y: loc.y };
        println!("Moving unit from {} to {}", loc, new_loc);

        match game.move_toplevel_unit_by_loc(secrets[player], loc, new_loc) {
            Ok(move_result) => {
                println!("{:?}", move_result);
            }
            Err(msg) => {
                panic!("Error during move: {}", msg);
            }
        }

        let result = game.end_then_begin_turn(secrets[player], secrets[(player + 1) % 2], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().current_player, 1 - player);
    }
}

#[test]
fn test_move_unit_by_id_far() {
    let mut map = MapData::new(Dims::new(180, 90), |_| Terrain::Water);
    let unit_id = map
        .new_unit(
            Location::new(0, 0),
            UnitType::Fighter,
            Alignment::Belligerent { player: 0 },
            "Han Solo",
        )
        .unwrap();

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::BOTH);

    game.begin_turn(secrets[0], false).unwrap();

    for i in 0..10 {
        let mut delta = Vec2d::new(0, 0);

        while delta.x == 0 && delta.y == 0 {
            delta = Vec2d::new(game.rng.gen_range(-5..6), game.rng.gen_range(-5..6));
        }

        let unit_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
        let dest = game
            .wrapping()
            .wrapped_add(game.dims(), unit_loc, delta)
            .unwrap();

        match game.move_unit_by_id(secrets[0], unit_id, dest) {
            Ok(result) => {
                // If the move happens, but is considered unsuccessful, make sure it's only
                // due to running out of fuel.
                // In such a case, we're done moving so break
                if !result.moved_successfully() {
                    assert!(result.fuel_ran_out());
                    break;
                }

                assert_eq!(result.ending_loc(), Some(dest));
            }
            Err(e) => {
                // The only error we're expecting is not having enough fuel to make the move
                assert_eq!(e, GameError::MoveError(MoveError::InsufficientFuel));
            }
        }

        // Make sure there are no duplicate unit observations
        let observed_units = game
            .player_observations(secrets[0])
            .unwrap()
            .iter()
            .filter(|obs| match obs {
                Obs::Observed { tile, .. } => match tile.unit.as_ref() {
                    Some(unit) => {
                        assert_eq!(unit.id, unit_id);
                        assert_eq!(unit.type_, UnitType::Fighter);
                        true
                    }
                    None => false,
                },
                Obs::Unobserved => false,
            })
            .count();
        assert_eq!(
            observed_units, 1,
            "Extra copies of the unit found after move {}",
            i
        );

        game.force_end_then_begin_turn(secrets[0], secrets[0], false)
            .unwrap();
    }
}

#[test]
fn test_move_unit() {
    let map = MapData::try_from("--0-+-+-1--").unwrap();
    {
        let loc1 = Location { x: 2, y: 0 };
        let loc2 = Location { x: 8, y: 0 };

        let city1tile = map.tile(loc1).unwrap();
        let city2tile = map.tile(loc2).unwrap();
        assert_eq!(city1tile.terrain, Terrain::Land);
        assert_eq!(city2tile.terrain, Terrain::Land);

        let city1 = city1tile.city.as_ref().unwrap();
        let city2 = city2tile.city.as_ref().unwrap();
        assert_eq!(city1.alignment, Alignment::Belligerent { player: 0 });
        assert_eq!(city2.alignment, Alignment::Belligerent { player: 1 });
        assert_eq!(city1.loc, loc1);
        assert_eq!(city2.loc, loc2);
    }

    let (mut game, secrets) = Game::new_with_map(
        None,
        false,
        map,
        2,
        false,
        Some(Arc::new(RwLock::new(unit_namer(None)))),
        Wrap2d::BOTH,
    );
    assert_eq!(game.current_player(), 0);

    game.begin_turn(secrets[0], false).unwrap();

    {
        let loc = game
            .player_production_set_requests(secrets[0])
            .unwrap()
            .next()
            .unwrap();

        assert_eq!(
            game.set_production_by_loc(secrets[0], loc, UnitType::Armor)
                .map(|ps| ps.prior_production),
            Ok(None)
        );

        let result = game.end_then_begin_turn(secrets[0], secrets[1], false);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().current_player, 1);
    }

    {
        let loc = game
            .player_production_set_requests(secrets[1])
            .unwrap()
            .next()
            .unwrap();

        assert_eq!(
            game.set_production_by_loc(secrets[1], loc, UnitType::Carrier)
                .map(|ps| ps.prior_production),
            Ok(None)
        );

        let result = game.end_then_begin_turn(secrets[1], secrets[0], false);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().current_player, 0);
    }

    for _ in 0..(UnitType::Armor.cost() - 1) {
        let result = game.end_then_begin_turn(secrets[0], secrets[1], false);
        assert_eq!(result.unwrap().current_player, 1);

        let result = game.end_then_begin_turn(secrets[1], secrets[0], false);
        assert_eq!(result.unwrap().current_player, 0);
    }
    assert_eq!(
        game.end_then_begin_turn(secrets[0], secrets[1], false),
        Err(GameError::TurnEndRequirementsNotMet { player: 0 })
    );

    // Move the armor unit to the right until it attacks the opposing city
    for round in 0..3 {
        assert_eq!(game.current_player_unit_orders_requests().count(), 1);
        let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();
        let loc = {
            let unit = game.current_player_unit_by_id(unit_id).unwrap();
            assert_eq!(unit.type_, UnitType::Armor);
            unit.loc
        };

        let dest_loc = Location {
            x: loc.x + 2,
            y: loc.y,
        };
        println!("Moving from {} to {}", loc, dest_loc);
        let move_result = game
            .move_toplevel_unit_by_loc(secrets[0], loc, dest_loc)
            .unwrap();
        println!("Result: {:?}", move_result);

        assert_eq!(move_result.unit.type_, UnitType::Armor);
        assert_eq!(
            move_result.unit.alignment,
            Alignment::Belligerent { player: 0 }
        );

        // Check the first move component
        assert_eq!(move_result.components.len(), 2);
        let move1 = move_result.components.first().unwrap();
        assert_eq!(
            move1.loc,
            Location {
                x: loc.x + 1,
                y: loc.y
            }
        );
        assert_eq!(move1.unit_combat, None);
        assert_eq!(move1.city_combat, None);

        if move_result.moved_successfully() {
            // the unit conquered the city

            assert_eq!(move_result.ending_loc().unwrap(), dest_loc);

            assert_eq!(move_result.unit.moves_remaining(), 0);

            // Check the second move component, only here because the unit wasn't destroyed
            let move2 = move_result.components.get(1).unwrap();
            assert_eq!(move2.loc, dest_loc);
            assert_eq!(move2.unit_combat, None);

            if round < 2 {
                assert_eq!(move2.city_combat, None);
            } else {
                assert!(move2.city_combat.is_some());

                // Since the armor defeated the city, set its production so we can end the turn
                let conquered_city = move_result.conquered_city().unwrap();
                let production_set_result =
                    game.set_production_by_loc(secrets[0], conquered_city.loc, UnitType::Fighter);
                assert_eq!(
                    production_set_result.map(|ps| ps.prior_production),
                    Ok(Some(UnitType::Carrier))
                );
            }
        } else {
            // The unit was destroyed
            assert_eq!(move_result.unit.moves_remaining(), 1);
        }

        let result = game.end_then_begin_turn(secrets[0], secrets[1], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().current_player, 1);

        let result = game.end_then_begin_turn(secrets[1], secrets[0], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().current_player, 0);
    }
}

#[test]
fn test_terrainwise_movement() {
    let mut map = MapData::try_from(" t-").unwrap();
    map.set_terrain(Location::new(1, 0), Terrain::Water)
        .unwrap();

    let transport_id = map.toplevel_unit_by_loc(Location::new(1, 0)).unwrap().id;

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::BOTH);

    game.begin_turn(secrets[0], false).unwrap();

    game.move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Left)
        .unwrap();
    game.move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Right)
        .unwrap();

    assert_eq!(
        game.move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Right),
        Err(GameError::MoveError(MoveError::NoRoute {
            id: transport_id,
            src: Location::new(1, 0),
            dest: Location::new(2, 0),
        }))
    );
}

#[test]
fn test_unit_moves_onto_transport() {
    let map = MapData::try_from("---it   ").unwrap();
    let infantry_loc = Location { x: 3, y: 0 };
    let transport_loc = Location { x: 4, y: 0 };

    let transport_id: UnitID = map.toplevel_unit_id_by_loc(transport_loc).unwrap();

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::BOTH);

    game.begin_turn(secrets[0], false).unwrap();

    let move_result = game
        .move_toplevel_unit_by_loc(secrets[0], infantry_loc, transport_loc)
        .unwrap();
    assert_eq!(move_result.starting_loc, infantry_loc);
    assert_eq!(move_result.ending_loc(), Some(transport_loc));
    assert!(move_result.moved_successfully());
    assert_eq!(move_result.ending_carrier(), Some(transport_id));
}

#[test]
fn test_loaded_transport_attack() {
    let mut victorious = false;
    let mut defeated = false;
    while !victorious || !defeated {
        let map = MapData::try_from("itP").unwrap();

        let infantry_id = map.toplevel_unit_by_loc(Location::new(0, 0)).unwrap().id;
        let transport_id = map.toplevel_unit_by_loc(Location::new(1, 0)).unwrap().id;
        let battleship_id = map.toplevel_unit_by_loc(Location::new(2, 0)).unwrap().id;

        let (mut game, secrets) =
            Game::new_with_map(None, false, map, 2, false, None, Wrap2d::NEITHER);

        game.begin_turn(secrets[0], false).unwrap();

        // Load the infantry onto the transport
        let inf_move = game
            .move_unit_by_id_in_direction(secrets[0], infantry_id, Direction::Right)
            .unwrap();
        assert!(inf_move.moved_successfully());
        assert_eq!(
            inf_move.ending_loc(),
            game.current_player_unit_loc(transport_id)
        );
        assert_eq!(inf_move.ending_carrier(), Some(transport_id));

        // Attack the battleship with the transport
        let move_ = game
            .move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Right)
            .unwrap();
        if move_.moved_successfully() {
            victorious = true;

            assert!(game
                .current_player_units()
                .any(|unit| unit.id == infantry_id));
            assert!(game
                .current_player_units()
                .any(|unit| unit.id == transport_id));

            assert_eq!(
                game.current_player_unit_by_id(infantry_id).unwrap().loc,
                Location::new(2, 0)
            );
            assert_eq!(
                game.current_player_unit_by_id(transport_id).unwrap().loc,
                Location::new(2, 0)
            );

            assert_eq!(
                game.current_player_tile(Location::new(0, 0))
                    .unwrap()
                    .unit
                    .as_ref(),
                None
            );
            assert_eq!(
                game.current_player_tile(Location::new(1, 0))
                    .unwrap()
                    .unit
                    .as_ref(),
                None
            );
            {
                let unit = game
                    .current_player_tile(Location::new(2, 0))
                    .unwrap()
                    .unit
                    .as_ref()
                    .unwrap();
                assert_eq!(unit.type_, UnitType::Transport);
                assert_eq!(unit.id, transport_id);
                assert!(unit
                    .carried_units()
                    .any(|carried_unit| carried_unit.id == infantry_id));
            }

            game.force_end_then_begin_turn(secrets[0], secrets[1], false)
                .unwrap(); // ignore remaining moves

            assert!(!game
                .current_player_units()
                .any(|unit| unit.id == battleship_id));
            assert!(!game
                .current_player_unit_orders_requests()
                .any(|unit_id| unit_id == battleship_id));
        } else {
            defeated = true;

            assert!(!game
                .current_player_units()
                .any(|unit| unit.id == infantry_id));
            assert!(!game
                .current_player_units()
                .any(|unit| unit.id == transport_id));

            assert_eq!(game.current_player_unit_by_id(infantry_id), None);
            assert_eq!(game.current_player_unit_by_id(transport_id), None);

            assert_eq!(
                game.current_player_tile(Location::new(0, 0))
                    .unwrap()
                    .unit
                    .as_ref(),
                None
            );
            assert_eq!(
                game.current_player_tile(Location::new(1, 0))
                    .unwrap()
                    .unit
                    .as_ref(),
                None
            );
            assert_eq!(
                game.current_player_tile(Location::new(2, 0))
                    .unwrap()
                    .unit
                    .as_ref()
                    .unwrap()
                    .id,
                battleship_id
            );

            game.end_then_begin_turn(secrets[0], secrets[1], false)
                .unwrap();

            assert!(game
                .current_player_units()
                .any(|unit| unit.id == battleship_id));
            assert!(game
                .current_player_unit_orders_requests()
                .any(|unit_id| unit_id == battleship_id));
        }
    }
}

#[test]
fn test_set_orders() {
    let map = MapData::try_from("i").unwrap();
    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::NEITHER);
    let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();

    assert_eq!(
        game.current_player_unit_by_id(unit_id).unwrap().orders,
        None
    );
    assert_eq!(
        game.current_player_unit_by_id(unit_id).unwrap().name(),
        &String::from("Unit_0_0")
    );

    game.set_orders(secrets[0], unit_id, Orders::Sentry)
        .unwrap();

    assert_eq!(
        game.current_player_unit_by_id(unit_id).unwrap().orders,
        Some(Orders::Sentry)
    );
}

#[test]
pub fn test_order_unit_explore() {
    let map = MapData::try_from("i--------------------").unwrap();
    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::NEITHER);

    game.begin_turn(secrets[0], false).unwrap();

    let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();

    let outcome = game.order_unit_explore(secrets[0], unit_id).unwrap();
    assert_eq!(outcome.ordered_unit.id, unit_id);
    assert_eq!(outcome.orders, Orders::Explore);
    assert_eq!(outcome.status, OrdersStatus::InProgress);
}

#[test]
pub fn test_propose_move_unit_by_id() {
    super::test_support::test_propose_move_unit_by_id();
}

#[test]
pub fn test_current_player_unit_legal_one_step_destinations() {
    let dirs = [
        Direction::UpLeft,
        Direction::Up,
        Direction::UpRight,
        Direction::Left,
        Direction::Right,
        Direction::DownLeft,
        Direction::Down,
        Direction::DownRight,
    ];

    let possible: Vec<char> = " 01iI".chars().collect();
    let mut traversable: BTreeMap<char, bool> = BTreeMap::new();
    traversable.insert(' ', false); //water
    traversable.insert('0', true); //friendly city
    traversable.insert('1', true); //enemy city
    traversable.insert('i', false); //friendly unit
    traversable.insert('I', true); //enemy unit

    for up_left in &possible {
        for up in &possible {
            for up_right in &possible {
                for left in &possible {
                    for right in &possible {
                        for down_left in &possible {
                            for down in &possible {
                                for down_right in &possible {
                                    let cs: Vec<char> = dirs
                                        .iter()
                                        .map(|dir| match dir {
                                            Direction::UpLeft => up_left,
                                            Direction::Up => up,
                                            Direction::UpRight => up_right,
                                            Direction::Left => left,
                                            Direction::Right => right,
                                            Direction::DownLeft => down_left,
                                            Direction::Down => down,
                                            Direction::DownRight => down_right,
                                        })
                                        .cloned()
                                        .collect();

                                    let s = format!(
                                        "{}{}{}\n{}i{}\n{}{}{}",
                                        cs[0], cs[1], cs[2], cs[3], cs[4], cs[5], cs[6], cs[7]
                                    );

                                    let map = MapData::try_from(s.clone()).unwrap();
                                    assert_eq!(map.dims(), Dims::new(3, 3));

                                    let (mut game, secrets) = Game::new_with_map(
                                        None,
                                        false,
                                        map,
                                        2,
                                        false,
                                        None,
                                        Wrap2d::BOTH,
                                    );

                                    game.begin_turn(secrets[0], false).unwrap();

                                    let id = game
                                        .current_player_toplevel_unit_by_loc(Location {
                                            x: 1,
                                            y: 1,
                                        })
                                        .unwrap()
                                        .id;

                                    let inclusions: Vec<bool> = cs
                                        .iter()
                                        .map(|c| traversable.get(c).unwrap())
                                        .cloned()
                                        .collect();

                                    assert_eq!(cs.len(), inclusions.len());
                                    assert_eq!(cs.len(), dirs.len());

                                    let src = Location::new(1, 1);
                                    let dests: BTreeSet<Location> = game
                                        .player_unit_legal_one_step_destinations(secrets[0], id)
                                        .unwrap();

                                    for (i, loc) in dirs
                                        .iter()
                                        .map(|dir| {
                                            let v: Vec2d<i32> = (*dir).into();
                                            Location {
                                                x: ((src.x as i32) + v.x) as u16,
                                                y: ((src.y as i32) + v.y) as u16,
                                            }
                                        })
                                        .enumerate()
                                    {
                                        if inclusions[i] {
                                            assert!(
                                                dests.contains(&loc),
                                                "Erroneously omitted {:?} on \"{}\"",
                                                loc,
                                                s.replace('\n', "\\n")
                                            );
                                        } else {
                                            assert!(
                                                !dests.contains(&loc),
                                                "Erroneously included {:?} on \"{}\"",
                                                loc,
                                                s.replace('\n', "\\n")
                                            );
                                        }
                                    }
                                } // down_right
                            } // down
                        } // down_left
                    } // right
                } // left
            } // up_right
        } // up
    } // up_left
}

#[test]
fn test_current_player_unit_legal_one_step_destinations_wrapping() {
    // Make sure the same destinations are found in these cases regardless of wrapping
    for wrapping in Wrap2d::values().iter().cloned() {
        {
            // 1x1
            let mut map = MapData::new(Dims::new(1, 1), |_loc| Terrain::Land);
            let unit_id = map
                .new_unit(
                    Location::new(0, 0),
                    UnitType::Infantry,
                    Alignment::Belligerent { player: 0 },
                    "Eunice",
                )
                .unwrap();
            let (game, secrets) = Game::new_with_map(None, false, map, 1, false, None, wrapping);

            assert!(game
                .player_unit_legal_one_step_destinations(secrets[0], unit_id)
                .unwrap()
                .is_empty());
        }

        {
            // 2x1
            let mut map = MapData::new(Dims::new(2, 1), |_loc| Terrain::Land);
            let unit_id = map
                .new_unit(
                    Location::new(0, 0),
                    UnitType::Infantry,
                    Alignment::Belligerent { player: 0 },
                    "Eunice",
                )
                .unwrap();
            let (game, secrets) = Game::new_with_map(None, false, map, 1, false, None, wrapping);

            let dests: BTreeSet<Location> = game
                .player_unit_legal_one_step_destinations(secrets[0], unit_id)
                .unwrap();
            assert_eq!(
                dests.len(),
                1,
                "Bad dests: {:?} with wrapping {:?}",
                dests,
                wrapping
            );
            assert!(dests.contains(&Location::new(1, 0)));
        }

        {
            // 3x1
            let mut map = MapData::new(Dims::new(3, 1), |_loc| Terrain::Land);
            let unit_id = map
                .new_unit(
                    Location::new(1, 0),
                    UnitType::Infantry,
                    Alignment::Belligerent { player: 0 },
                    "Eunice",
                )
                .unwrap();
            let (game, secrets) = Game::new_with_map(None, false, map, 1, false, None, wrapping);

            let dests: BTreeSet<Location> = game
                .player_unit_legal_one_step_destinations(secrets[0], unit_id)
                .unwrap();
            assert_eq!(
                dests.len(),
                2,
                "Bad dests: {:?} with wrapping {:?}",
                dests,
                wrapping
            );
            assert!(dests.contains(&Location::new(0, 0)));
            assert!(dests.contains(&Location::new(2, 0)));
        }

        {
            // 3x1 with infantry in transport
            let mut map = MapData::try_from(".ti").unwrap();
            let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
            let inf_id = map.toplevel_unit_id_by_loc(Location::new(2, 0)).unwrap();
            map.carry_unit_by_id(transport_id, inf_id).unwrap();

            let (game, secrets) = Game::new_with_map(None, false, map, 1, false, None, wrapping);

            let dests: BTreeSet<Location> = game
                .player_unit_legal_one_step_destinations(secrets[0], inf_id)
                .unwrap();
            assert_eq!(
                dests.len(),
                2,
                "Bad dests: {:?} with wrapping {:?}",
                dests,
                wrapping
            );
            assert!(dests.contains(&Location::new(0, 0)));
            assert!(dests.contains(&Location::new(2, 0)));
        }
    }
}

#[test]
pub fn test_one_step_routes() {
    let mut map = MapData::new(Dims::new(10, 10), |_loc| Terrain::Land);
    let unit_id = map
        .new_unit(
            Location::new(0, 0),
            UnitType::Armor,
            Alignment::Belligerent { player: 0 },
            "Forest Gump",
        )
        .unwrap();

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::BOTH);

    game.begin_turn(secrets[0], false).unwrap();

    for (i, src) in game.dims().iter_locs().enumerate() {
        // for _ in 0..1000 {
        //     let src = game.dims().sample(&mut rand);

        // // Recenter the unit on `src`
        // game.map.relocate_unit_by_id(unit_id, src).unwrap();

        // Recenter the unit on `src`
        if i > 0 {
            game.move_unit_by_id(secrets[0], unit_id, src).unwrap();
            game.order_unit_skip(secrets[0], unit_id).unwrap();
            game.end_then_begin_turn(secrets[0], secrets[0], false)
                .unwrap();
        }

        for dir in Direction::values().iter().cloned() {
            let src = game.current_player_unit_loc(unit_id).unwrap();
            let dest = game
                .wrapping
                .wrapped_add(game.dims(), src, dir.into())
                .unwrap();

            game.move_unit_by_id(secrets[0], unit_id, dest).expect(
                format!(
                    "Error moving unit with ID {:?} from {} to {}",
                    unit_id, src, dest
                )
                .as_str(),
            );
            assert_eq!(
                game.current_player_unit_loc(unit_id),
                Some(dest),
                "Wrong location after moving {:?} from {:?} to {:?}",
                dir,
                src,
                dest
            );

            game.move_unit_by_id(secrets[0], unit_id, src).expect(
                format!(
                    "Error moving unit with ID {:?} from {} to {}",
                    unit_id, dest, src
                )
                .as_str(),
            );
            game.end_then_begin_turn(secrets[0], secrets[0], false)
                .unwrap();

            game.move_unit_by_id_in_direction(secrets[0], unit_id, dir)
                .unwrap();
            assert_eq!(
                game.current_player_unit_loc(unit_id),
                Some(dest),
                "Wrong location after moving {:?} from {:?} to {:?}",
                dir,
                src,
                dest
            );

            game.move_unit_by_id_in_direction(secrets[0], unit_id, dir.opposite())
                .unwrap();
            game.end_then_begin_turn(secrets[0], secrets[0], false)
                .unwrap();
        }
    }
}

#[test]
pub fn test_order_unit_skip() {
    let mut map = MapData::new(Dims::new(10, 10), |_loc| Terrain::Land);
    let unit_id = map
        .new_unit(
            Location::new(0, 0),
            UnitType::Infantry,
            Alignment::Belligerent { player: 0 },
            "Skipper",
        )
        .unwrap();
    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::BOTH);

    game.begin_turn(secrets[0], false).unwrap();

    game.move_unit_by_id_in_direction(secrets[0], unit_id, Direction::Right)
        .unwrap();
    game.end_then_begin_turn(secrets[0], secrets[0], false)
        .unwrap();

    game.order_unit_skip(secrets[0], unit_id).unwrap();
    game.end_then_begin_turn(secrets[0], secrets[0], false)
        .unwrap();

    assert_eq!(
        game.current_player_unit_orders_requests().next(),
        Some(unit_id)
    );

    game.current_player_unit_by_id(unit_id).unwrap();
}

#[test]
pub fn test_movement_matches_carry_status() {
    let l1 = Location::new(0, 0);
    let l2 = Location::new(1, 0);
    let a = Alignment::Belligerent { player: 0 };

    for type1 in UnitType::values().iter().cloned() {
        let u1 = Unit::new(UnitID::new(0), l2, type1, a, "u1");

        for type2 in UnitType::values().iter().cloned() {
            let u2 = Unit::new(UnitID::new(1), l2, type2, a, "u2");

            let mut map = MapData::new(Dims::new(2, 1), |loc| {
                let mode = if loc == l1 {
                    u1.transport_mode()
                } else {
                    u2.transport_mode()
                };

                match mode {
                    TransportMode::Sea => Terrain::Water,
                    TransportMode::Land => Terrain::Land,
                    TransportMode::Air => Terrain::Land,
                }
            });

            map.set_unit(l1, u1.clone());
            map.set_unit(l2, u2.clone());

            let (mut game, secrets) =
                Game::new_with_map(None, false, map, 1, false, None, Wrap2d::NEITHER);

            game.begin_turn(secrets[0], false).unwrap();

            let result = game.move_unit_by_id_in_direction(secrets[0], u1.id, Direction::Right);

            if u2.can_carry_unit(&u1) {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
            }
        }
    }
}

#[test]
pub fn test_id_consistency() {
    let mut loc = Location::new(0, 0);

    let mut map = MapData::new(Dims::new(10, 1), |_| Terrain::Water);
    let unit_id = map
        .new_unit(
            loc,
            UnitType::Submarine,
            Alignment::Belligerent { player: 0 },
            "K-19",
        )
        .unwrap();

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::NEITHER);

    game.begin_turn(secrets[0], false).unwrap();

    for _ in 0..9 {
        game.move_unit_by_id_in_direction(secrets[0], unit_id, Direction::Right)
            .unwrap();
        loc = loc
            .shift_wrapped(Direction::Right, game.dims(), game.wrapping())
            .unwrap();

        let unit = game.current_player_toplevel_unit_by_loc(loc).unwrap();
        assert_eq!(unit.id, unit_id);

        game.force_end_then_begin_turn(secrets[0], secrets[0], false)
            .unwrap();
    }
}

#[test]
fn test_transport_moves_on_transport_unloaded() {
    let l1 = Location::new(0, 0);
    let l2 = Location::new(1, 0);

    let map = MapData::try_from("tt").unwrap();

    let t1_id = map.toplevel_unit_id_by_loc(l1).unwrap();

    {
        let (mut game, secrets) =
            Game::new_with_map(None, false, map.clone(), 1, false, None, Wrap2d::NEITHER);

        game.move_unit_by_id_in_direction(secrets[0], t1_id, Direction::Right)
            .expect_err("Transport should not be able to move onto transport");
    }

    let mut map2 = map.clone();
    map2.new_city(l2, Alignment::Belligerent { player: 0 }, "city")
        .unwrap();

    let (mut game, secrets) =
        Game::new_with_map(None, false, map2, 1, false, None, Wrap2d::NEITHER);

    game.move_unit_by_id_in_direction(secrets[0], t1_id, Direction::Right)
        .expect_err("Transport should not be able to move onto transport");
}

#[test]
fn test_transport_moves_on_transport_loaded() {
    let l1 = Location::new(1, 0);
    let l2 = Location::new(2, 0);

    let mut map = MapData::try_from(".tt.").unwrap();

    let t1_id = map.toplevel_unit_id_by_loc(l1).unwrap();
    let t2_id = map.toplevel_unit_id_by_loc(l2).unwrap();

    for i in 0..3 {
        println!("{}", i);
        let id = map
            .new_unit(
                Location::new(0, 0),
                UnitType::Infantry,
                Alignment::Belligerent { player: 0 },
                format!("inf{}", i),
            )
            .unwrap();
        map.carry_unit_by_id(t1_id, id).unwrap();
    }

    for i in 0..3 {
        let id = map
            .new_unit(
                Location::new(3, 0),
                UnitType::Infantry,
                Alignment::Belligerent { player: 0 },
                format!("inf{}", i + 100),
            )
            .unwrap();
        map.carry_unit_by_id(t2_id, id).unwrap();
    }

    {
        let (mut game, secrets) =
            Game::new_with_map(None, false, map.clone(), 1, false, None, Wrap2d::NEITHER);

        game.move_unit_by_id_in_direction(secrets[0], t1_id, Direction::Right)
            .expect_err("Transport should not be able to move onto transport");

        game.move_unit_by_id_in_direction(secrets[0], t2_id, Direction::Left)
            .expect_err("Transport should not be able to move onto transport");
    }

    let mut map2 = map.clone();
    map2.new_city(l2, Alignment::Belligerent { player: 0 }, "city")
        .unwrap();

    let (mut game, secrets) =
        Game::new_with_map(None, false, map2, 1, false, None, Wrap2d::NEITHER);

    game.move_unit_by_id_in_direction(secrets[0], t1_id, Direction::Right)
        .expect_err("Transport should not be able to move onto transport");
}

#[test]
fn test_embark_disembark() {
    let map = MapData::try_from("at -").unwrap();
    let armor_id = map.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
    let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::NEITHER);

    game.begin_turn(secrets[0], false).unwrap();

    // Embark
    game.move_unit_by_id_in_direction(secrets[0], armor_id, Direction::Right)
        .unwrap();
    assert_eq!(
        game.current_player_unit_loc(armor_id),
        Some(Location::new(1, 0))
    );
    assert_eq!(
        game.current_player_unit_loc(transport_id),
        Some(Location::new(1, 0))
    );

    // Move transport
    game.move_unit_by_id_in_direction(secrets[0], transport_id, Direction::Right)
        .unwrap();
    assert_eq!(
        game.current_player_unit_loc(armor_id),
        Some(Location::new(2, 0))
    );
    assert_eq!(
        game.current_player_unit_loc(transport_id),
        Some(Location::new(2, 0))
    );

    // Disembark
    game.move_unit_by_id_in_direction(secrets[0], armor_id, Direction::Right)
        .unwrap();
    assert_eq!(
        game.current_player_unit_loc(armor_id),
        Some(Location::new(3, 0))
    );
    assert_eq!(
        game.current_player_unit_loc(transport_id),
        Some(Location::new(2, 0))
    );
}

#[test]
fn test_embark_disembark_via_goto() {
    let map = MapData::try_from("at -").unwrap();
    let armor_id = map.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
    let transport_id = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();
    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::NEITHER);

    game.begin_turn(secrets[0], false).unwrap();

    // Embark
    game.order_unit_go_to(secrets[0], armor_id, Location::new(1, 0))
        .unwrap();
    assert_eq!(
        game.current_player_unit_loc(armor_id),
        Some(Location::new(1, 0))
    );
    assert_eq!(
        game.current_player_unit_loc(transport_id),
        Some(Location::new(1, 0))
    );

    // Move transport
    game.order_unit_go_to(secrets[0], transport_id, Location::new(2, 0))
        .unwrap();
    assert_eq!(
        game.current_player_unit_loc(armor_id),
        Some(Location::new(2, 0))
    );
    assert_eq!(
        game.current_player_unit_loc(transport_id),
        Some(Location::new(2, 0))
    );

    // Disembark
    game.order_unit_go_to(secrets[0], armor_id, Location::new(3, 0))
        .unwrap();
    assert_eq!(
        game.current_player_unit_loc(armor_id),
        Some(Location::new(3, 0))
    );
    assert_eq!(
        game.current_player_unit_loc(transport_id),
        Some(Location::new(2, 0))
    );
}

#[test]
fn test_shortest_paths_carrying() {
    let map = MapData::try_from("t t  ").unwrap();

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::NEITHER);

    game.move_toplevel_unit_by_loc(secrets[0], Location::new(0, 0), Location::new(4, 0))
        .expect_err("Transports shouldn't traverse transports on their way somewhere");
}

#[test]
fn test_valid_productions() {
    let map = MapData::try_from("...\n.0.\n...").unwrap();
    let (game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::NEITHER);

    let city_loc = game
        .current_player_production_set_requests()
        .next()
        .unwrap();

    let prods: BTreeSet<UnitType> = game
        .valid_productions(secrets[0], city_loc)
        .unwrap()
        .collect();

    for t in UnitType::values().iter().cloned() {
        if match t {
            UnitType::Armor => true,
            UnitType::Battleship => false,
            UnitType::Bomber => true,
            UnitType::Carrier => false,
            UnitType::Cruiser => false,
            UnitType::Destroyer => false,
            UnitType::Fighter => true,
            UnitType::Infantry => true,
            UnitType::Submarine => false,
            UnitType::Transport => false,
        } {
            assert!(prods.contains(&t));
        } else {
            assert!(!prods.contains(&t));
        }
    }
}

#[test]
fn test_valid_productions_conservative() {
    let map = MapData::try_from("...\n.0.\n...").unwrap();
    let (game, secrets) = Game::new_with_map(None, false, map, 1, false, None, Wrap2d::NEITHER);

    let city_loc = game
        .current_player_production_set_requests()
        .next()
        .unwrap();

    let prods: BTreeSet<UnitType> = game
        .valid_productions_conservative(secrets[0], city_loc)
        .unwrap()
        .collect();

    for t in UnitType::values().iter().cloned() {
        if match t {
            UnitType::Armor => true,
            UnitType::Battleship => false,
            UnitType::Bomber => true,
            UnitType::Carrier => false,
            UnitType::Cruiser => false,
            UnitType::Destroyer => false,
            UnitType::Fighter => true,
            UnitType::Infantry => true,
            UnitType::Submarine => false,
            UnitType::Transport => false,
        } {
            assert!(prods.contains(&t));
        } else {
            assert!(!prods.contains(&t));
        }
    }
}

#[test]
fn test_move_fighter_over_water() {
    let mut map = MapData::new(Dims::new(180, 90), |_| Terrain::Water);
    let unit_id = map
        .new_unit(
            Location::new(0, 0),
            UnitType::Fighter,
            Alignment::Belligerent { player: 0 },
            "Han Solo",
        )
        .unwrap();

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::BOTH);

    game.begin_turn(secrets[0], false).unwrap();

    let unit_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
    let dest = game
        .wrapping()
        .wrapped_add(game.dims(), unit_loc, Vec2d::new(5, 5))
        .unwrap();
    game.move_unit_by_id(secrets[0], unit_id, dest).unwrap();
}

#[test]
fn test_disband_unit_by_id() {
    {
        let map = MapData::try_from("i").unwrap();
        let (mut game, secrets) =
            Game::new_with_map(None, false, map, 1, true, None, Wrap2d::NEITHER);
        let id = UnitID::new(0);

        game.begin_turn(secrets[0], false).unwrap();

        let unit = game.current_player_unit_by_id(id).cloned().unwrap();

        assert!(game
            .current_player_unit_orders_requests()
            .any(|unit_id| unit_id == id));

        assert_eq!(
            game.disband_unit_by_id(secrets[0], id)
                .map(|disbanded| disbanded.unit),
            Ok(unit)
        );

        let id2 = UnitID::new(1);

        assert_eq!(
            game.disband_unit_by_id(secrets[0], id2),
            Err(GameError::NoSuchUnit { id: id2 })
        );

        assert!(!game
            .current_player_unit_orders_requests()
            .any(|unit_id| unit_id == id));
    }

    {
        let map2 = MapData::try_from("it ").unwrap();
        let infantry_id = map2.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
        let transport_id = map2.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();

        let (mut game2, secrets) =
            Game::new_with_map(None, false, map2, 1, true, None, Wrap2d::NEITHER);

        game2.begin_turn(secrets[0], false).unwrap();

        assert!(game2
            .current_player_unit_orders_requests()
            .find(|unit_id| *unit_id == infantry_id)
            .is_some());
        assert!(game2
            .current_player_unit_orders_requests()
            .find(|unit_id| *unit_id == transport_id)
            .is_some());

        game2
            .move_unit_by_id_in_direction(secrets[0], infantry_id, Direction::Right)
            .unwrap();

        game2
            .force_end_then_begin_turn(secrets[0], secrets[0], false)
            .unwrap();

        let infantry = game2
            .current_player_unit_by_id(infantry_id)
            .cloned()
            .unwrap();

        assert!(game2
            .current_player_unit_orders_requests()
            .find(|unit_id| *unit_id == infantry_id)
            .is_some());
        assert!(game2
            .current_player_unit_orders_requests()
            .find(|unit_id| *unit_id == transport_id)
            .is_some());

        let infantry_loc = infantry.loc;
        let prior_action_count = game2.player_action_count(secrets[0]).unwrap();

        assert_eq!(
            game2
                .disband_unit_by_id(secrets[0], infantry_id)
                .map(|disbanded| disbanded.unit),
            Ok(infantry)
        );

        match game2.current_player_obs(infantry_loc) {
            Obs::Observed { action_count, .. } => {
                assert_eq!(*action_count, prior_action_count + 1)
            }
            Obs::Unobserved => panic!(
                "The infantry's location prior to disbanding should be observed post-disband"
            ),
        }

        assert!(game2
            .current_player_unit_orders_requests()
            .find(|unit_id| *unit_id == infantry_id)
            .is_none());
        assert!(game2
            .current_player_unit_orders_requests()
            .find(|unit_id| *unit_id == transport_id)
            .is_some());

        let transport = game2
            .current_player_unit_by_id(transport_id)
            .cloned()
            .unwrap();

        assert_eq!(
            game2
                .disband_unit_by_id(secrets[0], transport_id)
                .map(|disbanded| disbanded.unit),
            Ok(transport)
        );

        assert!(game2
            .current_player_unit_orders_requests()
            .find(|unit_id| *unit_id == infantry_id)
            .is_none());
        assert!(game2
            .current_player_unit_orders_requests()
            .find(|unit_id| *unit_id == transport_id)
            .is_none());
    }

    {
        let map = MapData::try_from("ii").unwrap();
        let a = map.toplevel_unit_id_by_loc(Location::new(0, 0)).unwrap();
        let b = map.toplevel_unit_id_by_loc(Location::new(1, 0)).unwrap();

        let (mut game, secrets) =
            Game::new_with_map(None, false, map, 1, true, None, Wrap2d::NEITHER);

        game.begin_turn(secrets[0], false).unwrap();

        assert!(game.disband_unit_by_id(secrets[0], a).is_ok());

        assert!(game.current_player_unit_by_id(a).is_none());

        assert!(game
            .move_unit_by_id_in_direction(secrets[0], b, Direction::Left)
            .is_ok());
    }

    // Make sure a disbanded unit's absence is noted
    {
        let map2 = MapData::try_from("i").unwrap();
        let loc = Location::new(0, 0);
        let infantry_id = map2.toplevel_unit_id_by_loc(loc).unwrap();

        let (mut game, secrets) =
            Game::new_with_map(None, false, map2, 1, true, None, Wrap2d::NEITHER);

        game.begin_turn(secrets[0], false).unwrap();

        match game.player_obs(secrets[0], loc).unwrap() {
            Obs::Observed {
                turn, action_count, ..
            } => {
                assert_eq!(*turn, 0);
                assert_eq!(*action_count, 0);
            }
            Obs::Unobserved => panic!("Tile should be observed after turn start"),
        }

        assert_eq!(game.action_count, 0);

        let result = game.disband_unit_by_id(secrets[0], infantry_id);

        assert_eq!(game.action_count, 1);

        match result {
            Ok(disbanded) => match disbanded.obs.obs {
                Obs::Observed {
                    turn, action_count, ..
                } => {
                    assert_eq!(turn, 0);
                    assert_eq!(action_count, 1);
                }
                Obs::Unobserved => {
                    panic!("Tile should not be unobserved after disbanding a unit there")
                }
            },
            Err(e) => panic!("Could not disband unit; error {}", e),
        }

        // Make sure not just the returned obs, but the stored as well is updated
        let obs = game.player_obs(secrets[0], loc).unwrap();

        match obs {
            Obs::Observed {
                turn, action_count, ..
            } => {
                assert_eq!(*turn, 0);
                assert_eq!(*action_count, 1);
            }
            Obs::Unobserved => {
                panic!("Tile should not be unobserved after disbanding a unit there")
            }
        }
    }
}

#[test]
pub fn test_turn_is_done() {
    let map = MapData::try_from("0 ").unwrap();
    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::NEITHER);

    assert_eq!(
        game.turn_is_done(9999, 0),
        Err(GameError::NoSuchPlayer { player: 9999 })
    );

    assert_eq!(game.turn_is_done(0, 0), Ok(false));

    assert_eq!(game.turn_is_done(0, 1), Ok(false));

    game.begin_turn(secrets[0], false).unwrap();

    assert_eq!(game.turn_is_done(0, 0), Ok(false));

    assert_eq!(game.turn_is_done(0, 1), Ok(false));

    let city0id = game
        .player_city_by_loc(secrets[0], Location::new(0, 0))
        .unwrap()
        .unwrap()
        .id;

    game.set_production_by_id(secrets[0], city0id, UnitType::Infantry)
        .unwrap();

    assert_eq!(game.turn_is_done(0, 0), Ok(true));

    assert_eq!(game.turn_is_done(0, 1), Ok(false));

    // Let the infantry be produced
    for i in 0..UnitType::Infantry.cost() {
        game.end_turn(secrets[0]).unwrap();

        let turn = i as TurnNum + 1;

        assert!(!game.turn_is_done(0, turn).unwrap()); // not done because still in Pre phase
        assert!(!game.current_turn_is_done()); // not done because still in Pre phase

        game.begin_turn(secrets[0], false).unwrap();

        if i < UnitType::Infantry.cost() - 1 {
            assert!(game.turn_is_done(0, turn).unwrap()); // done because there's nothing to do
            assert!(game.current_turn_is_done()); // done because there's nothing to do
        } else {
            assert!(!game.turn_is_done(0, turn).unwrap()); // not done because we just produced an infantry
            assert!(!game.current_turn_is_done()); // not done because we just produced an infantry
        }
    }
}

#[test]
pub fn test_fuel_limit() {
    let map = MapData::try_from("f                    ").unwrap();

    let start = Location::new(0, 0);

    let id = {
        let tile = map.tile(start).unwrap();
        let unit = tile.unit.as_ref().unwrap();
        assert_eq!(unit.alignment, Alignment::Belligerent { player: 0 });
        unit.id
    };

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::NEITHER);

    for turn in 0..4 {
        game.begin_turn(secrets[0], false).unwrap();

        for step in 0..UnitType::Fighter.movement_per_turn() {
            let move_ = game
                .move_unit_by_id_in_direction(secrets[0], id, Direction::Right)
                .unwrap();

            if turn == 3 && step == UnitType::Fighter.movement_per_turn() - 1 {
                assert!(move_.fuel_ran_out());
                assert_eq!(game.player_unit_by_id(secrets[0], id), Ok(None));
            } else {
                assert!(!move_.fuel_ran_out(), "turn {} step {}", turn, step);
            }
        }

        game.end_turn(secrets[0]).unwrap();
    }
}

#[test]
pub fn test_refuel_in_carrier() {
    let map = MapData::try_from("b k").unwrap();

    let id = map.toplevel_unit_by_loc(Location::new(0, 0)).unwrap().id;

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::NEITHER);

    game.begin_turn(secrets[0], false).unwrap();

    game.move_unit_by_id_in_direction(secrets[0], id, Direction::Right)
        .unwrap();

    let max = match UnitType::Bomber.fuel() {
        Fuel::Limited { max, .. } => max,
        _ => panic!(),
    };

    assert_eq!(
        game.player_unit_by_id(secrets[0], id)
            .unwrap()
            .unwrap()
            .fuel,
        Fuel::Limited {
            max,
            remaining: max - 1
        }
    );

    game.move_unit_by_id_in_direction(secrets[0], id, Direction::Right)
        .unwrap();

    assert_eq!(
        game.player_unit_by_id(secrets[0], id)
            .unwrap()
            .unwrap()
            .fuel,
        Fuel::Limited {
            max,
            remaining: max - 2
        }
    );

    game.force_end_turn(secrets[0]).unwrap();

    assert_eq!(
        game.player_unit_by_id(secrets[0], id)
            .unwrap()
            .unwrap()
            .fuel,
        Fuel::Limited {
            max,
            remaining: max
        }
    );
}

#[test]
pub fn test_refuel_in_city() {
    let map = MapData::try_from("f 0").unwrap();

    let id = map.toplevel_unit_by_loc(Location::new(0, 0)).unwrap().id;

    let (mut game, secrets) = Game::new_with_map(None, false, map, 1, true, None, Wrap2d::NEITHER);

    game.begin_turn(secrets[0], false).unwrap();

    game.move_unit_by_id_in_direction(secrets[0], id, Direction::Right)
        .unwrap();

    let max = match UnitType::Fighter.fuel() {
        Fuel::Limited { max, .. } => max,
        _ => panic!(),
    };

    assert_eq!(
        game.player_unit_by_id(secrets[0], id)
            .unwrap()
            .unwrap()
            .fuel,
        Fuel::Limited {
            max,
            remaining: max - 1
        }
    );

    game.move_unit_by_id_in_direction(secrets[0], id, Direction::Right)
        .unwrap();

    assert_eq!(
        game.player_unit_by_id(secrets[0], id)
            .unwrap()
            .unwrap()
            .fuel,
        Fuel::Limited {
            max,
            remaining: max - 2
        }
    );

    game.force_end_turn(secrets[0]).unwrap();

    assert_eq!(
        game.player_unit_by_id(secrets[0], id)
            .unwrap()
            .unwrap()
            .fuel,
        Fuel::Limited {
            max,
            remaining: max
        }
    );
}

#[test]
pub fn test_player_feature_playernum_invariance() {
    // Make sure the feature vector only considers friendly/enemy relations, not
    // player number itself

    // For units
    {
        let (mut game0, secrets0) = Game::new_from_string(None, false, "i   I").unwrap();

        let (mut game1, secrets1) = Game::new_from_string(None, false, "I   i").unwrap();

        let loc0 = Location::new(0, 0);
        let loc1 = Location::new(4, 0);

        game0.begin_turn(secrets0[0], false).unwrap();

        game1.begin_turn(secrets1[0], false).unwrap();

        game0.force_end_turn(secrets0[0]).unwrap();

        game1.force_end_turn(secrets1[0]).unwrap();

        game0.begin_turn(secrets0[1], false).unwrap();

        game1.begin_turn(secrets1[1], false).unwrap();

        assert_eq!(
            game0
                .player_toplevel_unit_by_loc(secrets0[0], loc0)
                .unwrap()
                .unwrap()
                .alignment,
            game1
                .player_toplevel_unit_by_loc(secrets1[0], loc1)
                .unwrap()
                .unwrap()
                .alignment,
            "Units 0 have same alignment"
        );

        assert_eq!(
            game0
                .player_toplevel_unit_by_loc(secrets0[1], loc1)
                .unwrap()
                .unwrap()
                .alignment,
            game1
                .player_toplevel_unit_by_loc(secrets1[1], loc0)
                .unwrap()
                .unwrap()
                .alignment,
            "Units 1 have same alignment"
        );

        for player in 0..=1 {
            assert_eq!(
                game0
                    .player_observations(secrets0[player])
                    .unwrap()
                    .num_observed(),
                game1
                    .player_observations(secrets1[player])
                    .unwrap()
                    .num_observed(),
                "Player {} obs counts mismatch",
                player
            );
        }

        // These counts should be the same because the two units occupy identical positions in their game
        assert_eq!(
            game0
                .player_observations(secrets0[0])
                .unwrap()
                .num_observed(),
            game1
                .player_observations(secrets1[1])
                .unwrap()
                .num_observed(),
            "Game 0 player 0 and game 1 player 1 obs count mismatch"
        );

        let v0 = game0
            .player_features(secrets0[0], TrainingFocus::Unit)
            .unwrap();

        let v1 = game1
            .player_features(secrets1[1], TrainingFocus::Unit)
            .unwrap();

        assert_eq!(v0, v1);
    }

    // // For cities
    // {
    //     let game2 = Game::try_from("0   1").unwrap();
    //     let game3 = Game::try_from("1   0").unwrap();
    // }
}
