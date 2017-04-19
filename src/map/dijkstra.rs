//! Shortest path algorithm

use std::cmp::Ordering;
use std::collections::{BinaryHeap,HashSet};
// use std::u16::MAX as u16_max;
use std::fmt;
use std::ops::{Index,IndexMut};

use game::Game;
use map::{LocationGrid,Tile};
use unit::{Unit,UnitType};
use util::{Dims,Location,Vec2d,Wrap2d,wrapped_add};

pub trait TileSource {
    fn get(&self, loc: Location) -> Option<&Tile>;
    fn dims(&self) -> Dims;
}

impl TileSource for Game {
    fn get(&self, loc: Location) -> Option<&Tile> {
        self.current_player_tile(loc)
    }
    fn dims(&self) -> Dims {
        self.map_dims()
    }
}

impl TileSource for LocationGrid<Tile> {
    fn get(&self, loc: Location) -> Option<&Tile> {
        self.get(loc)
    }
    fn dims(&self) -> Dims {
        self.dims
    }
}

impl Index<Location> for Vec<Vec<u16>> {
    type Output = u16;
    fn index<'a>(&'a self, location: Location) -> &'a Self::Output {
        &self[location.x as usize][location.y as usize]
    }
}

impl IndexMut<Location> for Vec<Vec<u16>> {
    fn index_mut<'a>(&'a mut self, location: Location) -> &'a mut u16 {
        let col:  &mut Vec<u16> = self.get_mut(location.x as usize).unwrap();
        col.get_mut(location.y as usize).unwrap()
    }
}

pub struct ShortestPaths {
    pub dist: LocationGrid<Option<u16>>,
    pub prev: LocationGrid<Option<Location>>
}

impl fmt::Debug for ShortestPaths {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Distances:\n{:?}\nPrevious Nodes:\n{:?}", self.dist, self.prev)
    }
}

impl ShortestPaths {
    pub fn shortest_path(&self, dest: Location) -> Vec<Location> {
        let mut path = Vec::new();

        path.insert(0, dest);

        let mut most_recent = dest;

        loop {
            if let Some(prev) = self.prev[most_recent] {
                path.insert(0, prev);
                most_recent = prev;
            } else {
                break;
            }
        }

        path
    }
}

pub static RELATIVE_NEIGHBORS: [Vec2d<i32>; 8] = [
    Vec2d { x: -1, y: -1 },
    Vec2d { x: -1, y:  0 },
    Vec2d { x: -1, y:  1 },
    Vec2d { x:  0, y: -1 },
    Vec2d { x:  0, y:  1 },
    Vec2d { x:  1, y: -1 },
    Vec2d { x:  1, y:  0 },
    Vec2d { x:  1, y:  1}
];

pub fn neighbors<T:TileSource>(tiles: &T, loc: Location, unit: &Unit, wrapping: Wrap2d) -> HashSet<Location> {
    let mut neighbs = HashSet::new();
    for rel_neighb in RELATIVE_NEIGHBORS.iter() {
        if let Some(neighb_loc) = wrapped_add(loc, *rel_neighb, tiles.dims(), wrapping) {
            if let Some(tile) = tiles.get(neighb_loc) {
                if unit.can_move_on_tile(&tile) {
                    neighbs.insert(neighb_loc);
                }
            }
        }
    }

    neighbs
}

pub fn neighbors_terrain_only<T:TileSource>(tiles: &T, loc: Location, unit_type: UnitType, wrapping: Wrap2d) -> HashSet<Location> {
    let mut neighbs = HashSet::new();
    for rel_neighb in RELATIVE_NEIGHBORS.iter() {
        if let Some(neighb_loc) = wrapped_add(loc, *rel_neighb, tiles.dims(), wrapping) {
            if let Some(tile) = tiles.get(neighb_loc) {
                if unit_type.can_move_on_terrain(&tile.terrain) {
                    neighbs.insert(neighb_loc);
                }
            }
        }
    }

    neighbs
}

#[derive(Eq,PartialEq)]
struct State {
    dist_: u16,
    loc: Location
}

impl Ord for State {
    fn cmp(&self, other: &State) -> Ordering {
        other.dist_.cmp(&self.dist_)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &State) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// An implementation of Dijkstra's algorithm.
///
/// Finds all paths emanating from a single source location that could be traversed by the
/// referenced unit. The returned `ShortestPaths` object can then be queried for the shortest path
/// to any particular destination.
///
/// The provided wrapping strategy is respected.
pub fn shortest_paths<T:TileSource>(tiles: &T, source: Location, unit: &Unit, wrapping: Wrap2d) -> ShortestPaths {
    let mut q = BinaryHeap::new();

    let mut dist = LocationGrid::new(tiles.dims(), |_loc| None);
    let mut prev = LocationGrid::new(tiles.dims(), |_loc| None);

    q.push(State{ dist_: 0, loc: source });

    dist[source] = Some(0);

    while let Some(State{ dist_, loc }) = q.pop() {

        // Quit early since we're already doing worse than the best known route
        if dist[loc].is_some() && dist_ > dist[loc].unwrap() { continue; }

        // for neighb_loc in neighbors_with_same_terrain(tiles, &loc, wrapping) {
        for neighb_loc in neighbors(tiles, loc, unit, wrapping) {
            let new_dist = dist_ + 1;
            let next = State { dist_: new_dist, loc: neighb_loc };

            // If the new route to the neighbor is better than any we've found so far...
            if dist[neighb_loc].is_none() || new_dist < dist[neighb_loc].unwrap() {
                q.push(next);
                // Relaxation, we have now found a better way
                dist[neighb_loc] = Some(new_dist);
                prev[neighb_loc] = Some(loc);
            }
        }
    }

    ShortestPaths { dist: dist, prev: prev }
}

#[cfg(test)]
mod test {

    use std::convert::TryFrom;

    use map::LocationGrid;
    use map::dijkstra::{neighbors,neighbors_terrain_only,shortest_paths};
    use unit::{Alignment,Unit,UnitType};
    use util::{Location,WRAP_BOTH,WRAP_HORIZ,WRAP_VERT,WRAP_NEITHER};


    #[test]
    fn test_neighbors_terrain_only() {
        if let Ok(map) = LocationGrid::try_from("*xx\n\
                                                 x x\n\
                                                 xxx") {

            let loc = Location{x:0, y:2};

            let neighbs_both = neighbors_terrain_only(&map, loc, UnitType::Infantry, WRAP_BOTH);
            assert!(neighbs_both.contains(&Location{x:0, y:0}));
            assert!(neighbs_both.contains(&Location{x:0, y:1}));
            assert!(neighbs_both.contains(&Location{x:1, y:0}));
            assert!(neighbs_both.contains(&Location{x:1, y:2}));
            assert!(neighbs_both.contains(&Location{x:2, y:0}));
            assert!(neighbs_both.contains(&Location{x:2, y:1}));
            assert!(neighbs_both.contains(&Location{x:2, y:2}));

            let neighbs_horiz = neighbors_terrain_only(&map, loc, UnitType::Infantry, WRAP_HORIZ);
            assert!(!neighbs_horiz.contains(&Location{x:0, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:0, y:1}));
            assert!(!neighbs_horiz.contains(&Location{x:1, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:1, y:2}));
            assert!(!neighbs_horiz.contains(&Location{x:2, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:2, y:1}));
            assert!( neighbs_horiz.contains(&Location{x:2, y:2}));

            let neighbs_vert = neighbors_terrain_only(&map, loc, UnitType::Infantry, WRAP_VERT);
            assert!( neighbs_vert.contains(&Location{x:0, y:0}));
            assert!( neighbs_vert.contains(&Location{x:0, y:1}));
            assert!( neighbs_vert.contains(&Location{x:1, y:0}));
            assert!( neighbs_vert.contains(&Location{x:1, y:2}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:0}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:1}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:2}));

            let neighbs_neither = neighbors_terrain_only(&map, loc, UnitType::Infantry, WRAP_NEITHER);
            assert!(!neighbs_neither.contains(&Location{x:0, y:0}));
            assert!( neighbs_neither.contains(&Location{x:0, y:1}));
            assert!(!neighbs_neither.contains(&Location{x:1, y:0}));
            assert!( neighbs_neither.contains(&Location{x:1, y:2}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:0}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:1}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:2}));
        } else {
            assert!(false, "This map should have converted to a LocationGrid<Tile>");
        }
    }

    #[test]
    fn test_neighbors() {
        if let Ok(map) = LocationGrid::try_from("*xx\n\
                                                 x x\n\
                                                 xxx") {

            let loc = Location{x:0, y:2};
            let infantry = Unit::new(UnitType::Infantry, Alignment::Belligerent{player:0}, "Irving Harrison");
            let neighbs_both = neighbors(&map, loc, &infantry, WRAP_BOTH);
            assert!(neighbs_both.contains(&Location{x:0, y:0}));
            assert!(neighbs_both.contains(&Location{x:0, y:1}));
            assert!(neighbs_both.contains(&Location{x:1, y:0}));
            assert!(neighbs_both.contains(&Location{x:1, y:2}));
            assert!(neighbs_both.contains(&Location{x:2, y:0}));
            assert!(neighbs_both.contains(&Location{x:2, y:1}));
            assert!(neighbs_both.contains(&Location{x:2, y:2}));

            let neighbs_horiz = neighbors(&map, loc, &infantry, WRAP_HORIZ);
            assert!(!neighbs_horiz.contains(&Location{x:0, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:0, y:1}));
            assert!(!neighbs_horiz.contains(&Location{x:1, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:1, y:2}));
            assert!(!neighbs_horiz.contains(&Location{x:2, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:2, y:1}));
            assert!( neighbs_horiz.contains(&Location{x:2, y:2}));

            let neighbs_vert = neighbors(&map, loc, &infantry, WRAP_VERT);
            assert!( neighbs_vert.contains(&Location{x:0, y:0}));
            assert!( neighbs_vert.contains(&Location{x:0, y:1}));
            assert!( neighbs_vert.contains(&Location{x:1, y:0}));
            assert!( neighbs_vert.contains(&Location{x:1, y:2}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:0}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:1}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:2}));

            let neighbs_neither = neighbors(&map, loc, &infantry, WRAP_NEITHER);
            assert!(!neighbs_neither.contains(&Location{x:0, y:0}));
            assert!( neighbs_neither.contains(&Location{x:0, y:1}));
            assert!(!neighbs_neither.contains(&Location{x:1, y:0}));
            assert!( neighbs_neither.contains(&Location{x:1, y:2}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:0}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:1}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:2}));
        } else {
            assert!(false, "This map should have converted to a LocationGrid<Tile>");
        }
    }

    #[test]
    fn test_dijkstra() {
        match LocationGrid::try_from(
    "\
    xxx\n\
    x x\n\
    *xx") {
            Err(_) => {
                assert!(false, "This map should have converted to a LocationGrid<Tile>");
            },
            Ok(map) => {
                let loc = Location{x:0, y:0};
                let infantry = Unit::new(UnitType::Infantry, Alignment::Belligerent{player:0}, "Carmen Bentley");
                let shortest_neither = shortest_paths(&map, loc, &infantry, WRAP_NEITHER);
                println!("{:?}", shortest_neither);
                assert_eq!(shortest_neither.dist[Location{x:0, y:0}], Some(0));
                assert_eq!(shortest_neither.dist[Location{x:1, y:0}], Some(1));
                assert_eq!(shortest_neither.dist[Location{x:2, y:0}], Some(2));

                assert_eq!(shortest_neither.dist[Location{x:0, y:1}], Some(1));
                assert_eq!(shortest_neither.dist[Location{x:1, y:1}], None);
                assert_eq!(shortest_neither.dist[Location{x:2, y:1}], Some(2));

                assert_eq!(shortest_neither.dist[Location{x:0, y:2}], Some(2));
                assert_eq!(shortest_neither.dist[Location{x:1, y:2}], Some(2));
                assert_eq!(shortest_neither.dist[Location{x:2, y:2}], Some(3));


                let shortest_horiz = shortest_paths(&map, loc, &infantry, WRAP_HORIZ);
                println!("{:?}", shortest_horiz);
                assert_eq!(shortest_horiz.dist[Location{x:0, y:0}], Some(0));
                assert_eq!(shortest_horiz.dist[Location{x:1, y:0}], Some(1));
                assert_eq!(shortest_horiz.dist[Location{x:2, y:0}], Some(1));

                assert_eq!(shortest_horiz.dist[Location{x:0, y:1}], Some(1));
                assert_eq!(shortest_horiz.dist[Location{x:1, y:1}], None);
                assert_eq!(shortest_horiz.dist[Location{x:2, y:1}], Some(1));

                assert_eq!(shortest_horiz.dist[Location{x:0, y:2}], Some(2));
                assert_eq!(shortest_horiz.dist[Location{x:1, y:2}], Some(2));
                assert_eq!(shortest_horiz.dist[Location{x:2, y:2}], Some(2));

                let shortest_vert = shortest_paths(&map, loc, &infantry, WRAP_VERT);
                assert_eq!(shortest_vert.dist[Location{x:0, y:0}], Some(0));
                assert_eq!(shortest_vert.dist[Location{x:1, y:0}], Some(1));
                assert_eq!(shortest_vert.dist[Location{x:2, y:0}], Some(2));

                assert_eq!(shortest_vert.dist[Location{x:0, y:1}], Some(1));
                assert_eq!(shortest_vert.dist[Location{x:1, y:1}], None);
                assert_eq!(shortest_vert.dist[Location{x:2, y:1}], Some(2));

                assert_eq!(shortest_vert.dist[Location{x:0, y:2}], Some(1));
                assert_eq!(shortest_vert.dist[Location{x:1, y:2}], Some(1));
                assert_eq!(shortest_vert.dist[Location{x:2, y:2}], Some(2));

                let shortest_both = shortest_paths(&map, loc, &infantry, WRAP_BOTH);
                assert_eq!(shortest_both.dist[Location{x:0, y:0}], Some(0));
                assert_eq!(shortest_both.dist[Location{x:1, y:0}], Some(1));
                assert_eq!(shortest_both.dist[Location{x:2, y:0}], Some(1));

                assert_eq!(shortest_both.dist[Location{x:0, y:1}], Some(1));
                assert_eq!(shortest_both.dist[Location{x:1, y:1}], None);
                assert_eq!(shortest_both.dist[Location{x:2, y:1}], Some(1));

                assert_eq!(shortest_both.dist[Location{x:0, y:2}], Some(1));
                assert_eq!(shortest_both.dist[Location{x:1, y:2}], Some(1));
                assert_eq!(shortest_both.dist[Location{x:2, y:2}], Some(1));
            }
        }
    }
}
