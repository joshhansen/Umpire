use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    ai::BASE_CONV_FEATS, alignment::AlignedMaybe, map::dijkstra::Filter, ActionNum, PlayerNum,
};
use crate::{
    game::{
        fX,
        map::{
            dijkstra::{Source, SourceMut},
            grid::LocationGridI,
            LocationGrid, Terrain, Tile,
        },
        unit::UnitType,
        TurnNum,
    },
    util::{indicator as b, Dimensioned, Dims, Located, LocatedItem, Location, Vec2d, Wrap2d},
};

/// What a particular player knows about a tile
#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub enum Obs {
    Observed {
        tile: Tile,

        /// The turn when the observation was made
        turn: TurnNum,

        /// The number of actions taken globally when the observation was made
        ///
        /// Similar to `turn` but more granular
        action_count: ActionNum,

        current: bool,
    },
    Unobserved,
}

impl Obs {
    pub fn is_observed(&self) -> bool {
        !self.is_unobserved()
    }

    pub fn is_unobserved(&self) -> bool {
        *self == Obs::Unobserved
    }

    /// Observation features:
    /// - known to be land (0 or 1)
    /// - known to be sea (0 or 1)
    /// - known to have city (0 or 1)
    /// - known to have unit of type - 10 bits (one hot encoded)
    /// - carried_units - count
    /// - city/unit friendly - 1 bit
    /// - city/unit non-friendly - 1 bit (separate to allow unobserved to be neither friendly nor unfriendly)
    /// - city production progress - 1 fX
    /// - city production cost - 1 fX
    /// - city production as % of cost - 1 fX
    /// - is observation in bounds - 1 fX. All Obs are, but Location wrapped_add can yield Option<&Obs> of None
    //    that represent out-of-bounds.
    pub fn features(&self, player: PlayerNum) -> [fX; BASE_CONV_FEATS] {
        let none = UnitType::none_features();
        let unit_type_feats = match self {
            Self::Observed { tile, .. } => tile
                .unit
                .as_ref()
                .map_or(none, |unit| unit.type_.features()),
            Self::Unobserved => none,
        };

        let production_progress = match self {
            Self::Observed { tile, .. } => tile
                .city
                .as_ref()
                .map_or(0.0 as fX, |city| city.production_progress as fX),
            _ => 0.0 as fX,
        };
        [
            // 0: known to be land (0 or 1)
            match self {
                Self::Observed { ref tile, .. } => b(tile.terrain == Terrain::Land),
                Self::Unobserved => 0.0,
            },
            // 1: known to be sea (0 or 1)
            match self {
                Self::Observed { ref tile, .. } => b(tile.terrain == Terrain::Water),
                Self::Unobserved => 0.0,
            },
            // 2: known to have city (0 or 1)
            match self {
                Self::Observed { ref tile, .. } => b(tile.city.is_some()),
                Self::Unobserved => 0.0,
            },
            // 3-12: known to have unit of type - 10 bits (one hot encoded)
            unit_type_feats[0],
            unit_type_feats[1],
            unit_type_feats[2],
            unit_type_feats[3],
            unit_type_feats[4],
            unit_type_feats[5],
            unit_type_feats[6],
            unit_type_feats[7],
            unit_type_feats[8],
            unit_type_feats[9],
            // 13:  carried_units - count
            match self {
                Self::Observed { ref tile, .. } => tile
                    .unit
                    .as_ref()
                    .map_or(0.0, |unit| unit.carried_units().count() as fX),
                Self::Unobserved => 0.0,
            },
            // 14: city/unit friendly - 1 bit
            match self {
                Self::Observed { ref tile, .. } => tile
                    .alignment_maybe()
                    .map_or(0.0, |alignment| b(alignment.is_friendly_to_player(player))),
                Self::Unobserved => 0.0,
            },
            // 15: city/unit non-friendly - 1 bit
            match self {
                Self::Observed { ref tile, .. } => tile
                    .alignment_maybe()
                    .map_or(0.0, |alignment| b(alignment.is_enemy_of_player(player))),
                Self::Unobserved => 0.0,
            },
            // 16: city production progress
            production_progress,
            // 17: city production cost
            match self {
                Self::Observed { tile, .. } => tile.city.as_ref().map_or(0.0 as fX, |city| {
                    city.production().map_or(0.0 as fX, |ut| ut.cost() as fX)
                }),
                _ => 0.0 as fX,
            },
            // 18: city production as % of cost
            match self {
                Self::Observed { tile, .. } => tile.city.as_ref().map_or(0.0 as fX, |city| {
                    city.production()
                        .map_or(0.0 as fX, |ut| production_progress / ut.cost() as fX)
                }),
                _ => 0.0 as fX,
            },
            // 19: is in bounds; all Obs are
            1.0 as fX,
        ]
    }
}

//FIXME Merge with Map::draw_tile_no_flush?
impl fmt::Display for Obs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Observed { tile, .. } => std::fmt::Display::fmt(&tile, f),
            Self::Unobserved => write!(f, " "),
        }
    }
}

impl fmt::Debug for Obs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Observed { tile, .. } => std::fmt::Debug::fmt(&tile, f),
            Self::Unobserved => write!(f, " "),
        }
    }
}

/// Like LocatedObs but doesn't record the prior observation
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocatedObsLite {
    pub loc: Location,
    pub obs: Obs,
}
impl LocatedObsLite {
    pub fn new(loc: Location, obs: Obs) -> Self {
        Self { loc, obs }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocatedObs {
    pub loc: Location,
    pub obs: Obs,
    pub old_obs: Obs,
}
impl LocatedObs {
    pub fn new(loc: Location, obs: Obs, old_obs: Obs) -> Self {
        Self { loc, obs, old_obs }
    }

    pub fn passability_changed<F: Filter<Obs>>(&self, filter: &F) -> bool {
        let was_included = filter.include(&self.old_obs);
        let is_included = filter.include(&self.obs);
        was_included != is_included
    }

    pub fn lite(self) -> LocatedObsLite {
        LocatedObsLite {
            loc: self.loc,
            obs: self.obs,
        }
    }
}
impl Located for LocatedObs {
    fn loc(&self) -> Location {
        self.loc
    }
}

pub struct UnifiedObsTracker<'a, S> {
    truth: &'a mut S,
    observations: &'a mut ObsTracker,
}

impl<'a, S: Source<Tile>> UnifiedObsTracker<'a, S> {
    pub fn new(truth: &'a mut S, observations: &'a mut ObsTracker) -> Self {
        Self {
            truth,
            observations,
        }
    }

    pub fn track_observation(
        &mut self,
        loc: Location,
        tile: &Tile,
        turn: TurnNum,
        action: ActionNum,
    ) -> LocatedObs {
        self.observations.track_observation(loc, tile, turn, action)
    }
}

impl<'a, S: SourceMut<Tile>> UnifiedObsTracker<'a, S> {
    pub fn put_truth(&mut self, loc: Location, item: &Tile) -> LocatedItem<Tile> {
        self.truth.put(loc, item)
    }
}

impl<'a, S: Source<Tile>> Dimensioned for UnifiedObsTracker<'a, S> {
    fn dims(&self) -> Dims {
        // The truth and overlay dims are identical
        self.truth.dims()
    }
}

impl<'a, S: Source<Tile>> Source<Obs> for UnifiedObsTracker<'a, S> {
    fn get(&self, loc: Location) -> Option<&Obs> {
        self.observations.get(loc)
    }
}

impl<'a, S: Source<Tile>> Source<Tile> for UnifiedObsTracker<'a, S> {
    fn get(&self, loc: Location) -> Option<&Tile> {
        match <Self as Source<Obs>>::get(self, loc) {
            Some(Obs::Observed { tile, .. }) => Some(tile),
            _ => panic!(
                "Tried to get tile from unobserved or out of bounds tile {:?}",
                loc
            ),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
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

    /// Mark everything as no longer being current
    pub fn archive(&mut self) {
        for obs in self.observations.iter_mut() {
            if let Obs::Observed { current, .. } = obs {
                *current = false;
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Obs> {
        self.observations.iter()
    }

    pub fn num_observed(&self) -> usize {
        self.num_observed
    }

    fn _track(&mut self, loc: Location, obs: Obs) -> Option<Obs> {
        let new_is_unobserved = obs == Obs::Unobserved;
        let old = self.observations.replace(loc, obs);

        // Since we are always replacing with an Obs::Observed, the number observed will go up as long as there was
        // nothing or unobserved there previously
        if old.is_none() || old == Some(Obs::Unobserved) {
            self.num_observed += 1;
        } else if new_is_unobserved {
            // If there _was_ an observation, but now we're setting unobserved, decrease the count
            self.num_observed -= 1;
        }

        old
    }

    pub fn track_lite(&mut self, located_obs: LocatedObsLite) -> Option<Obs> {
        self._track(located_obs.loc, located_obs.obs)
    }

    pub fn track_located(&mut self, located_obs: LocatedObs) -> Option<Obs> {
        self._track(located_obs.loc, located_obs.obs)
    }

    pub fn track_observation(
        &mut self,
        loc: Location,
        tile: &Tile,
        turn: TurnNum,
        action_count: ActionNum,
    ) -> LocatedObs {
        let obs = Obs::Observed {
            tile: tile.clone(),
            turn,
            action_count,
            current: true,
        };

        //CLONE We make one copy to keep inside the ObsTracker, and send the other one back out to the UI
        let old = self._track(loc, obs.clone());

        LocatedObs::new(loc, obs, old.unwrap_or(Obs::Unobserved))
    }

    pub fn track_many<'a>(&mut self, observations: impl Iterator<Item = &'a LocatedObs>) {
        for obs in observations {
            self._track(obs.loc, obs.obs.clone());
        }
    }

    pub fn track_many_lite<'a>(&mut self, observations: impl Iterator<Item = &'a LocatedObsLite>) {
        for obs in observations {
            self.track_lite(obs.clone());
        }
    }

    pub fn track_many_lite_owned(&mut self, observations: impl Iterator<Item = LocatedObsLite>) {
        for obs in observations {
            self.track_lite(obs);
        }
    }
}

impl Dimensioned for ObsTracker {
    fn dims(&self) -> Dims {
        self.observations.dims()
    }
}

impl Source<Obs> for ObsTracker {
    fn get(&self, loc: Location) -> Option<&Obs> {
        LocationGridI::get(&self.observations, loc)
    }
}

impl fmt::Display for ObsTracker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.observations, f)
    }
}

impl fmt::Debug for ObsTracker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.observations, f)
    }
}

#[derive(Debug, Error)]
pub enum ObsTrackerError {
    #[error("No observation tracker present for player {0}")]
    NoTrackerForPlayer(PlayerNum),
}

/// Convenience struct to track the observations of one or more players
#[derive(Clone)]
pub struct PlayerObsTracker {
    /// The information that each player has about the state of the game
    player_observations: BTreeMap<PlayerNum, ObsTracker>,
}

impl PlayerObsTracker {
    pub fn new(players: PlayerNum, dims: Dims) -> Self {
        let mut player_observations = BTreeMap::new();

        for p in 0..players {
            player_observations.insert(p, ObsTracker::new(dims));
        }

        Self {
            player_observations,
        }
    }

    /// Track an observation made by the given player at the specified location
    ///
    /// Returns Err(()) if no such player is recognized
    pub fn track(
        &mut self,
        player: PlayerNum,
        loc: Location,
        obs: Obs,
    ) -> Result<Option<Obs>, ObsTrackerError> {
        let observations = self
            .player_observations
            .get_mut(&player)
            .ok_or(ObsTrackerError::NoTrackerForPlayer(player))?;

        Ok(observations._track(loc, obs))
    }

    pub fn tracker(&self, player: PlayerNum) -> Option<&ObsTracker> {
        self.player_observations.get(&player)
    }

    pub fn tracker_mut(&mut self, player: PlayerNum) -> Option<&mut ObsTracker> {
        self.player_observations.get_mut(&player)
    }
}

pub fn visible_coords_iter(sight_distance: u16) -> impl Iterator<Item = Vec2d<i32>> {
    let sight_distance = i32::from(sight_distance);
    (-sight_distance..=sight_distance).flat_map(move |x| {
        let y_max = sight_distance - x.abs();
        (-y_max..=y_max).map(move |y| Vec2d::new(x, y))
    })
}

pub trait Observer: Located {
    fn sight_distance(&self) -> u16;

    /// FIXME If we ever get support for impl Trait on trait methods switch to that rather than the likely performance hit of this
    /// vector instantiation
    fn observe(
        &self,
        tiles: &dyn Source<Tile>,
        turn: TurnNum,
        action: ActionNum,
        wrapping: Wrap2d,
        obs_tracker: &mut ObsTracker,
    ) -> Vec<LocatedObs> {
        visible_coords_iter(self.sight_distance())
            .filter_map(|inc| wrapping.wrapped_add(tiles.dims(), self.loc(), inc))
            .map(|loc| obs_tracker.track_observation(loc, tiles.get(loc).unwrap(), turn, action))
            .collect()
        // for inc in visible_coords_iter(self.sight_distance()) {
        //     if let Some(loc) = wrapping.wrapped_add(tiles.dims(), self.loc(), inc) {
        //         obs_tracker.observe(loc, tiles.get(loc), turn);
        //     }
        // }
    }
    fn can_see(&self, loc: Location) -> bool {
        self.loc().manhattan_distance(loc) <= self.sight_distance() as u32
    }
}

#[cfg(test)]
mod test {
    use crate::{
        game::{
            map::{dijkstra::Source, LocationGrid, Terrain, Tile},
            obs::{Obs, ObsTracker, Observer},
            unit::{Unit, UnitID, UnitType},
            Alignment,
        },
        util::{Dims, Location, Wrap2d},
    };

    #[test]
    fn test_fog_of_war_tracker() {
        let dims = Dims {
            width: 10,
            height: 20,
        };
        let map: LocationGrid<Tile> =
            LocationGrid::new(dims, |loc| -> Tile { Tile::new(Terrain::Land, loc) });
        let mut tracker = ObsTracker::new(dims);
        let loc = Location { x: 5, y: 10 };
        assert_eq!(tracker.get(loc).cloned(), Some(Obs::Unobserved));
        assert_eq!(tracker.get(Location { x: 1000, y: 2000 }), None);

        let tile = Tile::new(Terrain::Land, loc);

        let turn = 0;
        let action_count = 0;

        tracker.track_observation(loc, &tile, turn, action_count);

        assert_eq!(
            tracker.get(loc).cloned(),
            Some(Obs::Observed {
                tile,
                turn,
                action_count,
                current: true
            })
        );

        let infantry = Unit::new(
            UnitID::new(0),
            loc,
            UnitType::Infantry,
            Alignment::Belligerent { player: 0 },
            "George Glover",
        );
        infantry.observe(&map, turn, action_count, Wrap2d::BOTH, &mut tracker);
    }

    #[test]
    pub fn test_num_observed() {
        let mut tracker = ObsTracker::new(Dims::new(10, 1));
        assert_eq!(tracker.num_observed(), 0);

        tracker._track(
            Location { x: 0, y: 0 },
            Obs::Observed {
                tile: Tile::new(Terrain::Land, Location { x: 0, y: 0 }),
                turn: 0,
                action_count: 0,
                current: true,
            },
        );

        assert_eq!(tracker.num_observed(), 1);

        tracker._track(
            Location { x: 0, y: 0 },
            Obs::Observed {
                tile: Tile::new(Terrain::Land, Location { x: 0, y: 0 }),
                turn: 0,
                action_count: 0,
                current: true,
            },
        );

        assert_eq!(tracker.num_observed(), 1);
    }
}
