use failure::{Fail};

use crate::{
    game::{
        Game,
        Move,
        MoveComponent,
        MoveError,
        map::{
            dijkstra::{
                ObservedFilter,
                UnitMovementFilter,
                Xenophile,
                nearest_adjacent_unobserved_reachable_without_attacking,
                shortest_paths
            },
        },
        unit::UnitID,
    },
    util::{
        Dims,
        Location,
    },
};



#[derive(Copy,Clone,Debug,PartialEq)]
pub enum OrdersStatus {
    InProgress,
    Completed
}

// pub enum OrdersResult {
//     Skip,
//     Sentry,
//     GoTo {
//         move_: MoveResult,
//         status: OrdersStatus,
//     },
//     Explore {
//         move_: MoveResult,

//     }
// }

#[derive(Debug,PartialEq)]
pub struct OrdersOutcome {
    pub move_result: Option<Move>,
    pub status: OrdersStatus,
}
impl OrdersOutcome {
    pub fn completed_without_move() -> Self {
        Self { move_result: None, status: OrdersStatus::Completed }
    }

    pub fn in_progress_without_move() -> Self {
        Self { move_result: None, status: OrdersStatus::InProgress }
    }

    pub fn in_progress_with_move(move_result: Move) -> Self {
        Self { move_result: Some(move_result), status: OrdersStatus::InProgress }
    }

    pub fn completed_with_move(move_result: Move) -> Self {
        Self { move_result: Some(move_result), status: OrdersStatus::Completed }
    }

    pub fn move_result(&self) -> Option<&Move> {
        self.move_result.as_ref()
    }

    pub fn status(&self) -> OrdersStatus  {
        self.status
    }
}

#[derive(Debug,Fail,PartialEq)]
pub enum OrdersError {
    #[fail(display="Ordered unit with ID {:?} doesn't exist", id)]
    OrderedUnitDoesNotExist {
        id: UnitID,
    },

    #[fail(display="Cannot order unit with ID {:?} to go to {} because the destination is out of the bounds {}", id, dest, map_dims)]
    CannotGoToOutOfBounds {
        id: UnitID,
        dest: Location,
        map_dims: Dims,
    },

    #[fail(display="Orders failed due to a problem moving the unit: {:?}", 0)]
    MoveError(MoveError),
}

// type GoToResult = Result<(MoveResult,OrdersStatus),String>;

// type ExploreResult = Result<(MoveResult,OrdersStatus),String>;

// type OrdersResult = Result<OrdersStatus,String>;
pub type OrdersResult = Result<OrdersOutcome,OrdersError>;


#[derive(Copy,Clone,Debug,PartialEq)]
pub enum Orders {
    Skip,
    Sentry,
    GoTo{dest:Location},
    Explore
}

impl Orders {
    pub fn carry_out(self, unit_id: UnitID, game: &mut Game) -> OrdersResult {
        match self {
            Orders::Skip => {
                game.set_orders(unit_id, None).map(|_| OrdersOutcome::completed_without_move())
            },
            Orders::Sentry => {
                // do nothing---sentry is implemented as a reaction to approaching enemies
                Ok(OrdersOutcome::in_progress_without_move())

            },
            Orders::GoTo{dest} => {
                go_to(game, unit_id, dest)
            },
            Orders::Explore => {
                explore(game, unit_id)
            }
        }
    }

    /// A present-tense, progressive aspect verb phrase describing the action of the unit as it carries out these orders
    /// Example: "standing sentry" for a sentry unit.
    pub fn present_progressive_description(self) -> String {
        match self {
            Orders::Skip => {
                String::from("skipping its turn")
            },
            Orders::Sentry => {
                String::from("standing sentry")
            },
            Orders::GoTo{dest} => {
                format!("going to {}", dest)
            },
            Orders::Explore => {
                String::from("exploring")
            }
        }
    }
}

/// Keep moving toward the nearest unobserved tile we can see a path
/// to, until either there is no such tile or we run out of moves
/// If there are no such tiles then set the unit's orders to None
///
///

/*


www
wxw
www


*/
pub fn explore(game: &mut Game, unit_id: UnitID) -> OrdersResult {
    // // Shortest paths emanating from the starting location, considering only observed tiles
    // let shortest_paths_observed = shortest_paths_unit_limited(game, starting_loc,
    //                                             game.unit(starting_loc).unwrap(), game.wrapping());
    //
    // // Shortest paths emanating from the starting location, allowing inclusion of unobserved tiles.
    // let shortest_paths_xenophile = shortest_paths_unit_limited_xenophile(game, starting_loc,
    //                                             game.unit(starting_loc).unwrap(), game.wrapping());
    //
    //
    //
    let mut current_loc = game.unit_by_id(unit_id).unwrap().loc;
    let starting_loc = current_loc;
    let mut moves: Vec<MoveComponent> = Vec::new();
    // let mut unit = None;
    loop {
        // Get a fresh copy of the unit
        let unit = game.unit_by_id_mut(unit_id).expect("Somehow the unit disappeared during exploration").clone();

        if unit.moves_remaining() == 0 {
            return Ok(OrdersOutcome::in_progress_with_move(Move::new(unit, starting_loc, moves).unwrap()));
        }

        if let Some(mut goal) = nearest_adjacent_unobserved_reachable_without_attacking(game, current_loc, &unit, game.wrapping()) {

            // if unit.moves_remaining == 0 {
            //     return Ok(OrdersStatus::InProgress);
            // }

            let shortest_paths = shortest_paths(game, unit.loc, &ObservedFilter{}, game.wrapping());

            let mut dist_to_real_goal = shortest_paths.dist[goal].unwrap();
            while dist_to_real_goal > unit.moves_remaining() {
                goal = shortest_paths.prev[goal].unwrap();
                dist_to_real_goal -= 1;
            }

            let mut move_result = game.move_unit_by_id_avoiding_combat(unit_id, goal)
                                  .map_err(OrdersError::MoveError)?;

            // match move_result {
            //     Ok(mut move_result) => {
                    // ui.animate_move(game, &move_result);

                    if move_result.moved_successfully() {
                        current_loc = move_result.ending_loc().unwrap();
                        moves.append(&mut move_result.moves);
                    } else {
                        panic!("Unit was unexpectedly destroyed during exploration");
                    }

                    // Update the unit so that if/when we return it, it has the correct number of moves
                    // unit.moves_remaining -= move_result.moves.len();
                    

                    // unit.loc = move_result.ending_loc().unwrap();
                    // unit.record_movement(move_result.moves.len() as u16).unwrap();
            //     },
            //     Err(err) => {
            //         return Err(format!("Error moving unit toward {}: {}", goal, msg));
            //     }
            // }

        } else {
            // game.give_orders(unit_id, None, ui, false).unwrap();
            return game.set_orders(unit_id, None)
                .map(|_| OrdersOutcome::completed_with_move(
                    Move::new(unit, starting_loc, moves).unwrap()
                )
            );
            // return Ok(OrdersStatus::Completed);
        }
    }
}

/// Analysis of potential destinations:
/// Observed? | Accessible by Known Route? | Outcome
/// No        | No                         | Go to observed, accessible tile nearest the target
/// No        | Yes*                       | This doesn't exist; we don't know there's a route
///                                          there---it could be a mountain range or something.
/// Yes       | No                         | I.e. tile on different island. Go to observed,
///                                          accessible tile nearest the target.
/// Yes       | Yes                        | Take the known route to the target.
///
/// So, in all cases, the right thing to do is to go to the observed, accessible tile nearest the
/// target, going there by way of the shortest route we know of. Once we're there, clear the unit's
/// orders.
pub fn go_to(game: &mut Game, unit_id: UnitID, dest: Location) -> OrdersResult {
    // ui.log_message(format!("Destination 1: {}", dest));
    // let moves_remaining = {
    //     game.unit_by_id(unit_id).unwrap().moves_remaining
    // };
    //
    // ui.log_message(format!("Destination 2: {}", dest));


    let (moves_remaining, shortest_paths) = {
        let unit = game.unit_by_id(unit_id).unwrap();
        let moves_remaining = unit.moves_remaining;

        // Shortest paths emanating from the unit's location, allowing inclusion of unobserved tiles.
        let shortest_paths = shortest_paths(
            game,
            unit.loc,
            &Xenophile::new(UnitMovementFilter::new(unit)),
            game.wrapping());

        (moves_remaining, shortest_paths)
    };

    // ui.log_message(format!("Destination 3: {}", dest));
    // Find the observed tile on the path from source to destination that is nearest to the
    // destination but also within reach of this unit's limited moves
    let mut dest = dest;
    loop {
        if game.current_player_tile(dest).is_some() {
            if let Some(dist) = shortest_paths.dist[dest] {
                if dist <= moves_remaining {
                    break;
                }
            }
        }
        // if game.current_player_tile(dest).is_some() && shortest_paths.dist[dest].unwrap() <= moves_remaining {
        //     break;
        // }
        dest = shortest_paths.prev[dest].unwrap();
    }
    let dest = dest;

    // ui.log_message(format!("Destination 4: {}", dest));
    //
    // let dest = {
    //     let unit = game.unit(src).unwrap();
    //     nearest_reachable_adjacent_unobserved(game, src, &unit, game.wrapping())
    // };

    game.move_unit_by_id(unit_id, dest)
        .map(|move_result| {
            // ui.animate_move(game, &move_result);

            let status = if move_result.moved_successfully() && move_result.unit().moves_remaining > 0 {
                // game.give_orders(unit_id, None, ui, false).unwrap();
                game.set_orders(unit_id, None).unwrap();
                // Ok(OrdersStatus::Completed)
                OrdersStatus::Completed
                
            } else {
                // Ok(OrdersStatus::InProgress)
                OrdersStatus::InProgress
            };
            OrdersOutcome {
                move_result: Some(move_result),
                status
            }
        })
        .map_err(OrdersError::MoveError)
}

#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    use crate::{
        game::{
            Game,
            MoveError,
            map::MapData,
            unit::orders::OrdersError,
        },
        log::DefaultLog,
        name::unit_namer,
        util::Location,
    };

    use super::OrdersStatus;

    #[test]
    fn test_go_to() {
        let mut log = DefaultLog;
        let map = MapData::try_from("i----------").unwrap();
        let mut game = Game::new_with_map(map, 1, false, unit_namer(), &mut log);
        
        let id = game.toplevel_unit_by_loc(Location{x:0,y:0}).unwrap().id;
        let result1 = game.order_unit_go_to(id, Location{x:0,y:0});
        assert_eq!(result1, Err(OrdersError::MoveError(MoveError::ZeroLengthMove)));

        let result2 = game.order_unit_go_to(id, Location{x:5, y:0});
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap().status, OrdersStatus::InProgress);

        // Wait while the go-to order is carried out
        while game.unit_orders_requests().next().is_none() {
            eprintln!("Ending turn");
            assert_eq!(game.end_turn(&mut log).unwrap(), 0);
        }

        assert_eq!(game.turn(), 9);
    }
}