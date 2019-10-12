use failure::{Fail};

use crate::{
    game::{
        Game,
        ProposedAction,
        
        map::{
            dijkstra::{
                ObservedFilter,
                ObservedReachableByPacifistUnit,
                UnitMovementFilter,
                Xenophile,
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

/// The proposed outcome that would result if a unit carried out its orders
#[derive(Debug,PartialEq)]
pub struct ProposedOrdersOutcome {
    /// The ID of the ordered unit
    pub ordered_unit_id: UnitID,

    /// The orders that were given / carried out
    pub orders: Orders,

    /// Any movement that would be undertaken by the unit as part of its orders
    pub proposed_move: Option<ProposedMove>,

    /// A summary of the status of the orders, whether in progress or completed
    pub status: OrdersStatus,
}

// pub struct ProposedOrdersOutcome(OrdersOutcome);
impl ProposedOrdersOutcome {
    pub fn completed_without_move(ordered_unit_id: UnitID, orders: Orders) -> Self {
        Self { ordered_unit_id, orders, proposed_move: None, status: OrdersStatus::Completed }
    }

    pub fn in_progress_without_move(ordered_unit_id: UnitID, orders: Orders) -> Self {
        Self { ordered_unit_id, orders, proposed_move: None, status: OrdersStatus::InProgress }
    }

    pub fn in_progress_with_move(ordered_unit_id: UnitID, orders: Orders, proposed_move: ProposedMove) -> Self {
        Self { ordered_unit_id, orders, proposed_move: Some(proposed_move), status: OrdersStatus::InProgress }
    }

    pub fn completed_with_move(ordered_unit_id: UnitID, orders: Orders, proposed_move: ProposedMove) -> Self {
        Self { ordered_unit_id, orders, proposed_move: Some(proposed_move), status: OrdersStatus::Completed }
    }

    pub fn proposed_move(&self) -> Option<&ProposedMove> {
        self.proposed_move.as_ref()
    }

    pub fn status(&self) -> OrdersStatus  {
        self.status
    }
}

impl ProposedAction for ProposedOrdersOutcome {
    type Outcome = OrdersOutcome;
    fn take(self, game: &mut Game) -> Self::Outcome {
        // if self.orders==Orders::Skip {
        if self.status == OrdersStatus::Completed {
            game.set_orders(self.ordered_unit_id, None).unwrap();
        }

        OrdersOutcome {
            ordered_unit_id: self.ordered_unit_id,
            orders: self.orders,
            move_: self.proposed_move.map(|proposed_move| proposed_move.take(game)),
            status: self.status,
        }
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
pub type ProposedOrdersResult = Result<ProposedOrdersOutcome,OrdersError>;

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

    pub fn propose(self, unit_id: UnitID, game: &Game) -> ProposedOrdersResult {
        match self {
            Orders::Skip => {
                // When the `ProposedOrdersOutcome` here is "made" into an `OrdersOutcome`, the contained `Skip` orders will be carried out correctly.
                Ok(ProposedOrdersOutcome::completed_without_move(unit_id, self))
            },
            Orders::Sentry => {
                // do nothing---sentry is implemented as a reaction to approaching enemies
                Ok(ProposedOrdersOutcome::in_progress_without_move(unit_id, self))
            },
            Orders::GoTo{dest} => {
                propose_go_to(self, game, unit_id, dest)
            },
            Orders::Explore => {
                propose_exploration(self, game, unit_id)
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
pub fn explore(orders: Orders, game: &mut Game, unit_id: UnitID) -> OrdersResult {
    propose_exploration(orders, game, unit_id).map(|proposed_orders_result| proposed_orders_result.take(game))
}

pub fn propose_exploration(orders: Orders, game: &Game, unit_id: UnitID) -> ProposedOrdersResult {
    // let mut current_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
    // let starting_loc = current_loc;

    // An overlay atop the player's observations which tracks the changes that occur during this move
    let overlay = OverlayObsTracker::new(game.current_player_observations());

    // Clone the unit and simulate exploration using the clone
    let mut unit = game.current_player_unit_by_id(unit_id).expect("Somehow the unit disappeared during exploration").clone();

    let starting_loc = unit.loc;

    let mut move_components: Vec<MoveComponent> = Vec::new();
    // let mut unit = None;
    loop {
        // Get a fresh copy of the unit
        // let unit = game.current_player_unit_by_id(unit_id).expect("Somehow the unit disappeared during exploration").clone();

        if unit.moves_remaining() == 0 {
            return Ok(ProposedOrdersOutcome::in_progress_with_move(unit_id, orders, ProposedMove::new(unit, starting_loc, move_components).unwrap()));
        }

        if let Some(mut goal) = nearest_adjacent_unobserved_reachable_without_attacking(&overlay, unit.loc, &unit, game.wrapping()) {

            //                                                     //FIXME this simplistic filter may be the source of some trouble
            // let shortest_paths = shortest_paths(game, unit.loc, &ObservedFilter{}, game.wrapping());

            let shortest_paths = {
                let filter = ObservedReachableByPacifistUnit{unit: &unit};
                shortest_paths(game, unit.loc, &filter, game.wrapping())
            };

            let mut dist_to_real_goal = shortest_paths.dist[goal].unwrap();
            while dist_to_real_goal > unit.moves_remaining() {
                goal = shortest_paths.prev[goal].unwrap();
                dist_to_real_goal -= 1;
            }

            eprintln!("move from {} to {}", unit.loc, goal);
            // let mut move_result = game.propose_move_unit_avoiding_combat(unit, goal)
            //                       .map_err(|err| OrdersError::MoveError{id: unit_id, orders, move_error: err})?;

            let mut move_ = game.propose_move_unit_following_shortest_paths(unit, goal, shortest_paths)
                                      .map_err(|err| OrdersError::MoveError{id: unit_id, orders, move_error: err})?;


            if move_.0.moved_successfully() {
                // unit.loc = move_result.0.ending_loc().unwrap();
                unit = move_.0.unit;

                move_components.append(&mut move_.0.components);

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
                ProposedOrdersOutcome::completed_without_move(unit_id, orders)
            } else {
                ProposedOrdersOutcome::completed_with_move(
                    unit_id,
                    orders,
                    ProposedMove::new(unit, starting_loc, move_components).unwrap()
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
    propose_go_to(orders, game, unit_id, dest).map(|proposed_orders_outcome| proposed_orders_outcome.take(game))
}
pub fn propose_go_to(orders: Orders, game: &Game, unit_id: UnitID, dest: Location) -> ProposedOrdersResult {
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

    game.propose_move_unit_by_id(unit_id, dest2)
        .map(|proposed_move| {
            let status = if let Some(ending_loc) = proposed_move.0.ending_loc() {
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

            ProposedOrdersOutcome {
                ordered_unit_id: unit_id,
                orders,
                proposed_move: Some(proposed_move),
                status
            }
        })
        .map_err(|err| OrdersError::MoveError {
            id: unit_id,
            orders,
            move_error: err,
        })
}

pub mod test2 {
    use crate::{
        game::{
            Game,
            PlayerNum,
            map::{
                gen::generate_map,
            },
            unit::{
                UnitType,
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

    use super::{
        OrdersStatus,
    };

    pub fn _test_explore(dims: Dims) {
        let mut city_namer = IntNamer::new("city");
        let unit_namer = IntNamer::new("unit");
        let players: PlayerNum = 1;
        let map = generate_map(&mut city_namer, dims, players);


        let mut game = Game::new_with_map(map, players, true, Box::new(unit_namer), Wrap2d::BOTH);

        // Request a fighter to be produced
        let city_loc = game.production_set_requests().next().unwrap();
        game.set_production(city_loc, UnitType::Fighter).unwrap();

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


        // Wait until the fighter has explored everything

        let mut done = false;
        
        while game.unit_orders_requests().count() == 0 {
            let turn_start = game.end_turn().unwrap();
            assert_eq!(turn_start.carried_out_orders.len(), 1);

            let carried_out_orders = turn_start.carried_out_orders.get(0).unwrap();

            match carried_out_orders {
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
    use std::convert::TryFrom;

    use crate::{
        game::{
            Game,
            MoveError,
            map::{
                MapData,
            },
            unit::{
                orders::{
                    Orders,
                    OrdersError,
                    ProposedOrdersOutcome,
                    ProposedOrdersResult,
                    propose_exploration,
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
        let mut game = Game::new_with_map(map, 1, false, Box::new(unit_namer()), Wrap2d::BOTH);
        
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

    

    #[test]
    pub fn test_explore() {
        panic!("TODO: implement")
        //FIXME

        // super::test2::_test_explore(Dims::new(10, 10));

        // let mut city_namer = IntNamer::new("city");
        // let unit_namer = IntNamer::new("unit");
        // let dims_large = Dims::new(100, 100);
        // let players: PlayerNum = 1;
        // let map = generate_map(&mut city_namer, dims_large, players);


        // let mut game = Game::new_with_map(map, players, true, Box::new(unit_namer), Wrap2d::BOTH);

        // // Request a fighter to be produced
        // let city_loc = game.production_set_requests().next().unwrap();
        // game.set_production(city_loc, UnitType::Fighter).unwrap();

        // // Wait until the fighter is produced
        // while game.unit_orders_requests().count() == 0 {
        //     game.end_turn().unwrap();
        // }

        // game.clear_production_and_ignore(city_loc).unwrap();

        // let fighter_id = game.unit_orders_requests().next().unwrap();

        // game.order_unit_explore(fighter_id).unwrap();

        // // Wait until the fighter has explored everything
        // while game.unit_orders_requests().count() == 0 {
        //     game.end_turn().unwrap();
        // }
    }

   #[test]
   fn test_propose_exploration() {
    //    pub fn propose_exploration(orders: Orders, game: &Game, unit_id: UnitID) -> ProposedOrdersResult {
        let unit_namer = IntNamer::new("abc");
        let map = MapData::try_from("i--------------------").unwrap();
        let game = Game::new_with_map(map, 1, true, Box::new(unit_namer), Wrap2d::NEITHER);

        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();

        let outcome: ProposedOrdersOutcome = propose_exploration(Orders::Explore, &game, unit_id).unwrap();

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
        let proposed_move = outcome.proposed_move().unwrap();
        assert_eq!(proposed_move.0.unit.id, unit_id);
        assert_eq!(proposed_move.0.unit.loc, Location::new(1, 0));
        assert_eq!(proposed_move.0.starting_loc, Location::new(0, 0));

    // pub loc: Location,
    // /// Was the unit carried by another unit? If so, which one?
    // pub carrier: Option<UnitID>,
    // pub unit_combat: Option<CombatOutcome<Unit,Unit>>,
    // pub city_combat: Option<CombatOutcome<Unit,City>>,
    // pub observations_after_move: Vec<LocatedObs>,


        let component = &proposed_move.0.components[0];
        assert_eq!(component.loc, Location::new(1, 0));
        assert_eq!(component.carrier, None);
        assert_eq!(component.unit_combat, None);
        assert_eq!(component.city_combat, None);

   }
}