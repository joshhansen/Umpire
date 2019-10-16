use crate::{
    game::{
        TurnNum,
        map::{
            LocationGrid,
            Tile,
            dijkstra::Source,
        },
    },
    util::{Dims,Dimensioned,Location,Located,Vec2d,Wrap2d},
};


/// What a particular player knows about a tile
#[derive(Clone,Debug,PartialEq)]
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

#[derive(Debug,PartialEq)]
pub struct LocatedObs {
    pub loc: Location,
    pub obs: Obs,
}
impl LocatedObs {
    pub const fn new(loc: Location, obs: Obs) -> Self {
        Self { loc, obs }
    }
}

pub trait ObsTrackerI : Dimensioned+Source<Obs> {
    fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) -> LocatedObs;
}

pub struct ObsTracker {
    observations: LocationGrid<Obs>
}
impl ObsTracker {
    pub fn new(dims: Dims) -> Self {
        Self {
            observations: LocationGrid::new(dims, |_loc: Location| Obs::Unobserved)
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
}

impl Dimensioned for ObsTracker {
    fn dims(&self) -> Dims {
        self.observations.dims()
    }
}

impl ObsTrackerI for ObsTracker {
    fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) -> LocatedObs {
        let obs = Obs::Observed{ tile: tile.clone(), turn, current: true };
        self.observations[loc] = obs.clone();//CLONE We make one copy to keep inside the ObsTracker, and send the other one back out to the UI
        LocatedObs{ loc, obs }
    }
}

impl Source<Obs> for ObsTracker {
    fn get(&self, loc: Location) -> &Obs {
        if let Some(in_bounds) = self.observations.get(loc) {
            in_bounds
        } else {
            &Obs::Unobserved
        }
    }
}


pub struct OverlayObsTracker<'a, S:Source<Obs>> {
    inner: &'a S,
    overlay: LocationGrid<Option<Obs>>,
    // overlay: HashMap<Location,Option<Obs>>,
}

impl <'a,S:Source<Obs>> Dimensioned for OverlayObsTracker<'a, S> {
    fn dims(&self) -> Dims {
        // The inner and overlay dims are identical
        self.inner.dims()
    }
}

impl <'a,S:Source<Obs>> OverlayObsTracker<'a, S> {
    pub fn new(inner: &'a S) -> Self {
        Self {
            inner,
            overlay: LocationGrid::new(inner.dims(), |_loc| None),
        }
    }
}

impl <'a,S:Source<Obs>> Source<Obs> for OverlayObsTracker<'a, S> {
    fn get(&self, loc: Location) -> &Obs {
        if let Some(overlay_obs) = self.overlay[loc].as_ref() {
            overlay_obs
        } else {
            self.inner.get(loc)
        }
    }
}

impl <'a,S:Source<Obs>> ObsTrackerI for OverlayObsTracker<'a, S> {
    fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) -> LocatedObs {
        let obs = Obs::Observed{ tile: tile.clone(), turn, current: true };//CLONE
        self.overlay[loc] = Some(obs.clone());//CLONE We make one copy to keep inside the ObsTracker, and send the other one back out to the UI
        LocatedObs{ loc, obs }
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
                obs_tracker.observe(loc, tiles.get(loc), turn)
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

        tracker.observe(loc, &tile, turn);

        assert_eq!(*tracker.get(loc), Obs::Observed{tile: tile, turn: turn, current: true});

        let infantry = Unit::new(UnitID::new(0), loc, UnitType::Infantry, Alignment::Belligerent{player:0}, "George Glover");
        infantry.observe(&map, turn, Wrap2d::BOTH, &mut tracker);
    }
}
