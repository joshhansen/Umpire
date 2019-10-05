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
        Location,
    },
};

#[derive(Copy,Clone,Debug,PartialEq)]
pub enum OrdersStatus {
    InProgress,
    Completed
}

#[derive(Debug,PartialEq)]
pub struct OrdersOutcome {
    pub ordered_unit_id: UnitID,
    pub orders: Orders,
    pub move_result: Option<Move>,
    pub status: OrdersStatus,
}
impl OrdersOutcome {
    pub fn completed_without_move(ordered_unit_id: UnitID, orders: Orders) -> Self {
        Self { ordered_unit_id, orders, move_result: None, status: OrdersStatus::Completed }
    }

    pub fn in_progress_without_move(ordered_unit_id: UnitID, orders: Orders) -> Self {
        Self { ordered_unit_id, orders, move_result: None, status: OrdersStatus::InProgress }
    }

    pub fn in_progress_with_move(ordered_unit_id: UnitID, orders: Orders, move_result: Move) -> Self {
        Self { ordered_unit_id, orders, move_result: Some(move_result), status: OrdersStatus::InProgress }
    }

    pub fn completed_with_move(ordered_unit_id: UnitID, orders: Orders, move_result: Move) -> Self {
        Self { ordered_unit_id, orders, move_result: Some(move_result), status: OrdersStatus::Completed }
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
        orders: Orders,
    },

    // #[fail(display="Cannot order unit with ID {:?} to go to {} because the destination is out of the bounds {}", id, dest, map_dims)]
    // CannotGoToOutOfBounds {
    //     id: UnitID,
    //     dest: Location,
    //     map_dims: Dims,
    // },

    #[fail(display="Orders to unit with ID {:?} failed due to problem moving the unit: {}", id, move_error)]
    MoveError {
        id: UnitID,
        orders: Orders,
        move_error: MoveError,
    }
}

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
                game.set_orders(unit_id, None).map(|_| OrdersOutcome::completed_without_move(unit_id, self))
            },
            Orders::Sentry => {
                // do nothing---sentry is implemented as a reaction to approaching enemies
                Ok(OrdersOutcome::in_progress_without_move(unit_id, self))

            },
            Orders::GoTo{dest} => {
                go_to(self, game, unit_id, dest)
            },
            Orders::Explore => {
                explore(self, game, unit_id)
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
pub fn explore(orders: Orders, game: &mut Game, unit_id: UnitID) -> OrdersResult {
    let mut current_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
    let starting_loc = current_loc;
    let mut moves: Vec<MoveComponent> = Vec::new();
    // let mut unit = None;
    loop {
        // Get a fresh copy of the unit
        let unit = game.current_player_unit_by_id(unit_id).expect("Somehow the unit disappeared during exploration").clone();

        if unit.moves_remaining() == 0 {
            return Ok(OrdersOutcome::in_progress_with_move(unit_id, orders, Move::new(unit, starting_loc, moves).unwrap()));
        }

        if let Some(mut goal) = nearest_adjacent_unobserved_reachable_without_attacking(game, current_loc, &unit, game.wrapping()) {

            let shortest_paths = shortest_paths(game, unit.loc, &ObservedFilter{}, game.wrapping());

            let mut dist_to_real_goal = shortest_paths.dist[goal].unwrap();
            while dist_to_real_goal > unit.moves_remaining() {
                goal = shortest_paths.prev[goal].unwrap();
                dist_to_real_goal -= 1;
            }

            let mut move_result = game.move_unit_by_id_avoiding_combat(unit_id, goal)
                                  .map_err(|err| OrdersError::MoveError{id: unit_id, orders, move_error: err})?;


            if move_result.moved_successfully() {
                current_loc = move_result.ending_loc().unwrap();
                moves.append(&mut move_result.moves);
            } else {
                panic!("Unit was unexpectedly destroyed during exploration");
            }
        } else {
            return game.set_orders(unit_id, None)
                .map(|_| OrdersOutcome::completed_with_move(
                    unit_id,
                    orders,
                    Move::new(unit, starting_loc, moves).unwrap()
                )
            );
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
pub fn go_to(orders: Orders, game: &mut Game, unit_id: UnitID, dest: Location) -> OrdersResult {
    if !game.dims().contain(dest) {
        return Err(OrdersError::MoveError{ id: unit_id, orders, move_error: MoveError::DestinationOutOfBounds {
            dest,
            bounds: game.dims(),
        }});
    }

    let (moves_remaining, shortest_paths) = {
        let unit = game.current_player_unit_by_id(unit_id).unwrap();
        let moves_remaining = unit.moves_remaining;

        // Shortest paths emanating from the unit's location, allowing inclusion of unobserved tiles.
        let shortest_paths = shortest_paths(
            game,
            unit.loc,
            &Xenophile::new(UnitMovementFilter::new(unit)),
            game.wrapping());

        (moves_remaining, shortest_paths)
    };

    // Find the observed tile on the path from source to destination that is nearest to the
    // destination but also within reach of this unit's limited moves
    let mut dest2 = dest;
    loop {
        if game.current_player_tile(dest2).is_some() {
            if let Some(dist) = shortest_paths.dist[dest2] {
                if dist <= moves_remaining {
                    break;
                }
            }
        }
        dest2 = shortest_paths.prev[dest2].unwrap();
    }
    let dest2 = dest2;

    game.move_unit_by_id(unit_id, dest2)
        .map(|move_result| {
            let status = if let Some(ending_loc) = move_result.ending_loc() {
                // survived the immediate move

                if ending_loc == dest {
                    // got to the ultimate goal
                    game.set_orders(unit_id, None).unwrap();
                    OrdersStatus::Completed
                } else {
                    OrdersStatus::InProgress
                }

            } else {
                OrdersStatus::InProgress
            };

            OrdersOutcome {
                ordered_unit_id: unit_id,
                orders,
                move_result: Some(move_result),
                status
            }
        })
        .map_err(|err| OrdersError::MoveError {
            id: unit_id,
            orders,
            move_error: err,
        })
}

#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    use crate::{
        game::{
            Game,
            MoveError,
            map::MapData,
            unit::orders::{
                Orders,
                OrdersError
            },
        },
        name::unit_namer,
        util::{
            Location,
            Wrap2d,
        },
    };

    use super::OrdersStatus;

    #[test]
    fn test_go_to() {
        let map = MapData::try_from("i----------").unwrap();
        let mut game = Game::new_with_map(map, 1, false, unit_namer(), Wrap2d::BOTH);
        
        let id = game.current_player_toplevel_unit_by_loc(Location{x:0,y:0}).unwrap().id;

        let dest = Location{x: 0, y: 0};
        let result1 = game.order_unit_go_to(id, dest);
        assert_eq!(result1, Err(OrdersError::MoveError{id, orders: Orders::GoTo{dest}, move_error: MoveError::ZeroLengthMove}));

        let dest2 = Location{x: 255, y: 255};
        let result2 = game.order_unit_go_to(id, dest2);
        assert_eq!(result2, Err(OrdersError::MoveError{id, orders: Orders::GoTo{dest:dest2}, move_error: MoveError::DestinationOutOfBounds{
            dest: dest2,
            bounds: game.dims(),
        }}));

        let dest3 = Location{x:5, y:0};
        let result3 = game.order_unit_go_to(id, dest3);
        assert!(result3.is_ok());
        assert_eq!(result3.unwrap().status, OrdersStatus::InProgress);

        // Wait while the go-to order is carried out
        while game.unit_orders_requests().next().is_none() {
            let result = game.end_turn().unwrap();
            assert_eq!(result.current_player, 0);

            match result.carried_out_orders.len() {
                0|1 => {/* do nothing */},
                _ => panic!("Infantry shouldn't move more than 1 per turn")
            }
        }

        assert_eq!(game.turn(), 5);

        let final_dest = game.current_player_unit_loc(id).unwrap();
        assert_eq!(final_dest, dest3);
    }
}