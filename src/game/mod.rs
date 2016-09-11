//!
//! Abstract game engine.
//!
//! This implements the game logic without regard for user interface.

use std::collections::HashSet;
use std::ops::{Index,IndexMut};

use map::Tile;
use map::gen::generate_map;
use unit::{Alignment,PlayerNum,Unit,UnitType};
use util::{Dims,Location};

pub type TurnNum = u32;

/// What a particular player knows about a tile
#[derive(Clone)]
enum Obs {
    OBSERVED{tile: Tile, turn: TurnNum},
    UNOBSERVED
}

pub trait ProductionSetter {
    fn set_production(&self, game: &Game) -> UnitType;
}

struct ProductionSetRequest {

}

struct UnitMoveRequest {
    location: Location
}
impl UnitMoveRequest {
    pub fn move_unit(&self, new_location: Location) -> Option<UnitMoveRequest> {
        None
    }
}

struct TurnDecisions {

}

pub type Tiles = Vec<Vec<Tile>>;
impl Index<Location> for Tiles {
    type Output = Tile;
    fn index<'a>(&'a self, location: Location) -> &'a Tile {
        &self[location.x as usize][location.y as usize]
    }
}
impl IndexMut<Location> for Tiles {
    fn index_mut<'a>(&'a mut self, location: Location) -> &'a mut Tile {


        let col:  &mut Vec<Tile> = self.get_mut(location.x as usize).unwrap();
        col.get_mut(location.y as usize).unwrap()

        // &mut self[location.x as usize][location.y as usize]
    }
}


// struct PlayerTurn<'a> {
//     production_set_requests: HashSet<Location>,
//     unit_move_requests: HashSet<Location>
// }
//
// impl<'b> PlayerTurn<'b> {
//
//
//
// }

// pub trait Decider {
//     fn make_turn_decisions(&self, game: &Game, productions_to_set: Vec<Location>, units_to_move: )
// }

pub struct Game {
    pub map_dims: Dims,
    pub tiles: Tiles, // tiles[col][row]
    // player_maps: HashMap<PlayerNum,Vec<Vec<Obs>>>,
    pub turn: TurnNum,
    num_players: PlayerNum,
    next_player: PlayerNum,
    production_set_requests: HashSet<Location>,
    unit_move_requests: HashSet<Location>
}

impl Game {
    pub fn new(map_dims: Dims, num_players: PlayerNum) -> Self {
        // let mut player_map = Vec::new();
        // for x in 0..map_dims.width {
        //     let mut col = Vec::new();
        //     for y in 0..map_dims.height {
        //         col.push(Obs::UNOBSERVED);
        //     }
        //     player_map.push(col);
        // }
        //
        // let mut player_maps = HashMap::new();
        // for player in 0..num_players {
        //     if player == conf::HUMAN_PLAYER {
        //         player_maps.insert(player, player_map.clone());
        //     } else {
        //         player_maps.insert(player, player_map.clone());
        //     }
        // }

        Game {
            map_dims: map_dims,
            tiles: generate_map(map_dims),
            // player_maps: player_maps,
            turn: 0,
            num_players: num_players,
            next_player: 0,
            production_set_requests: HashSet::new(),
            unit_move_requests: HashSet::new()
        }
    }

    // pub fn next_player_turn(&mut self) -> (PlayerNum,PlayerTurn) {
    //
    //     let player = self.next_player;
    //     self.next_player = (self.next_player + 1) % self.num_players;
    //     // self.next_player += 1;
    //     // if self.next_player > self.num_players - 1 {
    //     //     self.next_player = 0;
    //     //     self.turn += 1;
    //     // }
    //
    //     let player_turn = self.begin_player_turn(player);
    //
    //
    //
    //
    //
    //     (player, player_turn)
    // }

    /// Returns the number of the player whose turn has just begun
    pub fn begin_next_player_turn(&mut self) -> PlayerNum {
        let player = self.next_player;
        self.next_player = (self.next_player + 1) % self.num_players;
        self.begin_player_turn(player);
        player
    }

    fn begin_player_turn(&mut self, player_num: PlayerNum) {
        for x in 0..self.map_dims.width {
            for y in 0..self.map_dims.height {
                let loc = Location{x:x, y:y};
                let tile: &mut Tile = &mut self.tiles[loc];
                
                if let Some(ref mut city) = tile.city {
                    if let Alignment::BELLIGERENT{player} = city.alignment {
                        if player==player_num {

                            if let Some(ref unit_under_production) = city.unit_under_production {
                                city.production_progress += 1;
                                if city.production_progress >= unit_under_production.cost() {
                                    let new_unit = Unit::new(*unit_under_production, city.alignment, loc);
                                    tile.unit = Some(new_unit);
                                    city.production_progress = 0;
                                }
                            } else {
                                self.production_set_requests.insert(loc);
                            }
                        }
                    }
                }

                if let Some(ref mut unit) = tile.unit {
                    unit.moves_remaining += unit.movement_per_turn();
                    if !unit.sentry {
                        self.unit_move_requests.insert(loc);
                    }
                }
            }
        }
    }

    // fn player_map(&self, player: PlayerNum) -> Option<&Vec<Vec<Obs>>> {
    //     self.player_maps.get(&player)
    // }


    pub fn production_set_requests(&self) -> &HashSet<Location> {
        &self.production_set_requests
    }

    pub fn unit_move_requests(&self) -> &HashSet<Location> {
        &self.unit_move_requests
    }

    fn request_unit_move(&mut self, location: Location) {
        self.unit_move_requests.insert(location);
    }

    pub fn move_unit(&mut self, src: Location, dest: Location) -> Result<(),()> {
        let distance = src.distance(&dest);

        let mut unit: Unit = self.tiles[src].pop_unit().unwrap();

        if distance > unit.moves_remaining {
            return Err(());
        }

        {
            let mut unit = &mut unit;

            self.unit_move_requests.remove(&src);
            unit.moves_remaining -= distance;


            if unit.moves_remaining > 0 {
                self.unit_move_requests.insert(dest);
            }
        }

        self.tiles[dest].set_unit(unit);

        Ok(())
    }

    fn request_set_production(&mut self, location: Location) {
        self.production_set_requests.insert(location);
    }

    pub fn set_production(&mut self, location: &Location, production: &UnitType) -> Result<(),()> {
        match self.tiles[*location].city {
            Some(ref mut city) => {
                city.unit_under_production = Some(*production)
            },
            None => {
                return Err(());
            }
        }

        self.production_set_requests.remove(location);

        Ok(())
    }
}
