//! Test support functions

use std::sync::{Arc, RwLock};

use crate::{
    game::{
        error::GameError,
        map::{MapData, Terrain},
        obs::Obs,
        unit::{UnitID, UnitType},
        Alignment, Game,
    },
    name::unit_namer,
    util::{Dims, Location, Wrap2d},
};

use super::PlayerSecret;

pub fn test_propose_move_unit_by_id() {
    let src = Location { x: 0, y: 0 };
    let dest = Location { x: 1, y: 0 };

    let (game, secrets) = game_two_cities_two_infantry();

    let unit_id: UnitID = game.current_player_unit_orders_requests().next().unwrap();

    {
        let unit = game.current_player_unit_by_id(unit_id).unwrap();
        assert_eq!(unit.loc, src);
    }

    let proposed_move = game
        .propose_move_unit_by_id(secrets[0], unit_id, dest)
        .unwrap()
        .outcome;

    let component = proposed_move.components.first().unwrap();

    // Make sure the intended destination is now observed as containing this unit, and that no other observed tiles
    // are observed as containing it
    for located_obs in &component.observations_after_move {
        match located_obs.obs {
            Obs::Observed {
                ref tile,
                turn,
                action_count: _,
                current,
            } => {
                if located_obs.loc == dest {
                    let unit = tile.unit.as_ref().unwrap();
                    assert_eq!(unit.id, unit_id);
                    assert_eq!(turn, 6);
                    assert!(current);
                } else if let Some(unit) = tile.unit.as_ref() {
                    assert_ne!(unit.id, unit_id);
                }
            }
            Obs::Unobserved => panic!("This should be observed"),
        }
    }
}

/// 10x10 grid of land only with two cities:
/// * Player 0's Machang at 0,0
/// * Player 1's Zanzibar at 0,1
fn map_two_cities(dims: Dims) -> MapData {
    let mut map = MapData::new(dims, |_loc| Terrain::Land);
    map.new_city(
        Location { x: 0, y: 0 },
        Alignment::Belligerent { player: 0 },
        "Machang",
    )
    .unwrap();
    map.new_city(
        Location { x: 0, y: 1 },
        Alignment::Belligerent { player: 1 },
        "Zanzibar",
    )
    .unwrap();
    map
}

pub fn game1() -> (Game, Vec<PlayerSecret>) {
    let players = 2;
    let fog_of_war = true;

    let map = map_two_cities(Dims::new(10, 10));
    let unit_namer = unit_namer(None);
    Game::new_with_map(
        None,
        false,
        map,
        players,
        fog_of_war,
        Some(Arc::new(RwLock::new(unit_namer))),
        Wrap2d::BOTH,
    )
}

pub fn game_two_cities_dims(dims: Dims) -> (Game, Vec<PlayerSecret>) {
    let players = 2;
    let fog_of_war = true;

    let map = map_two_cities(dims);
    let unit_namer = unit_namer(None);
    let (mut game, secrets) = Game::new_with_map(
        None,
        false,
        map,
        players,
        fog_of_war,
        Some(Arc::new(RwLock::new(unit_namer))),
        Wrap2d::BOTH,
    );

    game.begin_turn(secrets[0], false).unwrap();

    let loc: Location = game
        .current_player_production_set_requests()
        .next()
        .unwrap();

    // println!("Setting production at {:?} to infantry", loc);
    game.set_production_by_loc(secrets[0], loc, UnitType::Infantry)
        .unwrap();

    let player = game
        .end_then_begin_turn(secrets[0], secrets[1], false)
        .unwrap()
        .current_player;
    assert_eq!(player, 1);

    let loc: Location = game
        .current_player_production_set_requests()
        .next()
        .unwrap();
    // println!("Setting production at {:?} to infantry", loc);
    game.set_production_by_loc(secrets[1], loc, UnitType::Infantry)
        .unwrap();

    let player = game
        .end_then_begin_turn(secrets[1], secrets[0], false)
        .unwrap()
        .current_player;
    assert_eq!(player, 0);

    (game, secrets)
}

fn map_tunnel(dims: Dims) -> MapData {
    let mut map = MapData::new(dims, |_loc| Terrain::Land);
    map.new_city(
        Location::new(0, dims.height / 2),
        Alignment::Belligerent { player: 0 },
        "City 0",
    )
    .unwrap();
    map.new_city(
        Location::new(dims.width - 1, dims.height / 2),
        Alignment::Belligerent { player: 1 },
        "City 1",
    )
    .unwrap();
    map
}

pub fn game_tunnel(dims: Dims) -> (Game, Vec<PlayerSecret>) {
    let players = 2;
    let fog_of_war = false;
    let map = map_tunnel(dims);
    let unit_namer = unit_namer(None);
    Game::new_with_map(
        None,
        false,
        map,
        players,
        fog_of_war,
        Some(Arc::new(RwLock::new(unit_namer))),
        Wrap2d::NEITHER,
    )
}

// pub(crate) fn game_two_cities() -> Game {
//     game_two_cities_dims(Dims::new(10, 10))
// }

// pub(crate) fn game_two_cities_big() -> Game {
//     game_two_cities_dims(Dims::new(100, 100))
// }

pub fn game_two_cities_two_infantry_dims(dims: Dims) -> (Game, Vec<PlayerSecret>) {
    let (mut game, secrets) = game_two_cities_dims(dims);

    for _ in 0..5 {
        let player = game
            .end_then_begin_turn(secrets[0], secrets[1], false)
            .unwrap()
            .current_player;
        assert_eq!(player, 1);
        let player = game
            .end_then_begin_turn(secrets[1], secrets[0], false)
            .unwrap()
            .current_player;
        assert_eq!(player, 0);
    }

    assert_eq!(
        game.end_then_begin_turn(secrets[0], secrets[1], false),
        Err(GameError::TurnEndRequirementsNotMet { player: 0 })
    );
    assert_eq!(
        game.end_then_begin_turn(secrets[0], secrets[1], false),
        Err(GameError::TurnEndRequirementsNotMet { player: 0 })
    );

    (game, secrets)
}

pub fn game_two_cities_two_infantry() -> (Game, Vec<PlayerSecret>) {
    game_two_cities_two_infantry_dims(Dims::new(10, 10))
}

pub fn game_two_cities_two_infantry_big() -> (Game, Vec<PlayerSecret>) {
    game_two_cities_two_infantry_dims(Dims::new(100, 100))
}
