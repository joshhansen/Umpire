use failure::{Fail};

use crate::{
    game::{
        Game,
        ProposedAction,
        
        map::{
            dijkstra::{
                ObservedReachableByPacifistUnit,
                OverlaySource,
                PacifistXenophileUnitMovementFilter,
                nearest_adjacent_unobserved_reachable_without_attacking,
                shortest_paths
            },
        },
        move_::{
            Move,
            MoveComponent,
            MoveError,
            ProposedMove,
        },
        obs::{
            OverlayObsTracker,
            UnifiedObsTracker,
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

/// The outcome of a unit following its orders
#[derive(Debug,PartialEq)]
pub struct OrdersOutcome {
    /// The ID of the ordered unit
    pub ordered_unit_id: UnitID,

    /// The orders that were given / carried out
    pub orders: Orders,

    /// Any movement undertaken by the unit as part of its orders
    pub move_: Option<Move>,

    /// A summary of the status of the orders, whether in progress or completed
    pub status: OrdersStatus,
}
impl OrdersOutcome {
    pub fn completed_without_move(ordered_unit_id: UnitID, orders: Orders) -> Self {
        Self { ordered_unit_id, orders, move_: None, status: OrdersStatus::Completed }
    }

    pub fn in_progress_without_move(ordered_unit_id: UnitID, orders: Orders) -> Self {
        Self { ordered_unit_id, orders, move_: None, status: OrdersStatus::InProgress }
    }

    pub fn in_progress_with_move(ordered_unit_id: UnitID, orders: Orders, move_: Move) -> Self {
        Self { ordered_unit_id, orders, move_: Some(move_), status: OrdersStatus::InProgress }
    }

    pub fn completed_with_move(ordered_unit_id: UnitID, orders: Orders, move_: Move) -> Self {
        Self { ordered_unit_id, orders, move_: Some(move_), status: OrdersStatus::Completed }
    }

    pub fn move_(&self) -> Option<&Move> {
        self.move_.as_ref()
    }

    pub fn status(&self) -> OrdersStatus  {
        self.status
    }
}

// /// The proposed outcome that would result if a unit carried out its orders
// #[derive(Debug,PartialEq)]
// pub struct ProposedOrdersOutcome {
//     /// The ID of the ordered unit
//     pub ordered_unit_id: UnitID,

//     /// The orders that were given / carried out
//     pub orders: Orders,

//     /// Any movement that would be undertaken by the unit as part of its orders
//     pub proposed_move: Option<Move>,

//     /// A summary of the status of the orders, whether in progress or completed
//     pub status: OrdersStatus,
// }

// // pub struct ProposedOrdersOutcome(OrdersOutcome);
// impl ProposedOrdersOutcome {
//     pub fn completed_without_move(ordered_unit_id: UnitID, orders: Orders) -> Self {
//         Self { ordered_unit_id, orders, proposed_move: None, status: OrdersStatus::Completed }
//     }

//     pub fn in_progress_without_move(ordered_unit_id: UnitID, orders: Orders) -> Self {
//         Self { ordered_unit_id, orders, proposed_move: None, status: OrdersStatus::InProgress }
//     }

//     pub fn in_progress_with_move(ordered_unit_id: UnitID, orders: Orders, proposed_move: Move) -> Self {
//         Self { ordered_unit_id, orders, proposed_move: Some(proposed_move), status: OrdersStatus::InProgress }
//     }

//     pub fn completed_with_move(ordered_unit_id: UnitID, orders: Orders, proposed_move: Move) -> Self {
//         Self { ordered_unit_id, orders, proposed_move: Some(proposed_move), status: OrdersStatus::Completed }
//     }

//     pub fn proposed_move(&self) -> Option<&Move> {
//         self.proposed_move.as_ref()
//     }

//     pub fn status(&self) -> OrdersStatus  {
//         self.status
//     }
// }

// impl ProposedAction for ProposedOrdersOutcome {
//     type Outcome = OrdersOutcome;
//     fn take(self, game: &mut Game) -> Self::Outcome {
        
//         // We need to run the proposed move first so the cloned unit contained therein gets put in place inside `Game`.
//         let move_ = self.proposed_move.map(|proposed_move| {
//             proposed_move.take(game)
//         });

//         // Now we clear the unit's orders if approprirate
//         if self.status == OrdersStatus::Completed {
//             game.clear_orders(self.ordered_unit_id).unwrap();
//         }

//         OrdersOutcome {
//             ordered_unit_id: self.ordered_unit_id,
//             orders: self.orders,
//             move_,
//             status: self.status,
//         }
//     }
// }

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
// pub type ProposedOrdersResult = Result<ProposedOrdersOutcome,OrdersError>;

// impl ProposedAction for ProposedOrdersResult {
//     type Outcome = OrdersResult;

//     fn take(self, game: &mut Game) -> Self::Outcome {
//         let outcome = self.map(|proposed_orders_outcome| proposed_orders_outcome.take(game));

//         match outcome {
//             Ok(OrdersOutcome{ status: OrdersStatus::Completed, ordered_unit_id, .. }) => {
//                 // The orders are already complete, clear them out
//                 game.clear_orders(ordered_unit_id).unwrap();
//             },
//             Err(ref err) => {
//                 // The orders resulted in error when carried out, clear them out
//                 match err {
//                     OrdersError::OrderedUnitDoesNotExist { id , ..} | OrdersError::MoveError { id, .. } => {
//                         game.clear_orders(*id).unwrap();
//                     },
//                 }
//             },
//             _ => {
//                 // For all other cases, do not clear the orders
//             }
//         }

//         outcome
//     }
// }

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
                game.clear_orders(unit_id).map(|_| OrdersOutcome::completed_without_move(unit_id, self))
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

    // pub fn propose(self, unit_id: UnitID, game: &Game) -> ProposedOrdersResult {
    //     match self {
    //         Orders::Skip => {
    //             // When the `ProposedOrdersOutcome` here is "made" into an `OrdersOutcome`, the contained `Skip` orders will be carried out correctly.
    //             Ok(ProposedOrdersOutcome::completed_without_move(unit_id, self))
    //         },
    //         Orders::Sentry => {
    //             // do nothing---sentry is implemented as a reaction to approaching enemies
    //             Ok(ProposedOrdersOutcome::in_progress_without_move(unit_id, self))
    //         },
    //         Orders::GoTo{dest} => {
    //             propose_go_to(self, game, unit_id, dest)
    //         },
    //         Orders::Explore => {
    //             propose_exploration(self, game, unit_id)
    //         }
    //     }
    // }

    pub fn propose(self, unit_id: UnitID, game: &Game) -> OrdersResult {
        let mut game = game.clone();
        self.carry_out(unit_id, &mut game)
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

// /// A proposal that orders be given to a unit
// pub struct ProposedSetOrders {
//     unit_id: UnitID,
//     orders: Option<Orders>,
// }

// impl ProposedAction for ProposedSetOrders {
//     type Outcome = Result<(),OrdersError>;

//     fn take(self, game: &mut Game) -> Self::Outcome {
//         game.set_orders(self.unit_id, self.orders)
//     }
// }

// /// A proposed action in which the specified unit's orders are set and then carried out
// /// 
// /// The result that would come about is represented by `proposed_orders_result`.
// pub struct ProposedSetAndFollowOrders {
//     pub unit_id: UnitID,
//     pub orders: Orders,
//     pub proposed_orders_result: ProposedOrdersResult,
// }

// impl ProposedAction for ProposedSetAndFollowOrders {
//     type Outcome = OrdersResult;
//     fn take(mut self, game: &mut Game) -> Self::Outcome {
//         // Set the orders of the unit in our simulated orders outcome so that after the simulation is made real, the
//         // orders will still be set.
//         if let Ok(ref mut proposed_orders_outcome) = self.proposed_orders_result {
//             if let Some(ref mut proposed_move) = proposed_orders_outcome.proposed_move {
//                 proposed_move.0.unit.orders = Some(self.orders);
//             }
//         }
//         self.proposed_orders_result.take(game)
//     }
// }

/// Keep moving toward the nearest unobserved tile we can see a path
/// to, until either there is no such tile or we run out of moves
/// If there are no such tiles then set the unit's orders to None
pub fn explore(orders: Orders, game: &mut Game, unit_id: UnitID) -> OrdersResult {
     // An overlay atop the player's observations which tracks the changes that occur during this move
    // let mut overlay = OverlayObsTracker::new(game.current_player_observations());
    // let mut overlay = UnifiedObsTracker::new(&game.map, game.current_player_observations().clone());

    // let observations = game.current_player_observations();

    // let mut overlay_map = OverlaySource::new(&game.map);
    // let mut overlay_observations = OverlayObsTracker::new(game.current_player_observations());

    // Clone the unit and simulate exploration using the clone
    let mut unit = game.current_player_unit_by_id(unit_id).expect("Somehow the unit disappeared during exploration").clone();

    let starting_loc = unit.loc;

    let mut move_components: Vec<MoveComponent> = Vec::new();
    // let mut unit = None;
    loop {
        // Get a fresh copy of the unit
        // let unit = game.current_player_unit_by_id(unit_id).expect("Somehow the unit disappeared during exploration").clone();

        if unit.moves_remaining() == 0 {
            return Ok(OrdersOutcome::in_progress_with_move(unit_id, orders, Move::new(unit, starting_loc, move_components).unwrap()));
        }

        let observations = game.current_player_observations();
        if let Some(mut goal) = nearest_adjacent_unobserved_reachable_without_attacking(observations, unit.loc, &unit, game.wrapping()) {

            //                                                     //FIXME this simplistic filter may be the source of some trouble
            // let shortest_paths = shortest_paths(game, unit.loc, &ObservedFilter{}, game.wrapping());

            let filter = ObservedReachableByPacifistUnit{unit: &unit};
            let shortest_paths = shortest_paths(observations, unit.loc, &filter, game.wrapping());

            // let shortest_paths = {
            //     let filter = ObservedReachableByPacifistUnit{unit: &unit};
            //     shortest_paths(&overlay, unit.loc, &filter, game.wrapping())
            // };

            let mut dist_to_real_goal = shortest_paths.dist[goal].unwrap();
            while dist_to_real_goal > unit.moves_remaining() {
                goal = shortest_paths.prev[goal].unwrap();
                dist_to_real_goal -= 1;
            }

            // eprintln!("move from {} to {}", unit.loc, goal);
            // let mut move_result = game.propose_move_unit_avoiding_combat(unit, goal)
            //                       .map_err(|err| OrdersError::MoveError{id: unit_id, orders, move_error: err})?;

            let mut move_ = game.move_unit_by_id_using_filter(
                unit.id, goal, &filter
            ).map_err(|err| OrdersError::MoveError{id: unit_id, orders, move_error: err})?;

            // let mut move_ = game.propose_move_unit_following_shortest_paths_custom_tracker(&unit, goal, shortest_paths, &mut overlay)
            //                           .map_err(|err| OrdersError::MoveError{id: unit_id, orders, move_error: err})?;


            if move_.moved_successfully() {
                // unit.loc = move_result.0.ending_loc().unwrap();
                unit = move_.unit;

                move_components.append(&mut move_.components);

                // for move_component in move_.0.components {

                //     for located_obs in move_component.observations_after_move {

                //     }

                //     move_components.push(move_component);
                // }
            } else {
                panic!("Unit was unexpectedly destroyed during exploration");
            }
        } else {
            return Ok(if move_components.is_empty() {
                OrdersOutcome::completed_without_move(unit_id, orders)
            } else {
                OrdersOutcome::completed_with_move(
                    unit_id,
                    orders,
                    Move::new(unit, starting_loc, move_components).unwrap()
                )
            });
            // return game.set_orders(unit_id, None)
            //     .map(|_| if moves.is_empty() {
            //         ProposedOrdersOutcome::completed_without_move(unit_id, orders)
            //     } else {
            //         ProposedOrdersOutcome::completed_with_move(
            //             unit_id,
            //             orders,
            //             ProposedMove::new(unit, starting_loc, moves).unwrap()
            //         )
            //     }
            // );
        }
    }
}

pub fn propose_exploration(orders: Orders, game: &Game, unit_id: UnitID) -> OrdersResult {
    let mut new = game.clone();
    explore(orders, &mut new, unit_id)
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
        return Err(OrdersError::MoveError{ id: unit_id, orders, move_error: MoveError::DestinationOutOfBounds {}});
    }

    let (moves_remaining, shortest_paths, src) = {
        let unit = game.current_player_unit_by_id(unit_id).unwrap();
        let moves_remaining = unit.moves_remaining;

        let filter = PacifistXenophileUnitMovementFilter{unit: &unit};

        // Shortest paths emanating from the unit's location, allowing inclusion of unobserved tiles.
        let shortest_paths = shortest_paths(
            game,
            unit.loc,
            &filter,
            game.wrapping());

        (moves_remaining, shortest_paths, unit.loc)
    };

    if src==dest {
        return Err(OrdersError::MoveError{ id: unit_id, orders, move_error: MoveError::ZeroLengthMove});
    }

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

        if let Some(prev_dest) = shortest_paths.prev[dest2] {
            dest2 = prev_dest;
        } else {
            return Err(OrdersError::MoveError{ id: unit_id, orders, move_error: MoveError::NoRoute {
                id: unit_id,
                src,
                dest,
            }});
        }
    }
    let dest2 = dest2;

    if dest2 == src {
        // We aren't going anywhere---the hypothetical route to the destination isn't coming to pass
        //FIXME I'm not sure why this situation arises---why does following the shortest path
        //     not actually lead us to the destination sometimes?

        return Err(OrdersError::MoveError{ id: unit_id, orders, move_error: MoveError::NoRoute {
            id: unit_id,
            src,
            dest,
        }});
    }

    game.move_unit_by_id(unit_id, dest2)
        .map(|move_| {
            let status = if let Some(ending_loc) = move_.ending_loc() {
                // survived the immediate move

                if ending_loc == dest {
                    // got to the ultimate goal
                    // game.set_orders(unit_id, None).unwrap();
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
                move_: Some(move_),
                status
            }
        })
        .map_err(|err| OrdersError::MoveError {
            id: unit_id,
            orders,
            move_error: err,
        })
}
pub fn propose_go_to(orders: Orders, game: &Game, unit_id: UnitID, dest: Location) -> OrdersResult {
    let mut new = game.clone();
    go_to(orders, &mut new, unit_id, dest)
}

pub mod test_support {
    use std::sync::{
        Arc,
        RwLock,
    };

    use crate::{
        game::{
            Game,
            PlayerNum,
            map::{
                gen::generate_map,
            },
            unit::{
                UnitType,
                orders::Orders,
            },
        },
        name::{
            IntNamer,
        },
        util::{
            Dims,
            Wrap2d,
        },
    };

    use super::OrdersStatus;


    // We keep this out of cfg(test) so it can be used in a benchmark
    pub fn test_explore(dims: Dims) {
        let mut city_namer = IntNamer::new("city");
        let unit_namer = IntNamer::new("unit");
        let players: PlayerNum = 1;
        let map = generate_map(&mut city_namer, dims, players);

        let mut game = Game::new_with_map(map, players, true, Arc::new(RwLock::new(unit_namer)), Wrap2d::BOTH);

        // Request a fighter to be produced
        let city_loc = game.production_set_requests().next().unwrap();
        game.set_production_by_loc(city_loc, UnitType::Fighter).unwrap();

        // Wait until the fighter is produced
        while game.unit_orders_requests().count() == 0 {
            game.end_turn().unwrap();
        }

        game.clear_production_and_ignore(city_loc).unwrap();

        let fighter_id = game.unit_orders_requests().next().unwrap();

        let outcome = game.order_unit_explore(fighter_id).unwrap();
        assert_eq!(outcome.status, OrdersStatus::InProgress);
        assert!(outcome.move_.is_some());
        assert!(!outcome.move_.as_ref().unwrap().components.is_empty());


        let fighter = game.current_player_unit_by_id(fighter_id).unwrap();
        assert_eq!(fighter.orders, Some(Orders::Explore));

        // Wait until the fighter has explored everything

        let mut done = false;
        
        while game.unit_orders_requests().count() == 0 {
            let turn_start = game.end_turn().unwrap();
            assert_eq!(turn_start.orders_results.len(), 1);

            let orders_result = turn_start.orders_results.get(0).unwrap();
            match orders_result {
                Ok(orders_outcome) => {
                    assert!(!done);
                    if orders_outcome.move_.is_none() {
                        done = true;
                    } else {
                        assert!(!orders_outcome.move_.as_ref().unwrap().components.is_empty());
                    }
                },
                Err(orders_err) => panic!("Orders error: {}", orders_err),
            }
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::{
        convert::TryFrom,
        sync::{
            Arc,
            RwLock,
        },
    };

    use crate::{
        game::{
            AlignedMaybe,
            Game,
            MoveError,
            map::{
                MapData,
            },
            unit::{
                orders::{
                    Orders,
                    OrdersError,
                    OrdersOutcome,
                    propose_exploration,
                    test_support,
                },
                UnitID,
            },
        },
        name::{
            IntNamer,
            unit_namer,
        },
        util::{
            Dims,
            Location,
            Wrap2d,
        },
    };

    use super::OrdersStatus;

    #[test]
    fn test_go_to() {
        let map = MapData::try_from("i----------").unwrap();
        let mut game = Game::new_with_map(map, 1, false, Arc::new(RwLock::new(unit_namer())), Wrap2d::BOTH);
        
        let id = game.current_player_toplevel_unit_by_loc(Location{x:0,y:0}).unwrap().id;

        let dest = Location{x: 0, y: 0};
        let result1 = game.order_unit_go_to(id, dest);
        assert_eq!(result1, Err(OrdersError::MoveError{id, orders: Orders::GoTo{dest}, move_error: MoveError::ZeroLengthMove}));

        let dest2 = Location{x: 255, y: 255};
        let result2 = game.order_unit_go_to(id, dest2);
        assert_eq!(result2, Err(OrdersError::MoveError{id, orders: Orders::GoTo{dest:dest2}, move_error: MoveError::DestinationOutOfBounds{}}));

        let dest3 = Location{x:5, y:0};
        let result3 = game.order_unit_go_to(id, dest3);
        assert!(result3.is_ok());
        assert_eq!(result3.unwrap().status, OrdersStatus::InProgress);

        // Wait while the go-to order is carried out
        while game.unit_orders_requests().next().is_none() {
            let turn_start = game.end_turn().unwrap();
            assert_eq!(turn_start.current_player, 0);

            match turn_start.orders_results.len() {
                0|1 => {/* do nothing */},
                _ => panic!("Infantry shouldn't move more than 1 per turn")
            }

            // Make sure we don't go on too long
            assert!(game.turn() < 6);
        }

        assert_eq!(game.turn(), 5);

        let unit = game.current_player_unit_by_id(id).unwrap();
        assert!(!unit.has_orders());
        assert_eq!(unit.loc, dest3);
        assert_eq!(unit.moves_remaining, 1);
        assert!(unit.belongs_to_player(0));

        assert!(game.current_player_units().any(|x| x.id == unit.id));
        assert!(game.unit_orders_requests().any(|x| x == unit.id));
        assert!(!game.units_with_pending_orders().any(|x| x == unit.id));
    }

    #[test]
    pub fn test_explore() {
        test_support::test_explore(Dims::new(10, 10));
        test_support::test_explore(Dims::new(20, 20));
        test_support::test_explore(Dims::new(100, 100));
    }

   #[test]
   fn test_propose_exploration() {
    //    pub fn propose_exploration(orders: Orders, game: &Game, unit_id: UnitID) -> ProposedOrdersResult {
        let unit_namer = IntNamer::new("abc");
        let map = MapData::try_from("i--------------------").unwrap();
        let game = Game::new_with_map(map, 1, true, Arc::new(RwLock::new(unit_namer)), Wrap2d::NEITHER);

        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();

        let outcome: OrdersOutcome = propose_exploration(Orders::Explore, &game, unit_id).unwrap();

    //         /// The ID of the ordered unit
    // pub ordered_unit_id: UnitID,

    // /// The orders that were given / carried out
    // pub orders: Orders,

    // /// Any movement that would be undertaken by the unit as part of its orders
    // pub proposed_move: Option<ProposedMove>,

    // /// A summary of the status of the orders, whether in progress or completed
    // pub status: OrdersStatus,

        assert_eq!(outcome.ordered_unit_id, unit_id);
        assert_eq!(outcome.orders, Orders::Explore);
        assert_eq!(outcome.status, OrdersStatus::InProgress);
        let proposed_move = outcome.move_().unwrap();
        assert_eq!(proposed_move.unit.id, unit_id);
        assert_eq!(proposed_move.unit.loc, Location::new(1, 0));
        assert_eq!(proposed_move.starting_loc, Location::new(0, 0));

    // pub loc: Location,
    // /// Was the unit carried by another unit? If so, which one?
    // pub carrier: Option<UnitID>,
    // pub unit_combat: Option<CombatOutcome<Unit,Unit>>,
    // pub city_combat: Option<CombatOutcome<Unit,City>>,
    // pub observations_after_move: Vec<LocatedObs>,


        let component = &proposed_move.components[0];
        assert_eq!(component.loc, Location::new(1, 0));
        assert_eq!(component.carrier, None);
        assert_eq!(component.unit_combat, None);
        assert_eq!(component.city_combat, None);

   }
}