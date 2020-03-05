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

/// Test support functions
#[cfg(test)]
mod test {
    use crate::{
        game::{
            Alignment,
            Game,
            map::{
                MapData,
                Terrain,
            },
            unit::UnitType,
        },
        name::unit_namer,
        util::{
            Dims,
            Location,
            Wrap2d,
        },
    };

    /// 10x10 grid of land only with two cities:
    /// * Player 0's Machang at 0,0
    /// * Player 1's Zanzibar at 0,1
    fn map1() -> MapData {
        let dims = Dims{width: 10, height: 10};
        let mut map = MapData::new(dims, |_loc| Terrain::Land);
        map.new_city(Location{x:0,y:0}, Alignment::Belligerent{player:0}, "Machang").unwrap();
        map.new_city(Location{x:0,y:1}, Alignment::Belligerent{player:1}, "Zanzibar").unwrap();
        // LocationGrid::new(dims, |loc| {
        //     let mut tile = Tile::new(Terrain::Land, loc);
        //     if loc.x == 0 {
        //         if loc.y == 0 {
        //             tile.city = Some(City::new(Alignment::Belligerent{player:0}, loc, "Machang"));
        //         } else if loc.y == 1 {
        //             tile.city = Some(City::new(Alignment::Belligerent{player:1}, loc, "Zanzibar"));
        //         }
        //     }
        //     tile
        // })
        map
    }

    pub(crate) fn game1() -> Game {
        let players = 2;
        let fog_of_war = true;
 
        let map = map1();
        let unit_namer = unit_namer();
        Game::new_with_map(map, players, fog_of_war, Box::new(unit_namer), Wrap2d::BOTH)
    }

    pub(crate) fn game_two_cities() -> Game {
        let players = 2;
        let fog_of_war = true;
 
        let map = map1();
        let unit_namer = unit_namer();
        let mut game = Game::new_with_map(map, players, fog_of_war, Box::new(unit_namer), Wrap2d::BOTH);

        let loc: Location = game.production_set_requests().next().unwrap();

        println!("Setting production at {:?} to infantry", loc);
        game.set_production(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn().unwrap().current_player;
        assert_eq!(player, 1);

        let loc: Location = game.production_set_requests().next().unwrap();
        println!("Setting production at {:?} to infantry", loc);
        game.set_production(loc, UnitType::Infantry).unwrap();

        let player = game.end_turn().unwrap().current_player;
        assert_eq!(player, 0);

        game
    }

    pub(crate) fn game_two_cities_two_infantry() -> Game {
        let mut game = game_two_cities();

        for _ in 0..5 {
            let player = game.end_turn().unwrap().current_player;
            assert_eq!(player, 1);
            let player = game.end_turn().unwrap().current_player;
            assert_eq!(player, 0);
        }

        assert_eq!(game.end_turn(), Err(0));
        assert_eq!(game.end_turn(), Err(0));

        game
    }
}