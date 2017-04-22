//! Shortest path algorithm

use std::cmp::Ordering;
use std::collections::{BinaryHeap,HashSet};
// use std::u16::MAX as u16_max;
use std::fmt;
use std::ops::{Index,IndexMut};

use game::Game;
use game::obs::Obs;
use map::{LocationGrid,Terrain,Tile};
use unit::{Unit,UnitType};
use util::{Dims,Location,Vec2d,Wrap2d,wrapped_add};

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
pub static RELATIVE_NEIGHBORS_CARDINAL: [Vec2d<i32>; 4] = [
    Vec2d { x: -1, y:  0 },
    Vec2d { x:  0, y: -1 },
    Vec2d { x:  0, y:  1 },
    Vec2d { x:  1, y:  0 }
];
pub static RELATIVE_NEIGHBORS_DIAGONAL: [Vec2d<i32>; 4] = [
    Vec2d { x: -1, y: -1 },
    Vec2d { x: -1, y:  1 },
    Vec2d { x:  1, y: -1 },
    Vec2d { x:  1, y:  1 }
];

pub trait NeighbFilter : Filter<Tile> {}

struct UnitMovementFilter<'a> {
    unit: &'a Unit
}
impl <'a> Filter<Tile> for UnitMovementFilter<'a> {
    fn include(&self, neighb_tile: &Tile) -> bool {
        self.unit.can_move_on_tile(neighb_tile)
    }
}
pub struct TerrainFilter {
    pub terrain: Terrain
}
impl Filter<Tile> for TerrainFilter {
    fn include(&self, neighb_tile: &Tile) -> bool {
        self.terrain == neighb_tile.terrain
    }
}

pub trait Source<T> {
    fn get(&self, loc: Location) -> Option<&T>;
    fn dims(&self) -> Dims;
}
pub trait Filter<T> {
    fn include(&self, item: &T) -> bool;
}

impl Source<Tile> for Game {
    fn get(&self, loc: Location) -> Option<&Tile> {
        self.current_player_tile(loc)
    }
    fn dims(&self) -> Dims {
        self.map_dims()
    }
}

impl Source<Tile> for LocationGrid<Tile> {
    fn get(&self, loc: Location) -> Option<&Tile> {
        self.get(loc)
    }
    fn dims(&self) -> Dims {
        self.dims()
    }
}

#[allow(dead_code)]
struct UnobservedFilter {}
impl Filter<Obs> for UnobservedFilter {
    fn include(&self, obs: &Obs) -> bool {
        *obs == Obs::Unobserved
    }
}
struct ObservedFilter {}
impl Filter<Obs> for ObservedFilter {
    fn include(&self, obs: &Obs) -> bool {
        if let Obs::Observed{tile:_, turn:_} = *obs {
            true
        } else {
            false
        }
    }
}

pub fn neighbors<'a, T, F, N, S>(tiles: &S, loc: Location, rel_neighbs: N,
                                 filter: &F, wrapping: Wrap2d) -> HashSet<Location>
    where F:Filter<T>, S:Source<T>, N:Iterator<Item=&'a Vec2d<i32>> {

    let mut neighbs = HashSet::new();
    for rel_neighb in rel_neighbs.into_iter() {
        if let Some(neighb_loc) = wrapped_add(loc, *rel_neighb, tiles.dims(), wrapping) {
            if let Some(tile) = tiles.get(neighb_loc) {
                if filter.include(tile) {
                    neighbs.insert(neighb_loc);
                }
            }
        }
    }

    neighbs
}

struct UnitTypeFilter {
    unit_type: UnitType
}
impl Filter<Tile> for UnitTypeFilter {
    fn include(&self, neighb_tile: &Tile) -> bool {
        self.unit_type.can_move_on_terrain(&neighb_tile.terrain)
    }
}
pub fn neighbors_terrain_only<T:Source<Tile>>(tiles: &T, loc: Location, unit_type: UnitType, wrapping: Wrap2d) -> HashSet<Location> {
    neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &UnitTypeFilter{unit_type: unit_type}, wrapping)
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
pub fn shortest_paths<T:Source<Tile>>(tiles: &T, source: Location, unit: &Unit, wrapping: Wrap2d) -> ShortestPaths {
    let mut q = BinaryHeap::new();

    let mut dist = LocationGrid::new(tiles.dims(), |_loc| None);
    let mut prev = LocationGrid::new(tiles.dims(), |_loc| None);

    q.push(State{ dist_: 0, loc: source });

    dist[source] = Some(0);

    while let Some(State{ dist_, loc }) = q.pop() {

        // Quit early since we're already doing worse than the best known route
        if dist[loc].is_some() && dist_ > dist[loc].unwrap() { continue; }

        for neighb_loc in neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &UnitMovementFilter{unit: unit}, wrapping) {
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

    use std::collections::HashSet;
    use std::convert::TryFrom;

    use map::{LocationGrid,Tile};
    use map::dijkstra::{Source,UnitMovementFilter,neighbors,neighbors_terrain_only,shortest_paths,RELATIVE_NEIGHBORS};
    use unit::{Alignment,Unit,UnitType};
    use util::{Location,Wrap2d,WRAP_BOTH,WRAP_HORIZ,WRAP_VERT,WRAP_NEITHER};

    fn neighbors_all_unit<T:Source<Tile>>(tiles: &T, loc: Location, unit: &Unit, wrapping: Wrap2d) -> HashSet<Location> {
        neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &UnitMovementFilter{unit:unit}, wrapping)
    }

    #[test]
    fn test_neighbors_terrain_only() {
        let map = LocationGrid::try_from("*xx\n\
                                          x x\n\
                                          xxx").unwrap();

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
    }

    #[test]
    fn test_neighbors() {
        let map = LocationGrid::try_from("*xx\n\
                                          x x\n\
                                          xxx").unwrap();

        let loc = Location{x:0, y:2};
        let infantry = Unit::new(UnitType::Infantry, Alignment::Belligerent{player:0}, "Irving Harrison");
        let neighbs_both = neighbors_all_unit(&map, loc, &infantry, WRAP_BOTH);
        assert!(neighbs_both.contains(&Location{x:0, y:0}));
        assert!(neighbs_both.contains(&Location{x:0, y:1}));
        assert!(neighbs_both.contains(&Location{x:1, y:0}));
        assert!(neighbs_both.contains(&Location{x:1, y:2}));
        assert!(neighbs_both.contains(&Location{x:2, y:0}));
        assert!(neighbs_both.contains(&Location{x:2, y:1}));
        assert!(neighbs_both.contains(&Location{x:2, y:2}));

        let neighbs_horiz = neighbors_all_unit(&map, loc, &infantry, WRAP_HORIZ);
        assert!(!neighbs_horiz.contains(&Location{x:0, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:0, y:1}));
        assert!(!neighbs_horiz.contains(&Location{x:1, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:1, y:2}));
        assert!(!neighbs_horiz.contains(&Location{x:2, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:2, y:1}));
        assert!( neighbs_horiz.contains(&Location{x:2, y:2}));

        let neighbs_vert = neighbors_all_unit(&map, loc, &infantry, WRAP_VERT);
        assert!( neighbs_vert.contains(&Location{x:0, y:0}));
        assert!( neighbs_vert.contains(&Location{x:0, y:1}));
        assert!( neighbs_vert.contains(&Location{x:1, y:0}));
        assert!( neighbs_vert.contains(&Location{x:1, y:2}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:0}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:1}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:2}));

        let neighbs_neither = neighbors_all_unit(&map, loc, &infantry, WRAP_NEITHER);
        assert!(!neighbs_neither.contains(&Location{x:0, y:0}));
        assert!( neighbs_neither.contains(&Location{x:0, y:1}));
        assert!(!neighbs_neither.contains(&Location{x:1, y:0}));
        assert!( neighbs_neither.contains(&Location{x:1, y:2}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:0}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:1}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:2}));
    }

    #[test]
    fn test_dijkstra() {
        let map = LocationGrid::try_from(
    "\
    xxx\n\
    x x\n\
    *xx").unwrap();

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
