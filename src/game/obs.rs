use std::fmt;

use crate::{
    game::{
        TurnNum,
        map::{
            LocationGrid,
            grid::LocationGridI,
            Tile,
            dijkstra::{
                Source,
                SourceMut,
            },
        },
    },
    util::{Dims,Dimensioned,Location,Located,LocatedItem,Vec2d,Wrap2d},
};
use super::map::dijkstra::Filter;


/// What a particular player knows about a tile
#[derive(Clone,PartialEq)]
pub enum Obs {
    Observed{tile: Tile, turn: TurnNum, current: bool},
    Unobserved
}

impl Obs {
    pub fn is_observed(&self) -> bool {
        !self.is_unobserved()
    }

    pub fn is_unobserved(&self) -> bool {
        *self == Obs::Unobserved
    }
}

impl fmt::Debug for Obs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Obs::Observed{tile, ..} => tile.fmt(f),
            Obs::Unobserved => write!(f, "?")
        }
    }
}

#[derive(Debug,PartialEq)]
pub struct LocatedObs {
    pub loc: Location,
    pub obs: Obs,
    pub old_obs: Obs,
}
impl LocatedObs {
    pub fn new(loc: Location, obs: Obs, old_obs: Obs) -> Self {
        Self { loc, obs, old_obs }
    }

    pub fn passability_changed<F:Filter<Obs>>(&self, filter: &F) -> bool {
        let was_included = filter.include(&self.old_obs);
        let is_included = filter.include(&self.obs);
        was_included != is_included
    }
}
impl Located for LocatedObs {
    fn loc(&self) -> Location {
        self.loc
    }
}

pub trait ObsTrackerI : Source<Obs> {
    fn track_observation(&mut self, loc: Location, tile: &Tile, turn: TurnNum) -> LocatedObs;
}

pub trait UnifiedObsTrackerI : ObsTrackerI + Source<Obs> + Source<Tile> {
    fn track_observation_unified(&mut self, loc: Location, turn: TurnNum) -> LocatedObs;
}

pub struct UnifiedObsTracker<'a, O, S> {
    truth: &'a mut S,
    observations: &'a mut O,
}

impl <'a, O:ObsTrackerI, S:Source<Tile>> UnifiedObsTracker<'a, O, S> {
    pub fn new(truth: &'a mut S, observations: &'a mut O) -> Self {
        Self {
            truth,
            observations,
        }
    }

    pub fn track_observation(&mut self, loc: Location, tile: &Tile, turn: TurnNum) -> LocatedObs {
        self.observations.track_observation(loc, tile, turn)
    }
}

impl <'a, O:ObsTrackerI, S:SourceMut<Tile>> UnifiedObsTracker<'a, O, S> {
    pub fn put_truth(&mut self, loc: Location, item: &Tile) -> LocatedItem<Tile> {
        self.truth.put(loc, item)
    }
}



impl <'a, O:ObsTrackerI, S:Source<Tile>> Dimensioned for UnifiedObsTracker<'a, O, S> {
    fn dims(&self) -> Dims {
        // The truth and overlay dims are identical
        self.truth.dims()
    }
}

impl <'a, O:ObsTrackerI, S:Source<Tile>> Source<Obs> for UnifiedObsTracker<'a, O, S> {
    fn get(&self, loc: Location) -> &Obs {
        self.observations.get(loc)
    }
}

impl <'a, O:ObsTrackerI, S:Source<Tile>> Source<Tile> for UnifiedObsTracker<'a, O, S> {
    fn get(&self, loc: Location) -> &Tile {
        match <Self as Source<Obs>>::get(self, loc) {
            Obs::Observed{ref tile, ..} => tile,
            Obs::Unobserved => panic!("Tried to get tile from unobserved tile {:?}", loc)
        }
    }
}

impl <'a, O:ObsTrackerI, S:Source<Tile>> ObsTrackerI for UnifiedObsTracker<'a, O, S> {
    fn track_observation(&mut self, loc: Location, tile: &Tile, turn: TurnNum) -> LocatedObs {
        self.observations.track_observation(loc, tile, turn)
    }
}

impl <'a, O:ObsTrackerI, S:Source<Tile>> UnifiedObsTrackerI for UnifiedObsTracker<'a, O, S> {
    fn track_observation_unified(&mut self, loc: Location, turn: TurnNum) -> LocatedObs {
        let tile = self.truth.get(loc);
        self.observations.track_observation(loc, tile, turn)
    }
}




#[derive(Clone)]
pub struct ObsTracker {
    observations: LocationGrid<Obs>,
    num_observed: usize,
}
impl ObsTracker {
    pub fn new(dims: Dims) -> Self {
        Self {
            observations: LocationGrid::new(dims, |_loc: Location| Obs::Unobserved),
            num_observed: 0,
        }
    }

    /// Transfer all "current" to "observed"
    pub fn archive(&mut self) {
        for obs in self.observations.iter_mut() {
            if let Obs::Observed{current,..} = obs {
                *current = false;
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=&Obs> {
        self.observations.iter()
    }

    pub fn num_observed(&self) -> usize {
        self.num_observed
    }
}

impl Dimensioned for ObsTracker {
    fn dims(&self) -> Dims {
        self.observations.dims()
    }
}

impl ObsTrackerI for ObsTracker {
    fn track_observation(&mut self, loc: Location, tile: &Tile, turn: TurnNum) -> LocatedObs {
        let obs = Obs::Observed{ tile: tile.clone(), turn, current: true };

        //CLONE We make one copy to keep inside the ObsTracker, and send the other one back out to the UI
        let old = self.observations.replace(loc, obs.clone());

        // Since we are always replacing with an Obs::Observed, the number observed will go up as long as there was
        // nothing or unobserved there previously
        if old.is_none() || old==Some(Obs::Unobserved) {
            self.num_observed += 1;
        }

        LocatedObs::new(loc, obs, old.unwrap_or(Obs::Unobserved))
    }
}

impl Source<Obs> for ObsTracker {
    fn get(&self, loc: Location) -> &Obs {
        if let Some(in_bounds) = LocationGridI::get(&self.observations, loc) {
            in_bounds
        } else {
            &Obs::Unobserved
        }
    }
}

impl fmt::Debug for ObsTracker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.observations.fmt(f)
    }
}

pub fn visible_coords_iter(sight_distance: u16) -> impl Iterator<Item=Vec2d<i32>>  {
    let sight_distance = i32::from(sight_distance);
    (-sight_distance..=sight_distance).flat_map(move |x| {
        let y_max = sight_distance - x.abs();
        (-y_max..=y_max).map(move |y| {
            Vec2d::new(x,y)
        })
    } )
}

pub trait Observer : Located {
    fn sight_distance(&self) -> u16;

    /// FIXME If we ever get support for impl Trait on trait methods switch to that rather than the likely performance hit of this
    /// vector instantiation
    fn observe<O:ObsTrackerI>(&self, tiles: &dyn Source<Tile>, turn: TurnNum, wrapping: Wrap2d, obs_tracker: &mut O) -> Vec<LocatedObs> {
        visible_coords_iter(self.sight_distance())
            .filter_map(|inc| wrapping.wrapped_add(tiles.dims(), self.loc(), inc))
            .map(|loc| {
                obs_tracker.track_observation(loc, tiles.get(loc), turn)
            })
            .collect()
        // for inc in visible_coords_iter(self.sight_distance()) {
        //     if let Some(loc) = wrapping.wrapped_add(tiles.dims(), self.loc(), inc) {
        //         obs_tracker.observe(loc, tiles.get(loc), turn);
        //     }
        // }
    }

    /// FIXME If we ever get support for impl Trait on trait methods switch to that rather than the likely performance hit of this
    /// vector instantiation
    fn observe_unified<O:UnifiedObsTrackerI>(&self, tiles: &mut O, turn: TurnNum, wrapping: Wrap2d) -> Vec<LocatedObs> {
        let dims = tiles.dims();
        visible_coords_iter(self.sight_distance())
            .filter_map(|inc| wrapping.wrapped_add(dims, self.loc(), inc))
            .map(|loc| {
                tiles.track_observation_unified(loc, turn)
            })
            .collect()
        // for inc in visible_coords_iter(self.sight_distance()) {
        //     if let Some(loc) = wrapping.wrapped_add(tiles.dims(), self.loc(), inc) {
        //         obs_tracker.observe(loc, tiles.get(loc), turn);
        //     }
        // }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        game::{
            Alignment,
            map::{
                LocationGrid,
                Terrain,
                Tile,
                dijkstra::Source,
            },
            obs::{Obs,Observer,ObsTracker,ObsTrackerI},
            unit::{UnitID,Unit,UnitType},
        },
        util::{Dims,Location,Wrap2d},
    };
    
    #[test]
    fn test_fog_of_war_tracker() {
        let dims = Dims{width: 10, height: 20};
        let map: LocationGrid<Tile> = LocationGrid::new(dims, |loc| -> Tile { Tile::new(Terrain::Land, loc) });
        let mut tracker = ObsTracker::new(dims);
        let loc = Location{x: 5, y: 10};
        assert_eq!(*tracker.get(loc), Obs::Unobserved);
        assert_eq!(*tracker.get(Location{x:1000, y: 2000}), Obs::Unobserved);

        let tile = Tile::new(Terrain::Land, loc);

        let turn = 0;

        tracker.track_observation(loc, &tile, turn);

        assert_eq!(*tracker.get(loc), Obs::Observed{tile: tile, turn: turn, current: true});

        let infantry = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "George Glover");
        infantry.observe(&map, turn, Wrap2d::BOTH, &mut tracker);
    }
}
