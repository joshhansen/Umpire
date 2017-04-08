use game::TurnNum;
use map::{LocationGrid,Tile};
use util::{Dims,Location,Vec2d,Wrap2d,wrapped_add};

/// What a particular player knows about a tile
#[derive(Debug,PartialEq)]
pub enum Obs {
    /// They're observing the tile now
    Current,

    /// The tile was observed on the specified turn
    Observed{tile: Tile, turn: TurnNum},

    /// The tile has not been observed
    Unobserved
}

pub trait ObsTracker {
    fn get<'a>(&'a self, loc: Location) -> Option<&'a Obs>;
    fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum);
}

pub struct FogOfWarTracker {
    observations: LocationGrid<Obs>
}

impl FogOfWarTracker {
    pub fn new(dims: Dims) -> Self {
        FogOfWarTracker {
            observations: LocationGrid::new(dims, |_loc: Location| Obs::Unobserved)
        }
    }
}

impl ObsTracker for FogOfWarTracker {
    fn get<'a>(&'a self, loc: Location) -> Option<&'a Obs> {
        self.observations.get(&loc)
    }

    fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) {
        self.observations[loc] = Obs::Observed{tile:tile.clone(), turn:turn};
    }
}

pub struct UniversalVisibilityTracker {
    obs: Obs
}
impl UniversalVisibilityTracker {
    pub fn new() -> Self {
        UniversalVisibilityTracker {
            obs: Obs::Current
        }
    }
}
impl ObsTracker for UniversalVisibilityTracker {
    fn get<'a>(&'a self, _loc: Location) -> Option<&'a Obs> {
        Some(&self.obs)
    }

    fn observe(&mut self, _loc: Location, _tile: &Tile, _turn: TurnNum) {
        // do nothing
    }
}

pub fn visible_coords_iter(sight_distance: u16) -> impl Iterator<Item=Vec2d<i32>>  {
    let sight_distance = sight_distance as i32;
    (-sight_distance..sight_distance+1).flat_map(move |x| {
        let y_max = sight_distance - x.abs();
        (-y_max..y_max+1).map(move |y| {
            Vec2d::new(x,y)
        })
    } )
}

pub trait Observer {
    fn sight_distance(&self) -> u16;
    fn observe(&self, observer_loc: Location, tiles: &LocationGrid<Tile>, turn: TurnNum, wrapping: Wrap2d, obs_tracker: &mut Box<ObsTracker>) {
        for inc in visible_coords_iter(self.sight_distance()) {
            if let Some(loc) = wrapped_add(observer_loc, inc, tiles.dims(), wrapping) {
                obs_tracker.observe(loc, &tiles[loc], turn);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use game::obs::{FogOfWarTracker,Obs,Observer,ObsTracker};
    use map::{LocationGrid,Terrain,Tile};
    use util::{Dims,Location,WRAP_BOTH};
    use unit::{Alignment,Unit,UnitType};
    #[test]
    fn test_fog_of_war_tracker() {
        let dims = Dims{width: 10, height: 20};
        let map: LocationGrid<Tile> = LocationGrid::new(dims, |loc| -> Tile { Tile::new(Terrain::Land, loc) });
        let mut tracker: Box<ObsTracker> = Box::new(FogOfWarTracker::new(dims));
        let loc = Location{x: 5, y: 10};
        assert_eq!(tracker.get(loc), Some(&Obs::Unobserved));
        assert_eq!(tracker.get(Location{x:1000, y: 2000}), None);

        let tile = Tile::new(Terrain::Land, loc);

        let turn = 0;

        tracker.observe(loc, &tile, turn);

        assert_eq!(tracker.get(loc), Some(&Obs::Observed{tile: tile, turn: turn}));

        let infantry = Unit::new(UnitType::Infantry, Alignment::Belligerent{player:0}, "George Glover");
        infantry.observe(loc, &map, turn, WRAP_BOTH, &mut tracker);
    }
}
