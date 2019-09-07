use game::TurnNum;
use map::{LocationGrid,Tile};
use map::dijkstra::Source;
use util::{Dims,Location,Vec2d,Wrap2d,wrapped_add};



/// What a particular player knows about a tile
// #[derive(Debug,PartialEq)]
// pub enum Obs {
//     /// They're observing the tile now
//     Current{tile: Tile, turn: TurnNum},

//     /// The tile was observed on the specified turn
//     Observed{tile: Tile, turn: TurnNum},

//     /// The tile has not been observed
//     Unobserved
// }

// #[derive(Debug,PartialEq)]
// pub struct Obs {
//     tile: Tile,
//     turn: TurnNum,
//     current: bool,
// }

// impl Obs {
//     pub fn tile(&self) -> &Tile { &self.tile }
//     pub fn turn(&self) -> TurnNum { self.turn }
// }

#[derive(Debug,PartialEq)]
pub enum Obs {
    Observed{tile: Tile, turn: TurnNum, current: bool},
    Unobserved
}

// pub enum MaybeObs {
//     Observed(Obs),
//     Unobserved
// }
// impl MaybeObs {
//     pub fn is_observed(&self) -> bool {
//         if let MaybeObs::Observed(obs) = *self {
//             true
//         } else {
//             false
//         }
//     }

//     pub fn is_unobserved(&self) -> bool {
//         *self == MaybeObs::Unobserved
//     }
// }

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

    pub fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) {
        self.observations[loc] = Obs::Observed{ tile: tile.clone(), turn, current: true };
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

// // pub enum ResolvedObs {
// //     Observation{tile: Tile, turn: TurnNum},
// //     Unobserved
// // }

// pub enum ObsTracker {
//     UniversalVisibility,
//     FogOfWar { observations: LocationGrid<Option<Obs>> }
// }

// impl ObsTracker {
//     pub fn new_fog_of_war(dims: Dims) -> Self {
//         Self::FogOfWar {
//             observations: LocationGrid::new(dims, |_loc: Location| None)
//         }
//     }

//     pub fn get(&self, loc: Location) -> Option<&Obs> {
//         match *self {
//             ObsTracker::UniversalVisibility => Obs::Current,
//             ObsTracker::FogOfWar { observations } => observations.get(loc),
//         }
//     }

//     pub fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) {
//         match *self {
//             ObsTracker::UniversalVisibility => { /* do nothing */ },
//             ObsTracker::FogOfWar { observations } => observations[loc] = Some(Obs{ tile: tile.clone(), turn })
//         }
//     }

//     /// Transfer all "current" to "observed"
//     pub fn archive(&mut self) {
//         match *self {
//             ObsTracker::UniversalVisibility => { /* do nothing */ },
//             ObsTracker::FogOfWar { observations } => {
//                 for obs in observations.iter_mut() {
//                     if let Obs::Current{tile,turn} = obs {
//                         *obs = Obs::Observed{tile,turn}
//                     }
//                 }
//             }
//         }
//     }
// }


// pub struct ObsTracker {
//     strategy: ObsStrategy
// }

// impl ObsTracker {
//     fn new(strategy: ObsStrategy)
// }

// pub trait ObsTracker {
//     fn get(&self, loc: Location) -> Option<&Obs>;
//     fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum);
// }

// pub struct FogOfWarTracker {
//     observations: LocationGrid<Obs>
// }

// impl FogOfWarTracker {
//     pub fn new(dims: Dims) -> Self {
//         FogOfWarTracker {
//             observations: LocationGrid::new(dims, |_loc: Location| Obs::Unobserved)
//         }
//     }
// }

// impl ObsTracker for FogOfWarTracker {
//     fn get(&self, loc: Location) -> Option<&Obs> {
//         self.observations.get(loc)
//     }

//     fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) {
//         // self.observations[loc] = Obs::Observed{tile:tile.clone(), turn};
//         self.observations[loc] = Obs::Current;
//     }
// }

// pub struct UniversalVisibilityTracker {
//     obs: Obs
// }
// impl UniversalVisibilityTracker {
//     pub fn new() -> Self {
//         UniversalVisibilityTracker {
//             obs: Obs::Current
//         }
//     }
// }
// impl Default for UniversalVisibilityTracker {
//     fn default() -> Self {
//         UniversalVisibilityTracker {
//             obs: Obs::Current
//         }
//     }
// }
// impl ObsTracker for UniversalVisibilityTracker {
//     fn get(&self, _loc: Location) -> Option<&Obs> {
//         Some(&self.obs)
//     }

//     fn observe(&mut self, _loc: Location, _tile: &Tile, _turn: TurnNum) {
//         // do nothing
//     }
// }

pub fn visible_coords_iter(sight_distance: u16) -> impl Iterator<Item=Vec2d<i32>>  {
    let sight_distance = i32::from(sight_distance);
    (-sight_distance..=sight_distance).flat_map(move |x| {
        let y_max = sight_distance - x.abs();
        (-y_max..=y_max).map(move |y| {
            Vec2d::new(x,y)
        })
    } )
}

pub trait Observer {
    fn sight_distance(&self) -> u16;
    fn observe(&self, observer_loc: Location, tiles: &dyn Source<Tile>, turn: TurnNum, wrapping: Wrap2d, obs_tracker: &mut ObsTracker) {
        for inc in visible_coords_iter(self.sight_distance()) {
            if let Some(loc) = wrapped_add(observer_loc, inc, tiles.dims(), wrapping) {
                obs_tracker.observe(loc, tiles.get(loc), turn);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use game::Alignment;
    use game::obs::{Obs,Observer,ObsTracker};
    use map::{LocationGrid,Terrain,Tile};
    use map::newmap::UnitID;
    use util::{Dims,Location,WRAP_BOTH};
    use unit::{Unit,UnitType};
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
        infantry.observe(loc, &map, turn, WRAP_BOTH, &mut tracker);
    }
}
