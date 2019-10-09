//! Shortest path algorithm

use std::{
    cmp::Ordering,
    collections::{
        BinaryHeap,
        HashSet,
        VecDeque,
    },
    fmt,
    marker::PhantomData,
    ops::{
        Index,
        IndexMut,
    },
};

use crate::{
    game::{
        Alignment,
        map::{LocationGrid,Terrain,Tile},
        obs::Obs,
        unit::{Unit,UnitType},
    },
    util::{Dims,Location,Vec2d,Wrap2d},
};

impl Index<Location> for Vec<Vec<u16>> {
    type Output = u16;
    fn index(&self, location: Location) -> &Self::Output {
        &self[location.x as usize][location.y as usize]
    }
}

impl IndexMut<Location> for Vec<Vec<u16>> {
    fn index_mut(&mut self, location: Location) -> &mut u16 {
        let col: &mut Vec<u16> = &mut self[location.x as usize];
        &mut col[location.y as usize]
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

        while let Some(prev) = self.prev[most_recent] {
            path.insert(0, prev);
            most_recent = prev;
        }

        path
    }
}

const REL_NEIGHB_TOP:          Vec2d<i32> = Vec2d::new( 1,  0);
const REL_NEIGHB_BOTTOM:       Vec2d<i32> = Vec2d::new(-1,  0);
const REL_NEIGHB_LEFT:         Vec2d<i32> = Vec2d::new( 0, -1);
const REL_NEIGHB_RIGHT:        Vec2d<i32> = Vec2d::new( 0,  1);
const REL_NEIGHB_TOP_LEFT:     Vec2d<i32> = Vec2d::new( 1, -1);
const REL_NEIGHB_TOP_RIGHT:    Vec2d<i32> = Vec2d::new( 1,  1);
const REL_NEIGHB_BOTTOM_LEFT:  Vec2d<i32> = Vec2d::new(-1, -1);
const REL_NEIGHB_BOTTOM_RIGHT: Vec2d<i32> = Vec2d::new(-1,  1);

pub static RELATIVE_NEIGHBORS: [Vec2d<i32>; 8] = [
    REL_NEIGHB_TOP, REL_NEIGHB_BOTTOM, REL_NEIGHB_LEFT, REL_NEIGHB_RIGHT,
    REL_NEIGHB_TOP_LEFT, REL_NEIGHB_TOP_RIGHT, REL_NEIGHB_BOTTOM_LEFT, REL_NEIGHB_BOTTOM_RIGHT,
];
pub static RELATIVE_NEIGHBORS_CARDINAL: [Vec2d<i32>; 4] = [
    REL_NEIGHB_TOP, REL_NEIGHB_BOTTOM, REL_NEIGHB_LEFT, REL_NEIGHB_RIGHT,
];
pub static RELATIVE_NEIGHBORS_DIAGONAL: [Vec2d<i32>; 4] = [
    REL_NEIGHB_TOP_LEFT, REL_NEIGHB_TOP_RIGHT, REL_NEIGHB_BOTTOM_LEFT, REL_NEIGHB_BOTTOM_RIGHT,
];

pub trait NeighbFilter : Filter<Tile> {}

pub struct UnitMovementFilter<'a> {
    pub unit: &'a Unit
}
impl <'a> UnitMovementFilter<'a> {
    pub fn new(unit: &'a Unit) -> Self {
        UnitMovementFilter {
            unit
        }
    }
}
impl <'a> Filter<Tile> for UnitMovementFilter<'a> {
    fn include(&self, neighb_tile: &Tile) -> bool {
        self.unit.can_move_on_tile(neighb_tile)
    }
}
impl <'a> Filter<Obs> for UnitMovementFilter<'a> {
    fn include(&self, obs: &Obs) -> bool {
        if let Obs::Observed{tile,..} = obs {
            UnitMovementFilter::include(self, tile)
        } else {
            false
        }
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

// pub trait OwnedSource<T> {
//     fn get(&self, loc: Location) -> Option<T>;
//     fn dims(&self) -> Dims;
// }

pub trait Source<T> {
    fn get(&self, loc: Location) -> &T;
    fn dims(&self) -> Dims;
}
pub trait Filter<T> {
    fn include(&self, item: &T) -> bool;
}

pub struct AndFilter<T,F1,F2> where F1:Filter<T>,F2:Filter<T> {
    filter1: F1,
    filter2: F2,
    phantom: PhantomData<T>,
}
impl <T,F1:Filter<T>,F2:Filter<T>> AndFilter<T,F1,F2> {
    pub fn new(filter1: F1, filter2: F2) -> Self {
        Self { filter1, filter2, phantom: PhantomData }
    }
}
impl <T,F1:Filter<T>,F2:Filter<T>> Filter<T> for AndFilter<T,F1,F2> {
    fn include(&self, item: &T) -> bool {
        if !self.filter1.include(item) {
            return false;
        }
        self.filter2.include(item)
    }
}

// struct OrFilter<T,F1,F2> where F1:Filter<T>,F2:Filter<T> {
//     filter1: F1,
//     filter2: F2,
//     phantom: PhantomData<T>,
// }
// impl <T,F1:Filter<T>,F2:Filter<T>> OrFilter<T,F1,F2> {
//     fn new(filter1: F1, filter2: F2) -> Self {
//         Self {
//             filter1,
//             filter2,
//             phantom: PhantomData,
//         }
//     }
// }
// impl <T,F1:Filter<T>,F2:Filter<T>> Filter<T> for OrFilter<T,F1,F2> {
//     fn include(&self, item: &T) -> bool {
//         if self.filter1.include(item) {
//             return true;
//         }
//         self.filter2.include(item)

//     }
// }

pub struct All;
impl <T> Filter<T> for All {
    fn include(&self, _item: &T) -> bool {
        true
    }
}

pub struct ObservedFilter;
impl Filter<Obs> for ObservedFilter {
    fn include(&self, obs: &Obs) -> bool {
        obs.is_observed()
    }
}

pub struct UnobservedFilter;
impl Filter<Obs> for UnobservedFilter {
    fn include(&self, obs: &Obs) -> bool {
        obs.is_unobserved()
    }
}



pub struct NoUnitsFilter;
impl Filter<Tile> for NoUnitsFilter {
    fn include(&self, tile: &Tile) -> bool {
        tile.unit.is_none()
    }
}
impl Filter<Obs> for NoUnitsFilter {
    fn include(&self, obs: &Obs) -> bool {
        if let Obs::Observed { tile, .. } = obs {
            Filter::<Tile>::include(self, tile)
        } else {
            // NOTE: This is a misleading response---if the tile isn't observed, we can't tell if it has a unit or not
            // However, in practice this shouldn't be an issue as we should always observe it prior to reaching it
            true
        }
    }
}

pub struct NoCitiesButOursFilter {
    pub alignment: Alignment
}
impl Filter<Tile> for NoCitiesButOursFilter {
    fn include(&self, tile: &Tile) -> bool {
        if let Some(ref city) = tile.city {
            return city.alignment == self.alignment;
        }
        true
    }
}
impl Filter<Obs> for NoCitiesButOursFilter {
    fn include(&self, obs: &Obs) -> bool {
        if let Obs::Observed { tile, .. } = obs {
            if let Some(ref city) = tile.city {
                return city.alignment == self.alignment;
            }
        }

        // NOTE: This is a misleading response---if the tile isn't observed, we can't tell if it has a
        // city not belonging to this player. However, in practice this shouldn't be an issue as we should always
        // observe the tile prior to reaching it.
        true
    }
}

/// A filter that _always_ includes unobserved tiles. Otherwise it defers to a sub-filter.
pub struct Xenophile<F:Filter<Obs>> {
    sub_filter: F
}
impl <F:Filter<Obs>> Xenophile<F> {
    pub fn new(sub_filter: F) -> Self {
        Xenophile {
            sub_filter
        }
    }
}
impl <F:Filter<Obs>> Filter<Obs> for Xenophile<F> {
    fn include(&self, obs: &Obs) -> bool {
        // if obs.is_some() {
        //     self.sub_filter.include(obs)
        // } else {
        //     true
        // }
        if *obs == Obs::Unobserved {
            true
        } else {
            self.sub_filter.include(obs)
        }
    }
}

pub fn neighbors<'a, T, F, N, S>(tiles: &S, loc: Location, rel_neighbs: N,
                                 filter: &F, wrapping: Wrap2d) -> HashSet<Location>
    where F:Filter<T>, S:Source<T>, N:Iterator<Item=&'a Vec2d<i32>> {

    let mut neighbs = HashSet::new();
    for rel_neighb in rel_neighbs {
        if let Some(neighb_loc) = wrapping.wrapped_add(tiles.dims(), loc, *rel_neighb) {
            if filter.include(tiles.get(neighb_loc))  {
                neighbs.insert(neighb_loc);
            }
            // if let Some(tile) = tiles.get(neighb_loc) {
            //     if filter.include(tile) {
            //         neighbs.insert(neighb_loc);
            //     }
            // }
        }
    }

    neighbs
}

pub fn has_neighbor<'a, T, F, N, S>(tiles: &S, loc: Location, rel_neighbs: N,
                                 filter: &F, wrapping: Wrap2d) -> bool
    where F:Filter<T>, S:Source<T>, N:Iterator<Item=&'a Vec2d<i32>> {

    for rel_neighb in rel_neighbs {
        if let Some(neighb_loc) = wrapping.wrapped_add(tiles.dims(), loc, *rel_neighb) {
            if filter.include(tiles.get(neighb_loc))  {
                return true;
            }
        }
    }

    false
}

pub fn neighbors_iter<'a, T, F, N, S>(tiles: &'a S, loc: Location, rel_neighbs: N,
                                 filter: &'a F, wrapping: Wrap2d) -> impl Iterator<Item=Location> + 'a
    where F:Filter<T>, S:Source<T>, N:Iterator<Item=&'a Vec2d<i32>>+'a {

        rel_neighbs.filter_map(move |rel_neighb| wrapping.wrapped_add(tiles.dims(), loc, *rel_neighb))
                   .filter(move |neighb_loc| filter.include(tiles.get(*neighb_loc)))
}

struct UnitTypeFilter {
    unit_type: UnitType
}
impl Filter<Tile> for UnitTypeFilter {
    fn include(&self, neighb_tile: &Tile) -> bool {
        self.unit_type.can_move_on_tile(neighb_tile)
    }
}
pub fn neighbors_terrain_only<T:Source<Tile>>(tiles: &T, loc: Location, unit_type: UnitType, wrapping: Wrap2d) -> HashSet<Location> {
    neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &UnitTypeFilter{unit_type}, wrapping)
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
/// Finds all paths emanating from a single source location that could be traversed by accessing the nodes included
/// by the filter `filter`. The returned `ShortestPaths` object can then be queried for the shortest path
/// to any particular destination.
///
/// The provided wrapping strategy is respected.
pub fn shortest_paths<T,F:Filter<T>,S:Source<T>>(tiles: &S, source: Location, filter: &F, wrapping: Wrap2d) -> ShortestPaths {
    let mut q = BinaryHeap::new();

    let mut dist = LocationGrid::new(tiles.dims(), |_loc| None);
    let mut prev = LocationGrid::new(tiles.dims(), |_loc| None);

    q.push(State{ dist_: 0, loc: source });

    dist[source] = Some(0);

    while let Some(State{ dist_, loc }) = q.pop() {

        // Quit early since we're already doing worse than the best known route
        // if dist[loc].is_some() && dist_ > dist[loc].unwrap() { continue; }

        if let Some(dist) = dist[loc] {
            if dist_ > dist {
                continue;
            }
        }

        // for neighb_loc in neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), filter, wrapping) {
        for neighb_loc in neighbors_iter(tiles, loc, RELATIVE_NEIGHBORS.iter(), filter, wrapping) {
            let new_dist = dist_ + 1;
            let next = State { dist_: new_dist, loc: neighb_loc };

            // If the new route to the neighbor is better than any we've found so far...
            // if let Some(best_dist_so_far) = dist[neighb_loc] {
            //     if new_dist < best_dist_so_far {
            //         q.push(next);
            //         // Relaxation, we have now found a better way
            //         dist[neighb_loc] = Some(new_dist);
            //         prev[neighb_loc] = Some(loc);
            //     }
            // }
            if dist[neighb_loc].is_none() || new_dist < dist[neighb_loc].unwrap() {
                q.push(next);
                // Relaxation, we have now found a better way
                dist[neighb_loc] = Some(new_dist);
                prev[neighb_loc] = Some(loc);
            }
        }
    }

    ShortestPaths { dist, prev }
}

pub fn old_shortest_paths_for_unit<T:Source<Tile>>(tiles: &T, source: Location, unit: &Unit, wrapping: Wrap2d) -> ShortestPaths {
    shortest_paths(tiles, source, &UnitMovementFilter{unit}, wrapping)
}

// fn observed_no_units_no_cities_but_ours(alignment: Alignment) -> impl Filter<Obs> {
//     AndFilter::new(
//         AndFilter::new(
//             ObservedFilter{},
//             NoUnitsFilter{},
//         ),
//         NoCitiesButOursFilter{alignment},
//     )
// }

/// Return the (or a) closest tile to the source which is reachable by the given
/// unit and is adjacent to at least one unobserved tile. If no such tile exists
/// then return None
pub fn nearest_adjacent_unobserved_reachable_without_attacking<S:Source<Obs>>(
    tiles: &S,
    src: Location,
    unit: &Unit,
    wrapping: Wrap2d
) -> Option<Location> {

    let observed_and_reachable_filter = AndFilter::new(
        ObservedFilter,
        AndFilter::new(
            AndFilter::new(
                NoUnitsFilter{},
                NoCitiesButOursFilter{alignment: unit.alignment }
            ),
            UnitMovementFilter{unit}
        )
    );

    let mut q = VecDeque::new();
    q.push_back(src);

    let mut visited = LocationGrid::new(tiles.dims(), |loc| loc==src);

    while let Some(loc) = q.pop_front() {
        visited[loc] = true;

        let unobserved_neighbor_exists = has_neighbor(tiles, loc, RELATIVE_NEIGHBORS.iter(), &UnobservedFilter{}, wrapping);
        if unobserved_neighbor_exists {
            return Some(loc);
        }

        for neighb in neighbors_iter(tiles, loc, RELATIVE_NEIGHBORS.iter(), &observed_and_reachable_filter, wrapping)
            .filter(|neighb| !visited[*neighb])
        {
            q.push_back(neighb);
        }
    }
    None
}

#[cfg(test)]
mod test {

    use std::{
        collections::HashSet,
        convert::TryFrom,
    };

    use crate::{
        game::{
            Alignment,
            map::{
                LocationGrid,
                Tile,
                terrain::Terrain,
            },
            obs::Obs,
            unit::{UnitID,Unit,UnitType},
        },
        
        util::{Dims,Location,Wrap2d},
    };

    use super::{
        All,
        Source,
        UnitMovementFilter,
        Xenophile,
        nearest_adjacent_unobserved_reachable_without_attacking,
        neighbors,
        neighbors_terrain_only,
        old_shortest_paths_for_unit,
        shortest_paths,
        RELATIVE_NEIGHBORS,
    };
    
    fn neighbors_all_unit<T:Source<Tile>>(tiles: &T, loc: Location, unit: &Unit, wrapping: Wrap2d) -> HashSet<Location> {
        neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &UnitMovementFilter{unit}, wrapping)
    }

    #[test]
    fn test_neighbors_terrain_only() {
        let map = LocationGrid::try_from("*xx\n\
                                          x x\n\
                                          xxx").unwrap();

        let loc = Location{x:0, y:2};

        let neighbs_both = neighbors_terrain_only(&map, loc, UnitType::Infantry, Wrap2d::BOTH);
        assert!(neighbs_both.contains(&Location{x:0, y:0}));
        assert!(neighbs_both.contains(&Location{x:0, y:1}));
        assert!(neighbs_both.contains(&Location{x:1, y:0}));
        assert!(neighbs_both.contains(&Location{x:1, y:2}));
        assert!(neighbs_both.contains(&Location{x:2, y:0}));
        assert!(neighbs_both.contains(&Location{x:2, y:1}));
        assert!(neighbs_both.contains(&Location{x:2, y:2}));

        let neighbs_horiz = neighbors_terrain_only(&map, loc, UnitType::Infantry, Wrap2d::HORIZ);
        assert!(!neighbs_horiz.contains(&Location{x:0, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:0, y:1}));
        assert!(!neighbs_horiz.contains(&Location{x:1, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:1, y:2}));
        assert!(!neighbs_horiz.contains(&Location{x:2, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:2, y:1}));
        assert!( neighbs_horiz.contains(&Location{x:2, y:2}));

        let neighbs_vert = neighbors_terrain_only(&map, loc, UnitType::Infantry, Wrap2d::VERT);
        assert!( neighbs_vert.contains(&Location{x:0, y:0}));
        assert!( neighbs_vert.contains(&Location{x:0, y:1}));
        assert!( neighbs_vert.contains(&Location{x:1, y:0}));
        assert!( neighbs_vert.contains(&Location{x:1, y:2}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:0}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:1}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:2}));

        let neighbs_neither = neighbors_terrain_only(&map, loc, UnitType::Infantry, Wrap2d::NEITHER);
        assert!(!neighbs_neither.contains(&Location{x:0, y:0}));
        assert!( neighbs_neither.contains(&Location{x:0, y:1}));
        assert!(!neighbs_neither.contains(&Location{x:1, y:0}));
        assert!( neighbs_neither.contains(&Location{x:1, y:2}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:0}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:1}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:2}));
    }

    #[test]
    fn test_neighbors_all_unit() {
        let map = LocationGrid::try_from("*xx\n\
                                          x x\n\
                                          xxx").unwrap();

        let loc = Location{x:0, y:2};
        let infantry = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "Irving Harrison");
        let neighbs_both = neighbors_all_unit(&map, loc, &infantry, Wrap2d::BOTH);
        assert!(neighbs_both.contains(&Location{x:0, y:0}));
        assert!(neighbs_both.contains(&Location{x:0, y:1}));
        assert!(neighbs_both.contains(&Location{x:1, y:0}));
        assert!(neighbs_both.contains(&Location{x:1, y:2}));
        assert!(neighbs_both.contains(&Location{x:2, y:0}));
        assert!(neighbs_both.contains(&Location{x:2, y:1}));
        assert!(neighbs_both.contains(&Location{x:2, y:2}));

        let neighbs_horiz = neighbors_all_unit(&map, loc, &infantry, Wrap2d::HORIZ);
        assert!(!neighbs_horiz.contains(&Location{x:0, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:0, y:1}));
        assert!(!neighbs_horiz.contains(&Location{x:1, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:1, y:2}));
        assert!(!neighbs_horiz.contains(&Location{x:2, y:0}));
        assert!( neighbs_horiz.contains(&Location{x:2, y:1}));
        assert!( neighbs_horiz.contains(&Location{x:2, y:2}));

        let neighbs_vert = neighbors_all_unit(&map, loc, &infantry, Wrap2d::VERT);
        assert!( neighbs_vert.contains(&Location{x:0, y:0}));
        assert!( neighbs_vert.contains(&Location{x:0, y:1}));
        assert!( neighbs_vert.contains(&Location{x:1, y:0}));
        assert!( neighbs_vert.contains(&Location{x:1, y:2}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:0}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:1}));
        assert!(!neighbs_vert.contains(&Location{x:2, y:2}));

        let neighbs_neither = neighbors_all_unit(&map, loc, &infantry, Wrap2d::NEITHER);
        assert!(!neighbs_neither.contains(&Location{x:0, y:0}));
        assert!( neighbs_neither.contains(&Location{x:0, y:1}));
        assert!(!neighbs_neither.contains(&Location{x:1, y:0}));
        assert!( neighbs_neither.contains(&Location{x:1, y:2}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:0}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:1}));
        assert!(!neighbs_neither.contains(&Location{x:2, y:2}));
    }

    // pub fn neighbors<'a, T, F, N, S>(tiles: &S, loc: Location, rel_neighbs: N,
    //                                  filter: &F, wrapping: Wrap2d) -> HashSet<Location>
    //     where F:Filter<T>, S:Source<T>, N:Iterator<Item=&'a Vec2d<i32>> {
    #[test]
    fn test_neighbors() {//TODO
        let map: LocationGrid<Obs> = LocationGrid::try_from(
            "\
            xxx\n\
            ???\n\
            *xx").unwrap();

        let loc = Location{x:0, y:2};
        let infantry = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "Irving Harrison");
        {
            let neighbs_both = neighbors(&map, loc, RELATIVE_NEIGHBORS.iter(), &Xenophile::new(UnitMovementFilter::new(&infantry)), Wrap2d::BOTH);
            assert!( neighbs_both.contains(&Location{x:0, y:0}));
            assert!( neighbs_both.contains(&Location{x:0, y:1}));
            assert!(!neighbs_both.contains(&Location{x:0, y:2}));
            assert!( neighbs_both.contains(&Location{x:1, y:0}));
            assert!( neighbs_both.contains(&Location{x:1, y:1}));
            assert!( neighbs_both.contains(&Location{x:1, y:2}));
            assert!( neighbs_both.contains(&Location{x:2, y:0}));
            assert!( neighbs_both.contains(&Location{x:2, y:1}));
            assert!( neighbs_both.contains(&Location{x:2, y:2}));
        }

        {
            let neighbs_horiz = neighbors(&map, loc, RELATIVE_NEIGHBORS.iter(), &Xenophile::new(UnitMovementFilter::new(&infantry)), Wrap2d::HORIZ);
            assert!(!neighbs_horiz.contains(&Location{x:0, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:0, y:1}));
            assert!(!neighbs_horiz.contains(&Location{x:0, y:2}));
            assert!(!neighbs_horiz.contains(&Location{x:1, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:1, y:1}));
            assert!( neighbs_horiz.contains(&Location{x:1, y:2}));
            assert!(!neighbs_horiz.contains(&Location{x:2, y:0}));
            assert!( neighbs_horiz.contains(&Location{x:2, y:1}));
            assert!( neighbs_horiz.contains(&Location{x:2, y:2}));
        }

        {
            let neighbs_vert = neighbors(&map, loc, RELATIVE_NEIGHBORS.iter(), &Xenophile::new(UnitMovementFilter::new(&infantry)), Wrap2d::VERT);
            assert!( neighbs_vert.contains(&Location{x:0, y:0}));
            assert!( neighbs_vert.contains(&Location{x:0, y:1}));
            assert!(!neighbs_vert.contains(&Location{x:0, y:2}));
            assert!( neighbs_vert.contains(&Location{x:1, y:0}));
            assert!( neighbs_vert.contains(&Location{x:1, y:1}));
            assert!( neighbs_vert.contains(&Location{x:1, y:2}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:0}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:1}));
            assert!(!neighbs_vert.contains(&Location{x:2, y:2}));
        }

        {
            let neighbs_neither = neighbors(&map, loc, RELATIVE_NEIGHBORS.iter(), &Xenophile::new(UnitMovementFilter::new(&infantry)), Wrap2d::NEITHER);
            assert!(!neighbs_neither.contains(&Location{x:0, y:0}));
            assert!( neighbs_neither.contains(&Location{x:0, y:1}));
            assert!(!neighbs_neither.contains(&Location{x:0, y:2}));
            assert!(!neighbs_neither.contains(&Location{x:1, y:0}));
            assert!( neighbs_neither.contains(&Location{x:1, y:1}));
            assert!( neighbs_neither.contains(&Location{x:1, y:2}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:0}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:1}));
            assert!(!neighbs_neither.contains(&Location{x:2, y:2}));
        }
    }

    #[test]
    fn test_dijkstra() {
        let map = LocationGrid::try_from(
    "\
    xxx\n\
    x x\n\
    *xx").unwrap();

        let loc = Location{x:0, y:0};
        let infantry = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "Carmen Bentley");
        let shortest_neither = old_shortest_paths_for_unit(&map, loc, &infantry, Wrap2d::NEITHER);
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


        let shortest_horiz = old_shortest_paths_for_unit(&map, loc, &infantry, Wrap2d::HORIZ);
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

        let shortest_vert = old_shortest_paths_for_unit(&map, loc, &infantry, Wrap2d::VERT);
        assert_eq!(shortest_vert.dist[Location{x:0, y:0}], Some(0));
        assert_eq!(shortest_vert.dist[Location{x:1, y:0}], Some(1));
        assert_eq!(shortest_vert.dist[Location{x:2, y:0}], Some(2));

        assert_eq!(shortest_vert.dist[Location{x:0, y:1}], Some(1));
        assert_eq!(shortest_vert.dist[Location{x:1, y:1}], None);
        assert_eq!(shortest_vert.dist[Location{x:2, y:1}], Some(2));

        assert_eq!(shortest_vert.dist[Location{x:0, y:2}], Some(1));
        assert_eq!(shortest_vert.dist[Location{x:1, y:2}], Some(1));
        assert_eq!(shortest_vert.dist[Location{x:2, y:2}], Some(2));

        let shortest_both = old_shortest_paths_for_unit(&map, loc, &infantry, Wrap2d::BOTH);
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

    #[test]
    fn test_shortest_paths() {
        let map: LocationGrid<Obs> = LocationGrid::try_from(
            "\
            xxx\n\
            ???\n\
            *xx").unwrap();

        let loc = Location{x:0, y:0};
        let infantry = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "Carmen Bentley");

        let shortest_neither = shortest_paths(
            &map,
            loc,
            &Xenophile::new(UnitMovementFilter::new(&infantry)),
            Wrap2d::NEITHER);

        assert_eq!(shortest_neither.dist[Location{x:0, y:0}], Some(0));
        assert_eq!(shortest_neither.dist[Location{x:1, y:0}], Some(1));
        assert_eq!(shortest_neither.dist[Location{x:2, y:0}], Some(2));

        assert_eq!(shortest_neither.dist[Location{x:0, y:1}], Some(1));
        assert_eq!(shortest_neither.dist[Location{x:1, y:1}], Some(1));
        assert_eq!(shortest_neither.dist[Location{x:2, y:1}], Some(2));

        assert_eq!(shortest_neither.dist[Location{x:0, y:2}], Some(2));
        assert_eq!(shortest_neither.dist[Location{x:1, y:2}], Some(2));
        assert_eq!(shortest_neither.dist[Location{x:2, y:2}], Some(2));
    }

    #[test]
    fn test_nearest_adjacent_unobserved_reachable_without_attacking() {
        _test_nearest_adjacent_unobserved_reachable_without_attacking(Dims::new(10, 10));
        _test_nearest_adjacent_unobserved_reachable_without_attacking(Dims::new(100, 100));
    }

    fn _test_nearest_adjacent_unobserved_reachable_without_attacking(dims: Dims) {
        let src = Location::new(0, 0);
        let dest = Location::new(dims.width-1, dims.height-1);

        let grid = LocationGrid::new(dims, |loc| {
            if loc==dest {
                Obs::Unobserved
            } else {
                Obs::Observed {
                    tile: Tile::new(Terrain::Land, loc),
                    turn: 0,
                    current: false
                }
            }
        });

        for wrapping in [Wrap2d::BOTH, Wrap2d::HORIZ, Wrap2d::VERT, Wrap2d::NEITHER].iter() {
            let unit = Unit::new(UnitID::new(0), src, UnitType::Infantry, Alignment::Belligerent{player: 0}, "Juan de Fuca");

            let acceptable = neighbors(&grid, dest, RELATIVE_NEIGHBORS.iter(), &All, *wrapping);
            let naurwa = nearest_adjacent_unobserved_reachable_without_attacking(&grid, src, &unit, *wrapping);
            assert!(acceptable.contains(naurwa.as_ref().unwrap()));
        }
    }
}
