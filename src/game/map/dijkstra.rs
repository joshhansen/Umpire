//! Shortest path algorithm

use std::{
    cmp::Ordering,
    collections::{
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
        Aligned,
        Alignment,
        map::LocationGridI,
        map::{LocationGrid,SparseLocationGrid,Terrain,Tile},
        obs::Obs,
        unit::{Unit,UnitType},
    },
    util::{Dims,Dimensioned,Direction,LocatedItem,Location,Vec2d,Wrap2d},
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
    pub start_loc: Location,
    pub dist: SparseLocationGrid<u16>,
    pub prev: SparseLocationGrid<Location>
}

impl fmt::Debug for ShortestPaths {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Distances:\n{:?}\nPrevious Nodes:\n{:?}", self.dist, self.prev)
    }
}

impl ShortestPaths {
    pub fn shortest_path(&self, dest: Location) -> Option<Vec<Location>> {

        let mut path = Vec::new();

        path.insert(0, dest);

        let mut most_recent = dest;

        while let Some(prev) = self.prev.get(most_recent).cloned() {
            path.insert(0, prev);
            most_recent = prev;
        }

        if path.len() > 1 {
            Some(path)
        } else {
            None
        }
    }

    /// The next direction to move to begin moving along the shortest path to the destination
    pub fn direction_of(&self, dims: Dims, wrapping: Wrap2d, dest: Location) -> Option<Direction> {
        self.shortest_path(dest).map(|path| {
            let first_loc: Location = path[1];
            // let inc = first_loc - self.start_loc;
            let inc: Vec2d<i32> = wrapping.wrapped_sub(dims, self.start_loc, first_loc).unwrap();
            let dir = Direction::try_from(inc).expect(format!("{} not a Direction; first_loc: {:?}, start_loc: {:?}", inc, first_loc, self.start_loc).as_str());
            dir
        })
    }
}

static DIRECTIONS: [Direction; 8] = Direction::values();

const REL_NEIGHB_BOTTOM_LEFT:  Vec2d<i32> = Vec2d::new(-1, -1);
const REL_NEIGHB_BOTTOM:       Vec2d<i32> = Vec2d::new(-1,  0);
const REL_NEIGHB_BOTTOM_RIGHT: Vec2d<i32> = Vec2d::new(-1,  1);
const REL_NEIGHB_LEFT:         Vec2d<i32> = Vec2d::new( 0, -1);
const REL_NEIGHB_RIGHT:        Vec2d<i32> = Vec2d::new( 0,  1);
const REL_NEIGHB_TOP_LEFT:     Vec2d<i32> = Vec2d::new( 1, -1);
const REL_NEIGHB_TOP:          Vec2d<i32> = Vec2d::new( 1,  0);
const REL_NEIGHB_TOP_RIGHT:    Vec2d<i32> = Vec2d::new( 1,  1);

pub static RELATIVE_NEIGHBORS: [Vec2d<i32>; 8] = [
    REL_NEIGHB_BOTTOM_LEFT, REL_NEIGHB_BOTTOM, REL_NEIGHB_BOTTOM_RIGHT, REL_NEIGHB_LEFT,
    REL_NEIGHB_RIGHT, REL_NEIGHB_TOP_LEFT, REL_NEIGHB_TOP, REL_NEIGHB_TOP_RIGHT,
];
pub static RELATIVE_NEIGHBORS_CARDINAL: [Vec2d<i32>; 4] = [
    REL_NEIGHB_BOTTOM, REL_NEIGHB_LEFT, REL_NEIGHB_RIGHT, REL_NEIGHB_TOP,
];
pub static RELATIVE_NEIGHBORS_DIAGONAL: [Vec2d<i32>; 4] = [
    REL_NEIGHB_BOTTOM_LEFT, REL_NEIGHB_BOTTOM_RIGHT, REL_NEIGHB_TOP_LEFT, REL_NEIGHB_TOP_RIGHT,
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

/// Include tiles that a unit could attack if adjacent
pub struct UnitAttackFilter<'a> {
    pub unit: &'a Unit
}
impl <'a> Filter<Tile> for UnitAttackFilter<'a> {
    fn include(&self, neighb_tile: &Tile) -> bool {
        self.unit.can_attack_tile(neighb_tile)
    }
}
impl <'a> Filter<Obs> for UnitAttackFilter<'a> {
    fn include(&self, obs: &Obs) -> bool {
        if let Obs::Observed{tile,..} = obs {
            Self::include(self, tile)
        } else {
            false
        }
    }
}

pub struct UnitMovementFilterXenophile<'a> {
    pub unit: &'a Unit
}
impl <'a> UnitMovementFilterXenophile<'a> {
    pub fn new(unit: &'a Unit) -> Self {
        Self {
            unit
        }
    }
}
impl <'a> Filter<Obs> for UnitMovementFilterXenophile<'a> {
    fn include(&self, obs: &Obs) -> bool {
        if let Obs::Observed{tile,..} = obs {
            self.unit.can_move_on_tile(tile)
        } else {
            true
        }
    }
}

/// A filter that accepts all locations the unit could move on without attacking; unobserved are included automatically
/// with the assumption that by the time the unit reaches that tile, it will have become observed and will be handled
/// differently.
pub struct PacifistXenophileUnitMovementFilter<'a> {
    pub unit: &'a Unit,
}
impl <'a> Filter<Obs> for PacifistXenophileUnitMovementFilter<'a> {
    fn include(&self, obs: &Obs) -> bool {
        if let Obs::Observed{tile,..} = obs {

            if let Some(ref unit) = tile.unit {
                if unit.is_friendly_to(self.unit) {
                    return unit.can_carry_unit(&self.unit);
                }

                return false;
            }

            if let Some(ref city) = tile.city {
                if !city.is_friendly_to(self.unit) {
                    return false;
                }
            }

            self.unit.can_move_on_tile(tile)

        } else {
            true
        }
    }
}

/// A filter that yields observed tiles that a unit could reach in exploration (visiting tiles of appropriate terrain which contain no unit
/// and only friendly cities if any)
/// 
/// This disallows visits to carrier units under the presumption that an exploring unit would not bother boarding a transport or landing on an
/// aircraft carrier.
pub struct ObservedReachableByPacifistUnit<'a> {
    pub unit: &'a Unit
}
impl <'a> Filter<Obs> for ObservedReachableByPacifistUnit<'a> {
    fn include(&self, obs: &Obs) -> bool {
        if let Obs::Observed{tile,..} = obs {

            if tile.unit.is_some() {
                return false;
            }

            if let Some(ref city) = tile.city {
                if city.alignment != self.unit.alignment {
                    return false;
                }
            }

            self.unit.can_move_on_tile(tile)

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

pub trait Source<T> : Dimensioned {
    fn get(&self, loc: Location) -> &T;
}
pub trait SourceMut<T> : Source<T> {
    fn put(&mut self, loc: Location, item: &T) -> LocatedItem<T>;
}
pub struct OverlaySource<'a,T,S:Source<T>> {
    inner: &'a S,
    overlay: LocationGrid<Option<T>>,
}

impl <'a,T:Clone,S:Source<T>> OverlaySource<'a,T,S> {
    pub fn new(inner: &'a S) -> Self {
        Self {
            inner,
            overlay: LocationGrid::new(inner.dims(), |_loc| None),
        }
    }

    pub fn overlaid_items(&self) -> Vec<LocatedItem<T>> {
        let mut observations: Vec<LocatedItem<T>> = Vec::new();

        for loc in self.overlay.iter_locs() {
            if let Some(ref item) = self.overlay[loc] {
                observations.push(LocatedItem::new(loc, (*item).clone()));//CLONE
            }
        }

        observations
    }
}

impl <'a,T,S:Source<T>> Dimensioned for OverlaySource<'a,T,S> {
    fn dims(&self) -> Dims {
        // The inner and overlay dims are identical
        self.inner.dims()
    }
}

// impl <'a,S:Source<Obs>> Source<Obs> for OverlaySource<'a,Obs,S> {
//     fn get(&self, loc: Location) -> &Obs {
//         if let Some(overlay_obs) = self.overlay[loc].as_ref() {
//             overlay_obs
//         } else {
//             self.inner.get(loc)
//         }
//     }
// }

// impl <'a,S:Source<Obs>> Source<Tile> for OverlaySource<'a,Obs,S> {
//     fn get(&self, loc: Location) -> &Tile {
//         match <Self as Source<Obs>>::get(self, loc) {
//             Obs::Observed{ref tile, ..} => tile,
//             Obs::Unobserved => panic!("Tried to get tile from unobserved tile {:?}", loc)
//         }
//     }
// }

impl <'a,T,S:Source<T>> Source<T> for OverlaySource<'a,T,S> {
    fn get(&self, loc: Location) -> &T {
        if let Some(overlay_item) = self.overlay[loc].as_ref() {
            overlay_item
        } else {
            self.inner.get(loc)
        }
    }
}

impl <'a,T:Clone,S:Source<T>> SourceMut<T> for OverlaySource<'a,T,S> {
    fn put(&mut self, loc: Location, item: &T) -> LocatedItem<T> {
        self.overlay[loc] = Some((*item).clone());//CLONE
        LocatedItem::new(loc, (*item).clone())//CLONE
    }
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

/// An iterator yielding the absolute location and the looked-up `T` from each location for all neighbors of `loc` respecting the
/// provided wrapping rules `wrapping`.
fn all_resolved_neighbors_iter<'a, T: 'a, S>(tiles: &'a S, loc: Location, wrapping: Wrap2d) -> impl Iterator<Item=(Location,&'a T)>
    where S:Source<T> {

        RELATIVE_NEIGHBORS.iter()
            .filter_map(move |rel_neighb| wrapping.wrapped_add(tiles.dims(), loc, *rel_neighb))
            .map(move |neighb| (neighb,tiles.get(neighb)))
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

pub fn neighbors_iter_owned_filter<'a, T, F, N, S>(tiles: &'a S, loc: Location, rel_neighbs: N,
                                 filter: F, wrapping: Wrap2d) -> impl Iterator<Item=Location> + 'a
    where F:Filter<T>+'a, S:Source<T>, N:Iterator<Item=&'a Vec2d<i32>>+'a {

        rel_neighbs.filter_map(move |rel_neighb| wrapping.wrapped_add(tiles.dims(), loc, *rel_neighb))
                   .filter(move |neighb_loc| filter.include(tiles.get(*neighb_loc)))
}

pub fn directions_iter_owned_filter<'a, T, F, D, S>(tiles: &'a S, loc: Location, directions: D,
                                 filter: F, wrapping: Wrap2d) -> impl Iterator<Item=Direction> + 'a
    where F:Filter<T>+'a, S:Source<T>, D:Iterator<Item=&'a Direction>+'a {

        directions
            .filter_map(move |direction| {
                wrapping
                    .wrapped_add(tiles.dims(), loc, (*direction).into())
                    .map(|loc| (direction,loc))
            })
            .filter(move |(_direction,neighb_loc)| {
                filter.include(tiles.get(*neighb_loc))
            })
            .map(|(direction,_neighb_loc)| *direction)
}

struct UnitTypeFilter {
    unit_type: UnitType
}
impl Filter<Tile> for UnitTypeFilter {
    fn include(&self, neighb_tile: &Tile) -> bool {
        self.unit_type.can_move_on_tile(neighb_tile)
    }
}

/// Returns the set of locations for neighbors of the given location including only those which the given unit type
/// could theoretically move onto, considering only the terrain (not units or cities).
pub fn neighbors_terrain_only<T:Source<Tile>>(tiles: &T, loc: Location, unit_type: UnitType, wrapping: Wrap2d) -> HashSet<Location> {
    neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), &UnitTypeFilter{unit_type}, wrapping)
}

/// Returns the set of locations for neighbors of the given location including only those which the given unit could
/// theoretically move onto, considering all properties of the neighboring tiles (but not unit moves remaining and
/// such).
pub fn neighbors_unit_could_move_to<T:Source<Tile>>(tiles: &T, unit: &Unit, wrapping: Wrap2d) -> HashSet<Location> {
    neighbors(tiles, unit.loc, RELATIVE_NEIGHBORS.iter(), &UnitMovementFilter{unit}, wrapping)
}

pub fn neighbors_unit_could_move_to_iter<'a, T:Source<Tile>>(tiles: &'a T, unit: &'a Unit, wrapping: Wrap2d) -> impl Iterator<Item=Location> + 'a {
    let loc = unit.loc;
    let neighb_iter = RELATIVE_NEIGHBORS.iter();
    let filter = UnitMovementFilter{unit};
    neighbors_iter_owned_filter(tiles, loc, neighb_iter, filter, wrapping)
}

pub fn directions_unit_could_move_iter<'a, T:Source<Tile>>(tiles: &'a T, unit: &'a Unit, wrapping: Wrap2d) -> impl Iterator<Item=Direction> + 'a {
    let loc = unit.loc;
    // let neighb_iter = RELATIVE_NEIGHBORS.iter();
    // let dir_iter = Direction::values().iter();
    let filter = UnitMovementFilter{unit};
    directions_iter_owned_filter(tiles, loc, DIRECTIONS.iter(), filter, wrapping)
}

#[derive(Eq,PartialEq)]
struct State {
    dist_: u16,
    loc: Location
}

impl Ord for State {
    fn cmp(&self, other: &State) -> Ordering {
        let c = other.dist_.cmp(&self.dist_);
        if c == Ordering::Equal {
            self.loc.cmp(&other.loc)
        } else {
            c
        }
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
/// 
/// # Arguments
/// 
/// * `max_dist`: the maximum distance to consider in the path search
pub fn shortest_paths<T,F:Filter<T>,S:Source<T>>(tiles: &S, source: Location, filter: &F, wrapping: Wrap2d, max_dist: u16) -> ShortestPaths {
    let mut q = VecDeque::new();

    let mut dist: SparseLocationGrid<u16> = SparseLocationGrid::new(tiles.dims());
    let mut prev: SparseLocationGrid<Location> = SparseLocationGrid::new(tiles.dims());

    q.push_back(State{ dist_: 0, loc: source });

    dist.replace(source, 0);

    while let Some(State{ dist_, loc }) = q.pop_front() {

        // Quit early since we're already doing worse than the best known route
        if let Some(dist) = dist.get(loc) {
            if dist_ > *dist {
                continue;
            }
        }

        // for neighb_loc in neighbors(tiles, loc, RELATIVE_NEIGHBORS.iter(), filter, wrapping) {
        for neighb_loc in neighbors_iter(tiles, loc, RELATIVE_NEIGHBORS.iter(), filter, wrapping) {
            let new_dist = dist_ + 1;

            if new_dist > max_dist {
                continue;//NOTE we might be able to just return here
            }

            let next = State { dist_: new_dist, loc: neighb_loc };

            // if let Some(neighb_dist) = dist.get(neighb_loc) {
            if dist.get(neighb_loc).is_none() || new_dist < dist[neighb_loc] {
                q.push_back(next);
                dist.replace(neighb_loc, new_dist);
                prev.replace(neighb_loc, loc);
            }
        }
    }

    ShortestPaths { start_loc: source, dist, prev }
}

/// Return the (or a) closest tile to the source which is reachable by the given
/// unit and is adjacent to at least one unobserved tile. If no such tile exists
/// then return None
pub fn nearest_adjacent_unobserved_reachable_without_attacking<S:Source<Obs>>(
    tiles: &S,
    src: Location,
    unit: &Unit,
    wrapping: Wrap2d
) -> Option<Location> {
    let candidate_filter = ObservedReachableByPacifistUnit{ unit };
    let target_filter = UnobservedFilter;
    bfs(tiles, src, wrapping, &candidate_filter, &target_filter)
}

/// Perform a breadth first search.
/// 
/// # Arguments
/// * `candidate_filter` determines which tiles to include in the search
/// * `target_filter` specifies which tiles we're searching for
pub fn bfs<T,S:Source<T>,CandidateFilter:Filter<T>,TargetFilter:Filter<T>>(
    tiles: &S,
    src: Location,
    wrapping: Wrap2d,
    candidate_filter: &CandidateFilter,
    target_filter: &TargetFilter,
) -> Option<Location> {

    let mut q: VecDeque<Location> = VecDeque::new();
    q.push_back(src);

    // let mut visited: SparseLocationGrid<bool> = SparseLocationGrid::new(tiles.dims());
    let mut visited: HashSet<Location> = HashSet::new();
    visited.insert(src);

    while let Some(loc) = q.pop_front() {
        for (neighb,obs) in all_resolved_neighbors_iter(tiles, loc, wrapping) {

            if target_filter.include(obs) {
                return Some(loc);
            }

            if !visited.contains(&neighb) && candidate_filter.include(obs) {
                q.push_back(neighb);
                visited.insert(neighb);
                // visited.replace(src, true);// do this now to preempt duplicates, even though we haven't visited it yet
            }

        }
    }
    None
}

#[cfg(test)]
mod test {

    use std::{
        collections::HashSet,
    };

    use crate::{
        game::{
            Alignment,
            map::{
                LocationGrid,
                LocationGridI,
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
        Filter,
        PacifistXenophileUnitMovementFilter,
        Source,
        UnitMovementFilter,
        Xenophile,
        nearest_adjacent_unobserved_reachable_without_attacking,
        neighbors,
        neighbors_terrain_only,
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
        let map: LocationGrid<Tile> = LocationGrid::try_from(
    "\
    *xx\n\
    x x\n\
    xxx").unwrap();

        let loc = Location{x:0, y:0};
        let armor = Unit::new(UnitID::new(0), loc, UnitType::Armor, Alignment::Belligerent{player:0}, "Carmen Bentley");
        let filter = UnitMovementFilter::new(&armor);
        let shortest_neither = shortest_paths(&map, loc, &filter, Wrap2d::NEITHER, armor.moves_remaining());
        println!("{:?}", shortest_neither);
        assert_eq!(shortest_neither.dist[Location{x:0, y:0}], 0);
        assert_eq!(shortest_neither.dist[Location{x:1, y:0}], 1);
        assert_eq!(shortest_neither.dist[Location{x:2, y:0}], 2);

        assert_eq!(shortest_neither.dist[Location{x:0, y:1}], 1);
        assert_eq!(shortest_neither.dist.get(Location{x:1, y:1}), None);// Nothing here
        assert_eq!(shortest_neither.dist[Location{x:2, y:1}], 2);

        assert_eq!(shortest_neither.dist[Location{x:0, y:2}], 2);
        assert_eq!(shortest_neither.dist[Location{x:1, y:2}], 2);
        assert_eq!(shortest_neither.dist.get(Location{x:2, y:2}), None);// Out of range, takes 3 moves


        let shortest_horiz = shortest_paths(&map, loc, &filter, Wrap2d::HORIZ, armor.moves_remaining());
        println!("{:?}", shortest_horiz);
        assert_eq!(shortest_horiz.dist[Location{x:0, y:0}], 0);
        assert_eq!(shortest_horiz.dist[Location{x:1, y:0}], 1);
        assert_eq!(shortest_horiz.dist[Location{x:2, y:0}], 1);

        assert_eq!(shortest_horiz.dist[Location{x:0, y:1}], 1);
        assert_eq!(shortest_horiz.dist.get(Location{x:1, y:1}), None);
        assert_eq!(shortest_horiz.dist[Location{x:2, y:1}], 1);

        assert_eq!(shortest_horiz.dist[Location{x:0, y:2}], 2);
        assert_eq!(shortest_horiz.dist[Location{x:1, y:2}], 2);
        assert_eq!(shortest_horiz.dist[Location{x:2, y:2}], 2);

        let shortest_vert = shortest_paths(&map, loc, &filter, Wrap2d::VERT, armor.moves_remaining());
        assert_eq!(shortest_vert.dist[Location{x:0, y:0}], 0);
        assert_eq!(shortest_vert.dist[Location{x:1, y:0}], 1);
        assert_eq!(shortest_vert.dist[Location{x:2, y:0}], 2);

        assert_eq!(shortest_vert.dist[Location{x:0, y:1}], 1);
        assert_eq!(shortest_vert.dist.get(Location{x:1, y:1}), None);
        assert_eq!(shortest_vert.dist[Location{x:2, y:1}], 2);

        assert_eq!(shortest_vert.dist[Location{x:0, y:2}], 1);
        assert_eq!(shortest_vert.dist[Location{x:1, y:2}], 1);
        assert_eq!(shortest_vert.dist[Location{x:2, y:2}], 2);

        let shortest_both = shortest_paths(&map, loc, &filter, Wrap2d::BOTH, armor.moves_remaining());
        assert_eq!(shortest_both.dist[Location{x:0, y:0}], 0);
        assert_eq!(shortest_both.dist[Location{x:1, y:0}], 1);
        assert_eq!(shortest_both.dist[Location{x:2, y:0}], 1);

        assert_eq!(shortest_both.dist[Location{x:0, y:1}], 1);
        assert_eq!(shortest_both.dist.get(Location{x:1, y:1}), None);
        assert_eq!(shortest_both.dist[Location{x:2, y:1}], 1);

        assert_eq!(shortest_both.dist[Location{x:0, y:2}], 1);
        assert_eq!(shortest_both.dist[Location{x:1, y:2}], 1);
        assert_eq!(shortest_both.dist[Location{x:2, y:2}], 1);
    }

    #[test]
    fn test_shortest_paths() {
        let map: LocationGrid<Obs> = LocationGrid::try_from(
            "*..
???
...").unwrap();

        let loc = Location{x:0, y:0};
        let infantry = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "Carmen Bentley");

        let shortest_neither = shortest_paths(
            &map,
            loc,
            &Xenophile::new(UnitMovementFilter::new(&infantry)),
            Wrap2d::NEITHER,
            infantry.moves_remaining(),
        );

        assert_eq!(shortest_neither.dist[Location{x:0, y:0}], 0);
        assert_eq!(shortest_neither.dist[Location{x:1, y:0}], 1);
        assert_eq!(shortest_neither.dist.get(Location{x:2, y:0}), None);//distance 2

        assert_eq!(shortest_neither.dist[Location{x:0, y:1}], 1);
        assert_eq!(shortest_neither.dist[Location{x:1, y:1}], 1);
        assert_eq!(shortest_neither.dist.get(Location{x:2, y:1}), None);// distance 2

        assert_eq!(shortest_neither.dist.get(Location{x:0, y:2}), None);// distance 2
        assert_eq!(shortest_neither.dist.get(Location{x:1, y:2}), None);// distance 2
        assert_eq!(shortest_neither.dist.get(Location{x:2, y:2}), None);// distance 2
    }

    #[test]
    fn test_nearest_adjacent_unobserved_reachable_without_attacking() {
        _test_nearest_adjacent_unobserved_reachable_without_attacking(Dims::new(10, 10));
        _test_nearest_adjacent_unobserved_reachable_without_attacking(Dims::new(10, 1));
        _test_nearest_adjacent_unobserved_reachable_without_attacking(Dims::new(1, 10));
        // _test_nearest_adjacent_unobserved_reachable_without_attacking(Dims::new(100, 1));
        // _test_nearest_adjacent_unobserved_reachable_without_attacking(Dims::new(1, 100));
        // _test_nearest_adjacent_unobserved_reachable_without_attacking(Dims::new(100, 100));
        //FIXME We should be able to test this at higher dimensionality
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

    #[test]
    pub fn test_unit_movement_filter() {
        let l1 = Location::new(0,0);
        let l2 = Location::new(1,0);
        let a = Alignment::Belligerent{player:0};
        let u1 = Unit::new(UnitID::new(0), l1, UnitType::Infantry,
            a, "u1");

        let u2 = Unit::new(UnitID::new(1), l2, UnitType::Infantry,
            a, "u2");

        let filter = UnitMovementFilter::new(&u1);

        let mut tile = Tile::new(Terrain::Land, l2);
        tile.unit = Some(u2);

        assert!(!filter.include(&tile));
    }

    // FIXME: This test isn't very thorough---it only tests loading onto a transport
    #[test]
    fn test_pacifist_xenophile_movement_filter() {
        let l1 = Location::new(0,0);
        let l2 = Location::new(1,0);
        let a = Alignment::Belligerent{player:0};

        let armor = Unit::new(UnitID::new(0), l1, UnitType::Armor, a, "Armie");
        let transport = Unit::new(UnitID::new(1), l2, UnitType::Transport, a, "Portia");

        let filter = PacifistXenophileUnitMovementFilter{unit: &armor};

        let mut transport_tile = Tile::new(Terrain::Water, transport.loc);
        transport_tile.unit = Some(transport);

        let obs = Obs::Observed {
            tile: transport_tile,
            turn: 0,
            current: true,
        };

        assert!(filter.include(&obs));
    
    }
}
