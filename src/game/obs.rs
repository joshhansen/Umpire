use game::TurnNum;
use map::{LocationGrid,Tile};
use util::{Dims,Location};

/// What a particular player knows about a tile
#[derive(Debug,PartialEq)]
pub enum Obs {
    /// They're observing the tile now
    CURRENT,
    /// The tile was observed on the specified turn
    OBSERVED{tile: Tile, turn: TurnNum},
    /// The tile has not been observed
    UNOBSERVED
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
            observations: LocationGrid::new(&dims, |_loc: &Location| Obs::UNOBSERVED)
        }
    }
}

impl ObsTracker for FogOfWarTracker {
    fn get<'a>(&'a self, loc: Location) -> Option<&'a Obs> {
        self.observations.get(&loc)
    }

    fn observe(&mut self, loc: Location, tile: &Tile, turn: TurnNum) {
        self.observations[loc] = Obs::OBSERVED{tile:tile.clone(), turn:turn};
    }
}

pub struct UniversalVisibilityTracker {
    obs: Obs
}
impl UniversalVisibilityTracker {
    pub fn new() -> Self {
        UniversalVisibilityTracker {
            obs: Obs::CURRENT
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



#[cfg(test)]
mod test {
    use game::obs::{FogOfWarTracker,Obs,ObsTracker};
    use map::{LocationGrid,Terrain,Tile};
    use util::{Dims,Location,WRAP_BOTH};
    use unit::{Alignment,Observer,Unit,UnitType};
    #[test]
    fn test_fog_of_war_tracker() {
        let dims = Dims{width: 10, height: 20};
        let map: LocationGrid<Tile> = LocationGrid::new(&dims, |loc| -> Tile { Tile::new(Terrain::LAND, *loc) });
        let mut tracker: Box<ObsTracker> = Box::new(FogOfWarTracker::new(dims));
        let loc = Location{x: 5, y: 10};
        assert_eq!(tracker.get(loc), Some(&Obs::UNOBSERVED));
        assert_eq!(tracker.get(Location{x:1000, y: 2000}), None);

        let tile = Tile::new(Terrain::LAND, loc);

        let turn = 0;

        tracker.observe(loc, &tile, turn);

        assert_eq!(tracker.get(loc), Some(&Obs::OBSERVED{tile: tile, turn: turn}));



        let infantry = Unit::new(UnitType::INFANTRY, Alignment::BELLIGERENT{player:0});
        infantry.observe(loc, &map, turn, WRAP_BOTH, &mut tracker);
    }
}
