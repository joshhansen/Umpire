use game::Game;
use log::LogTarget;
use map::dijkstra::{UnitMovementFilter,Xenophile,nearest_reachable_adjacent_unobserved,shortest_paths};
use map::newmap::UnitID;
use ui::MoveAnimator;
use util::Location;

#[derive(Clone,Debug,PartialEq)]
pub enum Orders {
    Sentry,
    GoTo{dest:Location},
    Explore
}

impl Orders {
    pub fn carry_out<U:LogTarget+MoveAnimator>(&self, unit_id: UnitID, game: &mut Game, ui: &mut U) {
        match *self {
            Orders::Sentry => {
                // do nothing---sentry is implemented as a reaction to approaching enemies
            },
            Orders::GoTo{dest} => {
                go_to(game, unit_id, dest, ui);
            },
            Orders::Explore => {
                explore(game, unit_id, ui);
            }
        }//match orders
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
pub fn explore<U:LogTarget+MoveAnimator>(game: &mut Game, unit_id: UnitID, ui: &mut U) {
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
    loop {
        if let Some(goal) = {
            let unit = game.unit_by_id(unit_id).unwrap();
            nearest_reachable_adjacent_unobserved(game, current_loc, unit, game.wrapping())
        } {

            match game.move_unit_by_id(unit_id, goal) {
                Ok(move_result) => {
                    ui.animate_move(game, &move_result);

                    if move_result.moved_successfully() {
                        current_loc = move_result.ending_loc().unwrap();
                    }
                },
                Err(msg) => {
                    ui.log_message(format!("Error moving unit toward {}: {}", goal, msg));
                }
            }

        } else {
            game.give_orders(unit_id, None, ui).unwrap();
            break;
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
pub fn go_to<U:LogTarget+MoveAnimator>(game: &mut Game, unit_id: UnitID, dest: Location, ui: &mut U) {
    ui.log_message(format!("Destination 1: {}", dest));
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

    ui.log_message(format!("Destination 3: {}", dest));
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

    ui.log_message(format!("Destination 4: {}", dest));
    //
    // let dest = {
    //     let unit = game.unit(src).unwrap();
    //     nearest_reachable_adjacent_unobserved(game, src, &unit, game.wrapping())
    // };

    match game.move_unit_by_id(unit_id, dest) {
        Ok(move_result) => {
            ui.animate_move(game, &move_result);

            if move_result.moved_successfully() && move_result.unit().moves_remaining > 0 {
                game.give_orders(unit_id, None, ui).unwrap();
            }
        },
        Err(msg) => {
            ui.log_message(format!("Error moving unit toward {}: {}", dest, msg));
        }
    }
}
