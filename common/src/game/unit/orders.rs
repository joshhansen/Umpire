use super::Unit;
use crate::{
    game::{
        map::{
            dijkstra::{
                nearest_adjacent_unobserved_reachable_without_attacking, shortest_paths,
                ObservedReachableByPacifistUnit, PacifistXenophileUnitMovementFilter,
            },
            LocationGridI,
        },
        move_::{Move, MoveComponent, MoveError},
        unit::UnitID,
        Game, GameError, Proposed,
    },
    util::Location,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum OrdersStatus {
    InProgress,
    Completed,
}

/// The outcome of a unit following its orders
#[derive(Debug, PartialEq)]
pub struct OrdersOutcome {
    /// The ID of the ordered unit
    pub ordered_unit: Unit,

    /// The orders that were given / carried out
    pub orders: Orders,

    /// Any movement undertaken by the unit as part of its orders
    pub move_: Option<Move>,

    /// A summary of the status of the orders, whether in progress or completed
    pub status: OrdersStatus,
}
impl OrdersOutcome {
    pub fn completed_without_move(ordered_unit: Unit, orders: Orders) -> Self {
        Self {
            ordered_unit,
            orders,
            move_: None,
            status: OrdersStatus::Completed,
        }
    }

    pub fn in_progress_without_move(ordered_unit: Unit, orders: Orders) -> Self {
        Self {
            ordered_unit,
            orders,
            move_: None,
            status: OrdersStatus::InProgress,
        }
    }

    pub fn in_progress_with_move(ordered_unit: Unit, orders: Orders, move_: Move) -> Self {
        Self {
            ordered_unit,
            orders,
            move_: Some(move_),
            status: OrdersStatus::InProgress,
        }
    }

    pub fn completed_with_move(ordered_unit: Unit, orders: Orders, move_: Move) -> Self {
        Self {
            ordered_unit,
            orders,
            move_: Some(move_),
            status: OrdersStatus::Completed,
        }
    }

    pub fn move_(&self) -> Option<&Move> {
        self.move_.as_ref()
    }

    pub fn status(&self) -> OrdersStatus {
        self.status
    }
}

pub type OrdersResult = Result<OrdersOutcome, GameError>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Orders {
    Skip,
    Sentry,
    GoTo { dest: Location },
    Explore,
}

impl Orders {
    pub fn carry_out(self, unit_id: UnitID, game: &mut Game) -> OrdersResult {
        match self {
            Orders::Skip => {
                let unit = game.map.unit_by_id(unit_id).unwrap().clone();
                game.clear_orders(unit_id)
                    .map(|_| OrdersOutcome::completed_without_move(unit, self))
            }
            Orders::Sentry => {
                // do nothing---sentry is implemented as a reaction to approaching enemies
                let unit = game.map.unit_by_id(unit_id).unwrap().clone();
                Ok(OrdersOutcome::in_progress_without_move(unit, self))
            }
            Orders::GoTo { dest } => go_to(self, game, unit_id, dest),
            Orders::Explore => explore(self, game, unit_id),
        }
    }

    pub fn propose(self, unit_id: UnitID, game: &Game) -> Proposed<OrdersResult> {
        let mut game = game.clone();
        let delta = self.carry_out(unit_id, &mut game);
        Proposed::new(game, delta)
    }

    /// A present-tense, progressive aspect verb phrase describing the action of the unit as it carries out these orders
    /// Example: "standing sentry" for a sentry unit.
    pub fn present_progressive_description(self) -> String {
        match self {
            Orders::Skip => String::from("skipping its turn"),
            Orders::Sentry => String::from("standing sentry"),
            Orders::GoTo { dest } => {
                format!("going to {}", dest)
            }
            Orders::Explore => String::from("exploring"),
        }
    }
}

/// Keep moving toward the nearest unobserved tile we can see a path
/// to, until either there is no such tile or we run out of moves
/// If there are no such tiles then set the unit's orders to None
pub fn explore(orders: Orders, game: &mut Game, unit_id: UnitID) -> OrdersResult {
    // Clone the unit and simulate exploration using the clone
    let mut unit: Unit = game
        .current_player_unit_by_id(unit_id)
        .ok_or(GameError::NoSuchUnit { id: unit_id })?
        .clone();

    let starting_loc = unit.loc;

    let mut move_components: Vec<MoveComponent> = Vec::new();

    loop {
        if unit.moves_remaining() == 0 {
            return Ok(OrdersOutcome::in_progress_with_move(
                unit.clone(),
                orders,
                Move::new(unit, starting_loc, move_components).unwrap(),
            ));
        }

        let observations = game.current_player_observations();
        if let Some(mut goal) = nearest_adjacent_unobserved_reachable_without_attacking(
            observations,
            unit.loc,
            &unit,
            game.wrapping(),
        ) {
            let filter = ObservedReachableByPacifistUnit { unit: &unit };
            let shortest_paths = shortest_paths(
                observations,
                unit.loc,
                &filter,
                game.wrapping(),
                std::u16::MAX,
            );

            // Find the proximate goal that the unit can reach on this turn
            let mut dist_to_real_goal = shortest_paths.dist[goal];
            while dist_to_real_goal > unit.moves_remaining() {
                goal = shortest_paths.prev[goal];
                dist_to_real_goal -= 1;
            }

            let mut move_ = game
                .move_unit_by_id_using_filter(unit.id, goal, &filter)
                .map_err(GameError::MoveError)?;

            if move_.moved_successfully() {
                unit = move_.unit;

                move_components.append(&mut move_.components);
            } else {
                panic!("Unit was unexpectedly destroyed during exploration");
            }
        } else {
            return Ok(if move_components.is_empty() {
                OrdersOutcome::completed_without_move(unit, orders)
            } else {
                OrdersOutcome::completed_with_move(
                    unit.clone(),
                    orders,
                    Move::new(unit, starting_loc, move_components).unwrap(),
                )
            });
        }
    }
}

pub fn propose_exploration(orders: Orders, game: &Game, unit_id: UnitID) -> Proposed<OrdersResult> {
    let mut new = game.clone();
    let delta = explore(orders, &mut new, unit_id);
    Proposed::new(new, delta)
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
        return Err(GameError::MoveError(MoveError::DestinationOutOfBounds {}));
    }

    let (moves_remaining, shortest_paths, src) = {
        let unit = game
            .current_player_unit_by_id(unit_id)
            .ok_or(GameError::NoSuchUnit { id: unit_id })?;

        let moves_remaining = unit.moves_remaining;

        let filter = PacifistXenophileUnitMovementFilter { unit: &unit };

        // Shortest paths emanating from the unit's location, allowing inclusion of unobserved tiles.
        let shortest_paths =
            shortest_paths(game, unit.loc, &filter, game.wrapping(), std::u16::MAX);

        (moves_remaining, shortest_paths, unit.loc)
    };

    if src == dest {
        return Err(GameError::MoveError(MoveError::ZeroLengthMove));
    }

    // Find the observed tile on the path from source to destination that is nearest to the
    // destination but also within reach of this unit's limited moves
    let mut dest2 = dest;
    loop {
        if game.current_player_tile(dest2).is_some() {
            if let Some(dist) = shortest_paths.dist.get(dest2).cloned() {
                if dist <= moves_remaining {
                    break;
                }
            }
        }

        dest2 = shortest_paths
            .prev
            .get(dest2)
            .cloned()
            .ok_or(GameError::MoveError(MoveError::NoRoute {
                id: unit_id,
                src,
                dest,
            }))?;
    }
    let dest2 = dest2;

    if dest2 == src {
        // We aren't going anywhere---the hypothetical route to the destination isn't coming to pass
        //FIXME I'm not sure why this situation arises---why does following the shortest path
        //     not actually lead us to the destination sometimes?

        return Err(GameError::MoveError(MoveError::NoRoute {
            id: unit_id,
            src,
            dest,
        }));
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
                ordered_unit: game.current_player_unit_by_id(unit_id).unwrap().clone(),
                orders,
                move_: Some(move_),
                status,
            }
        })
        .map_err(GameError::MoveError)
}
pub fn propose_go_to(
    orders: Orders,
    game: &Game,
    unit_id: UnitID,
    dest: Location,
) -> Proposed<OrdersResult> {
    let mut new = game.clone();
    let delta = go_to(orders, &mut new, unit_id, dest);
    Proposed::new(new, delta)
}

pub mod test_support {
    use crate::{
        game::{
            map::gen::generate_map,
            unit::{orders::Orders, UnitType},
            Game, PlayerNum,
        },
        name::IntNamer,
        util::{Dims, Wrap2d},
    };

    use super::OrdersStatus;

    // We keep this out of cfg(test) so it can be used in a benchmark
    pub fn test_explore(dims: Dims) {
        let mut city_namer = IntNamer::new("city");
        let players: PlayerNum = 1;
        let map = generate_map(&mut city_namer, dims, players);

        let mut game = Game::new_with_map(map, players, true, None, Wrap2d::BOTH);

        // Request a fighter to be produced
        let city_loc = game.production_set_requests().next().unwrap();
        game.set_production_by_loc(city_loc, UnitType::Fighter)
            .unwrap();

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
                }
                Err(orders_err) => panic!("Orders error: {}", orders_err),
            }
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::{rc::Rc, sync::RwLock};

    use crate::{
        game::{
            map::MapData,
            unit::{
                orders::{propose_exploration, test_support, Orders},
                UnitID,
            },
            AlignedMaybe, Game, GameError, MoveError, Proposed,
        },
        name::unit_namer,
        util::{Dims, Location, Wrap2d},
    };

    use super::{OrdersResult, OrdersStatus};

    #[test]
    fn test_go_to() {
        let map = MapData::try_from("i----------").unwrap();
        let mut game = Game::new_with_map(
            map,
            1,
            false,
            Some(Rc::new(RwLock::new(unit_namer()))),
            Wrap2d::BOTH,
        );

        let id = game
            .current_player_toplevel_unit_by_loc(Location { x: 0, y: 0 })
            .unwrap()
            .id;

        let dest = Location { x: 0, y: 0 };
        let result1 = game.order_unit_go_to(id, dest);
        assert_eq!(
            result1,
            Err(GameError::MoveError(MoveError::ZeroLengthMove))
        );

        let dest2 = Location { x: 255, y: 255 };
        let result2 = game.order_unit_go_to(id, dest2);
        assert_eq!(
            result2,
            Err(GameError::MoveError(MoveError::DestinationOutOfBounds {}))
        );

        let dest3 = Location { x: 5, y: 0 };
        let result3 = game.order_unit_go_to(id, dest3);
        assert!(result3.is_ok());
        assert_eq!(result3.unwrap().status, OrdersStatus::InProgress);

        // Wait while the go-to order is carried out
        while game.unit_orders_requests().next().is_none() {
            let turn_start = game.end_turn().unwrap();
            assert_eq!(turn_start.current_player, 0);

            match turn_start.orders_results.len() {
                0 | 1 => { /* do nothing */ }
                _ => panic!("Infantry shouldn't move more than 1 per turn"),
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
        let map = MapData::try_from("i--------------------").unwrap();
        let game = Game::new_with_map(map, 1, true, None, Wrap2d::NEITHER);

        let unit_id: UnitID = game.unit_orders_requests().next().unwrap();

        let proposed_outcome: Proposed<OrdersResult> =
            propose_exploration(Orders::Explore, &game, unit_id);
        let outcome = proposed_outcome.delta.unwrap();

        //         /// The ID of the ordered unit
        // pub ordered_unit_id: UnitID,

        // /// The orders that were given / carried out
        // pub orders: Orders,

        // /// Any movement that would be undertaken by the unit as part of its orders
        // pub proposed_move: Option<ProposedMove>,

        // /// A summary of the status of the orders, whether in progress or completed
        // pub status: OrdersStatus,

        assert_eq!(outcome.ordered_unit.id, unit_id);
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
