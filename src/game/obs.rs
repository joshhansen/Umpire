use crate::{
    game::{
        TurnNum,
        map::{
            LocationGrid,
            Tile,
            dijkstra::Source,
        },
        unit::Located,
    },
    util::{Dims,Location,Vec2d,Wrap2d},
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

pub struct ObsTracker {
    observations: LocationGrid<Obs>
}
impl ObsTracker {
    pub fn new(dims: Dims) -> Self {
        Self {
            observations: LocationGrid::new(dims, |_loc: Location| Obs::Unobserved)
        }
    }

    pub fn get(&self, loc: Location) -> &Obs {
        if let Some(in_bounds) = self.observations.get(loc) {
            in_bounds
        } else {
            &Obs::Unobserved
        }
    }

    pub fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) -> LocatedObs {
        let obs = Obs::Observed{ tile: tile.clone(), turn, current: true };
        self.observations[loc] = obs.clone();//CLONE We make one copy to keep inside the ObsTracker, and send the other one back out to the UI
        LocatedObs{ loc, obs }
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
    fn observe(&self, tiles: &dyn Source<Tile>, turn: TurnNum, wrapping: Wrap2d, obs_tracker: &mut ObsTracker) -> Vec<LocatedObs> {
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
            },
            obs::{Obs,Observer,ObsTracker},
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
