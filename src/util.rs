//! Utility functions and structs

use std::{
    cmp::Ordering,
    convert::TryFrom,
    fmt,
    mem,
    ops::{
        Add,
        Sub,
    },
    thread::sleep,
    time::Duration, collections::HashMap,
};

use failure::Fail;

use rand::{
    Rng,
    distributions::Distribution,
};

use serde::{Deserialize,Serialize};

use unicode_segmentation::UnicodeSegmentation;

use crate::conf;

/// A location in a non-negative coordinate space such as the game map or viewport
pub type Location = Vec2d<u16>;

/// An increment or delta on `Location`s.
pub type Inc = Vec2d<i32>;

#[derive(Clone,Copy,Debug)]
pub struct Rect {
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16
}

impl Rect {
    pub fn new(left: u16, top: u16, width: u16, height: u16) -> Self {
        Self { left, top, width, height }
    }
    pub fn right(self) -> u16 { self.left + self.width }
    pub fn bottom(self) -> u16 { self.top + self.height }

    pub fn center(self) -> Location {
        Location {
            x: self.left + self.width / 2,
            y: self.top + self.height / 2,
        }
    }

    pub fn dims(self) -> Dims {
        Dims{ width: self.width, height: self.height }
    }
}

/// Dimensions in a two-dimensional space
/// 
/// This can be thought of as a rectangle with a particular width and height, but not located at any
/// particular point in space.
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Dims {
    pub width: u16,
    pub height: u16
}

impl Dims {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// Is the location `loc` contained within these dimensions?
    /// 
    /// More specifically, if these dimensions are taken to define a rectangle with one corner at the origin (0,0),
    /// then is the cartesian point represented by location `loc` contained by that rectangle?
    pub fn contain(self, loc: Location) -> bool {
        loc.x < self.width && loc.y < self.height
    }

    /// The area of a rectangle with these dimensions
    pub fn area(self) ->  u32 {
        u32::from(self.width) * u32::from(self.height)
    }

    /// Iterate through all `Location`s implied by placing the rectangle of these dimensions at the origin
    pub fn iter_locs(self) -> impl Iterator<Item=Location> {
        self.iter_locs_column_major()
    }

    /// Iterate through all `Location`s implied by placing the rectangle of these dimensions at the origin, in column-
    /// major order.
    pub fn iter_locs_column_major(self) -> impl Iterator<Item=Location> {
        let width: u16 = self.width;
        let height: u16 = self.height;
        (0..width).flat_map(move |x| {
            (0..height).map(move |y| {
                Location{x,y}
            })
        })
    }
}

impl Distribution<Location> for Dims {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Location {
        Location::new(
            rng.gen_range(0, self.width),
            rng.gen_range(0, self.height),
        )
    }
}

// TODO Implement IntoIterator for Dims
// This will be most easily accomplished when impl Trait in type aliases are stabilized
// https://github.com/rust-lang/rust/issues/63063
// impl IntoIterator for Dims {
//     type Item=Location;
//     type IntoIter=impl Iterator<Item=Location>;

//     fn into_iter(self) -> Self::IntoIter {
//         self.iter_locs()
//     }
// }

impl fmt::Display for Dims {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.width.fmt(f)
        .and(write!(f, "x"))
        .and(self.height.fmt(f))
    }
}

pub trait Dimensioned {
    fn dims(&self) -> Dims;
}

#[derive(Clone,Copy,Eq,PartialEq,Hash,PartialOrd,Ord)]
pub struct Vec2d<T> {
    pub x: T,
    pub y: T
}

impl<T> Vec2d<T> {
    pub const fn new(x: T, y: T) -> Self {
        Vec2d{ x, y }
    }
}

//TODO Someday when there's a `const` version of `Add`, implement it; could be useful for combining Vec2d's in const contexts
impl<N:Add<Output=N>> Add for Vec2d<N> {
    type Output = Vec2d<N>;
    fn add(self, rhs: Vec2d<N>) -> Vec2d<N> {
        Vec2d {
            x: self.x + rhs.x,
            y: self.y + rhs.y
        }
    }
}

impl <T:fmt::Display> fmt::Display for Vec2d<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")
        .and(self.x.fmt(f))
        .and(write!(f, ","))
        .and(self.y.fmt(f))
        .and(write!(f, ")"))
    }
}

impl <T:fmt::Display> fmt::Debug for Vec2d<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[derive(Clone,Copy,Debug,Deserialize,Eq,Hash,Ord,PartialEq,PartialOrd,Serialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight
}

impl Direction {
    pub const fn values() -> [Self; 8] {
        [
            Self::Up,
            Self::Down,
            Self::Left,
            Self::Right,
            Self::UpLeft,
            Self::UpRight,
            Self::DownLeft,
            Self::DownRight,
        ]
    }

    #[deprecated]
    pub fn vec2d(self) -> Vec2d<i32> {
        match self {
            Direction::Up         => Vec2d{x: 0, y:-1},
            Direction::Down       => Vec2d{x: 0, y: 1},
            Direction::Left       => Vec2d{x:-1, y: 0},
            Direction::Right      => Vec2d{x: 1, y: 0},
            Direction::UpLeft     => Vec2d{x:-1, y:-1},
            Direction::UpRight    => Vec2d{x: 1, y:-1},
            Direction::DownLeft   => Vec2d{x:-1, y: 1},
            Direction::DownRight  => Vec2d{x: 1, y: 1}
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Up         => Self::Down,
            Self::Down       => Self::Up,
            Self::Left       => Self::Right,
            Self::Right      => Self::Left,
            Self::UpLeft     => Self::DownRight,
            Self::UpRight    => Self::DownLeft,
            Self::DownLeft   => Self::UpRight,
            Self::DownRight  => Self::UpLeft,
        }
    }
}

impl Into<Vec2d<i32>> for Direction {
    fn into(self) -> Vec2d<i32> {
        match self {
            Direction::Up         => Vec2d{x: 0, y:-1},
            Direction::Down       => Vec2d{x: 0, y: 1},
            Direction::Left       => Vec2d{x:-1, y: 0},
            Direction::Right      => Vec2d{x: 1, y: 0},
            Direction::UpLeft     => Vec2d{x:-1, y:-1},
            Direction::UpRight    => Vec2d{x: 1, y:-1},
            Direction::DownLeft   => Vec2d{x:-1, y: 1},
            Direction::DownRight  => Vec2d{x: 1, y: 1}
        }
    }
}

impl TryFrom<Vec2d<i32>> for Direction {
    type Error = ();

    /// Turns a vector back into a direction if possible
    /// 
    /// If not, returns Err(())
    fn try_from(vec: Vec2d<i32>) -> Result<Self,Self::Error> {
        match vec {
            Vec2d{x: 0, y:-1} => Ok(Direction::Up),
            Vec2d{x: 0, y: 1} => Ok(Direction::Down),
            Vec2d{x:-1, y: 0} => Ok(Direction::Left),
            Vec2d{x: 1, y: 0} => Ok(Direction::Right),
            Vec2d{x:-1, y:-1} => Ok(Direction::UpLeft),
            Vec2d{x: 1, y:-1} => Ok(Direction::UpRight),
            Vec2d{x:-1, y: 1} => Ok(Direction::DownLeft),
            Vec2d{x: 1, y: 1} => Ok(Direction::DownRight),
            _ => Err(())
        }
    }
}

impl TryFrom<char> for Direction {
    type Error = String;
    fn try_from(c: char) -> Result<Direction,String> {
        match c {
            conf::KEY_UP         | conf::KEY_NUMPAD_UP         => Ok(Direction::Up),
            conf::KEY_DOWN       | conf::KEY_NUMPAD_DOWN       => Ok(Direction::Down),
            conf::KEY_LEFT       | conf::KEY_NUMPAD_LEFT       => Ok(Direction::Left),
            conf::KEY_RIGHT      | conf::KEY_NUMPAD_RIGHT      => Ok(Direction::Right),
            conf::KEY_UP_LEFT    | conf::KEY_NUMPAD_UP_LEFT    => Ok(Direction::UpLeft),
            conf::KEY_UP_RIGHT   | conf::KEY_NUMPAD_UP_RIGHT   => Ok(Direction::UpRight),
            conf::KEY_DOWN_LEFT  | conf::KEY_NUMPAD_DOWN_LEFT  => Ok(Direction::DownLeft),
            conf::KEY_DOWN_RIGHT | conf::KEY_NUMPAD_DOWN_RIGHT => Ok(Direction::DownRight),
            _                    => Err(format!("{} doesn't indicate a direction", c))
        }
    }
}

impl Direction {
    pub fn try_from_viewport_shift(c: char) -> Result<Direction,String> {
        match c {
            conf::KEY_VIEWPORT_SHIFT_UP         => Ok(Direction::Up),
            conf::KEY_VIEWPORT_SHIFT_DOWN       => Ok(Direction::Down),
            conf::KEY_VIEWPORT_SHIFT_LEFT       => Ok(Direction::Left),
            conf::KEY_VIEWPORT_SHIFT_RIGHT      => Ok(Direction::Right),
            conf::KEY_VIEWPORT_SHIFT_UP_LEFT    => Ok(Direction::UpLeft),
            conf::KEY_VIEWPORT_SHIFT_UP_RIGHT   => Ok(Direction::UpRight),
            conf::KEY_VIEWPORT_SHIFT_DOWN_LEFT  => Ok(Direction::DownLeft),
            conf::KEY_VIEWPORT_SHIFT_DOWN_RIGHT => Ok(Direction::DownRight),
            _                    => Err(format!("{} doesn't indicate a direction", c))
        }
    }
}

#[derive(Debug,Fail,PartialEq)]
enum WrapError {
    #[fail(display = "coord0? {}. Coordinate with value {} is out of the bounds 0 to {}", coord0, coord, dimension_width)]
    OutOfBounds { coord0: bool, coord: u16, dimension_width: u16},
}

#[derive(Clone,Copy,Debug)]
pub enum Wrap {
    Wrapping,
    NonWrapping
}

impl Wrap {
    /// Add an increment to a coordinate in a dimension of a particular width, respecting wrapping rules.
    /// If out of bounds in a non-wrapping mode, returns None
    pub fn wrapped_add(self, dimension_width: u16, coord: u16, inc: i32) -> Option<u16> {
        let dimension_width = i32::from(dimension_width);
        let mut new_coord: i32 = i32::from(coord) + inc;
        match self {
            Wrap::Wrapping => {
                if new_coord < 0 {
                    loop {
                        new_coord += dimension_width;
                        if new_coord >= 0 {
                            break;
                        }
                    }
                } else {
                    new_coord %= dimension_width;
                }
            },
            Wrap::NonWrapping => {
                if new_coord < 0 || new_coord >= dimension_width {
                    return None;
                }
            },
        }
        Some(new_coord as u16)
    }

    /// Get the vector that transforms the coord0 to coord1, taking wrapping into consideration. In other words, calculate
    /// `coord1 - coord0` thinking in terms of wrapping. This gives the minimal distance.
    fn wrapped_sub(self, dimension_width: u16, coord0: u16, coord1: u16) -> Result<i32,WrapError> {
        if coord0 >= dimension_width {
            return Err(WrapError::OutOfBounds{coord0: true, coord: coord0, dimension_width});
        }

        if coord1 >= dimension_width {
            return Err(WrapError::OutOfBounds{coord0: false, coord: coord1, dimension_width});
        }

        let mut coord0 = coord0 as i32;
        let mut coord1 = coord1 as i32;

        let cmp = coord0.cmp(&coord1);
        if let Ordering::Equal = cmp {
            return Ok(0);
        }

        if let Ordering::Greater = cmp {
            mem::swap(&mut coord0, &mut coord1);
        }

        let dimension_width = dimension_width as i32;
        let mut best_result = coord1 - coord0;

        if let Self::Wrapping = self {
            let wrapped_result = - coord0 - dimension_width + coord1;

            if wrapped_result.abs() < best_result.abs() {
                best_result = wrapped_result;
            }
        }

        if let Ordering::Greater = cmp {
            best_result *= -1;
        }

        Ok(best_result)
    }
}

#[derive(Debug,Fail,PartialEq)]
pub enum Wrap2dError {
    #[fail(display = "Location {} is out of the bounds of dimensions {}", loc, dims)]
    OutOfBounds { loc: Location, dims: Dims },
}

#[derive(Clone,Copy,Debug)]
pub struct Wrap2d {
    pub horiz: Wrap,
    pub vert: Wrap
}

impl Wrap2d {
    pub fn values() -> [Self; 4] {
        [
            Self::NEITHER,
            Self::HORIZ,
            Self::VERT,
            Self::BOTH,
        ]
    }
    pub const BOTH: Self = Self {
        horiz: Wrap::Wrapping,
        vert: Wrap::Wrapping
    };

    pub const HORIZ: Self = Self {
        horiz: Wrap::Wrapping,
        vert: Wrap::NonWrapping
    };

    pub const VERT: Self = Self {
        horiz: Wrap::NonWrapping,
        vert: Wrap::Wrapping
    };

    pub const NEITHER: Self = Self {
        horiz: Wrap::NonWrapping,
        vert: Wrap::NonWrapping
    };

    ///
    /// Add `inc` to `loc` respecting these wrapping rules in a space defined by `dims`.
    /// If the result is out of bounds, return None
    ///
    pub fn wrapped_add(self, dims: Dims, loc: Location, inc: Vec2d<i32>) -> Option<Location> {
        let new_x = self.horiz.wrapped_add(dims.width, loc.x, inc.x)?;
        self.vert.wrapped_add(dims.height, loc.y, inc.y).map(|new_y| {
            Location {
                x: new_x,
                y: new_y,
            }
        })
    }

    //// Subtract `loc0` from `loc1` respecting the wrapping rules. This basically means we will search for the
    /// smallest answer we can give (the way of seeing the two points as nearest on both dimensions independently)
    /// that respects the wrapping rules, whether that's via a wrapped subtraction or a normal subtraction
    pub fn wrapped_sub(self, dims: Dims, loc0: Location, loc1: Location) -> Result<Vec2d<i32>,Wrap2dError> {
        let inc_x = self.horiz.wrapped_sub(dims.width, loc0.x, loc1.x)
        .map_err(|err| match err {
            WrapError::OutOfBounds { coord0: true, .. } => Wrap2dError::OutOfBounds{loc: loc0, dims},
            WrapError::OutOfBounds { coord0: false, .. } => Wrap2dError::OutOfBounds{loc: loc1, dims},
        })?;

        self.vert.wrapped_sub(dims.height, loc0.y, loc1.y)
        .map(|inc_y| Vec2d::new(inc_x, inc_y))
        .map_err(|err| match err {
            WrapError::OutOfBounds { coord0: true, .. } => Wrap2dError::OutOfBounds{loc: loc0, dims},
            WrapError::OutOfBounds { coord0: false, .. } => Wrap2dError::OutOfBounds{loc: loc1, dims},
        })
    }
}

impl TryFrom<&str> for Wrap2d {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(
            match value {
                "h" => Wrap2d{horiz: Wrap::Wrapping, vert: Wrap::NonWrapping},
                "v" => Wrap2d{horiz: Wrap::NonWrapping, vert: Wrap::Wrapping},
                "b" => Wrap2d{horiz: Wrap::Wrapping, vert: Wrap::Wrapping},
                "n" => Wrap2d{horiz: Wrap::NonWrapping, vert: Wrap::NonWrapping},
                w => return Err(format!("Unrecognized wrapping specification '{}'", w))
            }
        )
    }
}

// ///
// /// Add `inc` to `loc` respecting the specified wrapping rules in a space defined by `dims`
// /// If the result is out of bounds, return None
// ///
// pub fn wrapped_add(loc: Location, inc: Vec2d<i32>, dims: Dims, wrapping: Wrap2d) -> Option<Location> {
//     let mut new_x: i32 = i32::from(loc.x) + inc.x;
//     if let Wrap::Wrapping = wrapping.horiz {
//         if new_x < 0 {
//             loop {
//                 new_x += i32::from(dims.width);
//                 if new_x >= 0 {
//                     break;
//                 }
//             }
//         } else {
//             new_x %= i32::from(dims.width);
//         }
//     } else if new_x < 0 || new_x >= i32::from(dims.width) {
//         return None;
//     }

//     let mut new_y: i32 = i32::from(loc.y) + inc.y;
//     if let Wrap::Wrapping = wrapping.vert {
//         if new_y < 0 {
//             loop {
//                 new_y += i32::from(dims.height);
//                 if new_y >= 0 {
//                     break;
//                 }
//             }
//         } else {
//             new_y %= i32::from(dims.height);
//         }
//     } else if new_y < 0 || new_y >= i32::from(dims.height) {
//         return None;
//     }

//     Some(Location {
//         x: new_x as u16,
//         y: new_y as u16
//     })
// }

impl Location {
    pub fn shift_wrapped(self, dir: Direction, dims: Dims, wrapping: Wrap2d) -> Option<Location> {
        wrapping.wrapped_add(dims, self, dir.into())
    }

    pub fn dist(self, other: Location) -> f64 {
        (
            (self.x as f64 - other.x as f64).powf(2.0) +
            (self.y as f64 - other.y as f64).powf(2.0)
        ).sqrt()
    }
}

impl Sub for Location {
    type Output = Vec2d<i32>;

    fn sub(self, rhs: Location) -> Vec2d<i32> {
        Vec2d {
            x: self.x as i32 - rhs.x as i32,
            y: self.y as i32 - rhs.y as i32
        }
    }
}

impl Into<Vec2d<i32>> for Location {
    fn into(self) -> Vec2d<i32> {
        Vec2d {
            x: self.x as i32,
            y: self.y as i32,
        }
    }
}

pub trait Located {
    fn loc(&self) -> Location;
}

#[derive(Debug,PartialEq)]
pub struct LocatedItem<T> {
    pub loc: Location,
    pub item: T,
}
impl <T> LocatedItem<T> {
    pub fn new(loc: Location, item: T) -> Self {
        Self { loc, item }
    }
}
impl <T> Located for LocatedItem<T> {
    fn loc(&self) -> Location {
        self.loc
    }
}

pub fn sleep_millis(millis: u64) {
    sleep(Duration::from_millis(millis));
}

pub fn grapheme_substr(s: &str, len: usize) -> String {
    let mut substr = String::with_capacity(len);

    for grapheme in UnicodeSegmentation::graphemes(s, true).take(len) {
        substr.push_str(grapheme);
    }

    substr
}

pub fn grapheme_len(s: &str) -> usize {
    UnicodeSegmentation::graphemes(s, true).count()
}

pub fn sparsify(v: Vec<f64>) -> (usize,HashMap<usize,f64>) {
    let num_features = v.len();
    let features: HashMap<usize,f64> = v.iter()
                                                .cloned()
                                                .enumerate()
                                                .filter(|(i,f)| *f != 0.0)
                                                .collect();
    (num_features, features)
}

#[cfg(test)]
mod test {
    use crate::{
        game::map::dijkstra::RELATIVE_NEIGHBORS,
    };

    use super::{
        Dims,
        Location,
        Vec2d,
        Wrap,
        Wrap2d,
    };

    #[test]
    fn test_wrapped_add() {
    /*
        xxxx* 5
        xxxxx 4
        xxxxx 3
        xxxxx 2
        xxxxx 1
        xxxxx 0

        01234
    */
        let dims = Dims{width: 5, height: 6};
        let loc = Location{x: 4, y: 5};

        let results_both: [Option<Location>; 8] = [
        // Vec2d { x: -1, y: -1 },
            Some(Location{x:3, y:4}),
        // Vec2d { x: -1, y:  0 },
            Some(Location{x:3, y:5}),
        // Vec2d { x: -1, y:  1 },
            Some(Location{x:3, y:0}),
        // Vec2d { x:  0, y: -1 },
            Some(Location{x:4, y:4}),
        // Vec2d { x:  0, y:  1 },
            Some(Location{x:4, y:0}),
        // Vec2d { x:  1, y: -1 },
            Some(Location{x:0, y:4}),
        // Vec2d { x:  1, y:  0 },
            Some(Location{x:0, y:5}),
        // Vec2d { x:  1, y:  1}
            Some(Location{x:0, y:0})
        ];

        let results_horiz: [Option<Location>; 8] = [
        // Vec2d { x: -1, y: -1 },
            Some(Location{x:3, y:4}),
        // Vec2d { x: -1, y:  0 },
            Some(Location{x:3, y:5}),
        // Vec2d { x: -1, y:  1 },
            None,
        // Vec2d { x:  0, y: -1 },
            Some(Location{x:4, y:4}),
        // Vec2d { x:  0, y:  1 },
            None,
        // Vec2d { x:  1, y: -1 },
            Some(Location{x:0, y:4}),
        // Vec2d { x:  1, y:  0 },
            Some(Location{x:0, y:5}),
        // Vec2d { x:  1, y:  1}
            None
        ];

        let results_vert: [Option<Location>; 8] = [
        // Vec2d { x: -1, y: -1 },
            Some(Location{x:3, y:4}),
        // Vec2d { x: -1, y:  0 },
            Some(Location{x:3, y:5}),
        // Vec2d { x: -1, y:  1 },
            Some(Location{x:3, y:0}),
        // Vec2d { x:  0, y: -1 },
            Some(Location{x:4, y:4}),
        // Vec2d { x:  0, y:  1 },
            Some(Location{x:4, y:0}),
        // Vec2d { x:  1, y: -1 },
            None,
        // Vec2d { x:  1, y:  0 },
            None,
        // Vec2d { x:  1, y:  1}
            None
        ];

        let results_neither: [Option<Location>; 8] = [
        // Vec2d { x: -1, y: -1 },
            Some(Location{x:3, y:4}),
        // Vec2d { x: -1, y:  0 },
            Some(Location{x:3, y:5}),
        // Vec2d { x: -1, y:  1 },
            None,
        // Vec2d { x:  0, y: -1 },
            Some(Location{x:4, y:4}),
        // Vec2d { x:  0, y:  1 },
            None,
        // Vec2d { x:  1, y: -1 },
            None,
        // Vec2d { x:  1, y:  0 },
            None,
        // Vec2d { x:  1, y:  1}
            None
        ];

        for (i, rel_neighb) in RELATIVE_NEIGHBORS.iter().enumerate() {
            assert_eq!( Wrap2d::BOTH.wrapped_add(dims, loc, *rel_neighb),    results_both   [i] );
            assert_eq!( Wrap2d::HORIZ.wrapped_add(dims, loc, *rel_neighb),   results_horiz  [i] );
            assert_eq!( Wrap2d::VERT.wrapped_add(dims, loc, *rel_neighb),    results_vert   [i] );
            assert_eq!( Wrap2d::NEITHER.wrapped_add(dims, loc, *rel_neighb), results_neither[i] );
        }

        assert_eq!(Wrap2d::HORIZ.wrapped_add(Dims{width:5, height:1}, Location{x:0, y:0}, Vec2d{x:-1, y:0}), Some(Location{x:4, y:0}));
        assert_eq!(Wrap2d::HORIZ.wrapped_add(Dims{width:5, height:1}, Location{x:0, y:0}, Vec2d{x:-10, y:0}), Some(Location{x:0, y:0}));
        assert_eq!(Wrap2d::HORIZ.wrapped_add(Dims{width:5, height:1}, Location{x:0, y:0}, Vec2d{x:-8, y:0}), Some(Location{x:2, y:0}));

        assert_eq!(Wrap2d::NEITHER.wrapped_add(Dims{width:5, height:1}, Location{x:0, y:0}, Vec2d{x:-1, y:0}), None);
        assert_eq!(Wrap2d::NEITHER.wrapped_add(Dims{width:5, height:1}, Location{x:0, y:0}, Vec2d{x:-10, y:0}), None);
        assert_eq!(Wrap2d::NEITHER.wrapped_add(Dims{width:5, height:1}, Location{x:0, y:0}, Vec2d{x:-8, y:0}), None);

        assert_eq!(Wrap2d::VERT.wrapped_add(Dims{width:1, height:5}, Location{x:0, y:0}, Vec2d{x:0, y:-1}), Some(Location{x:0, y:4}));
        assert_eq!(Wrap2d::VERT.wrapped_add(Dims{width:1, height:5}, Location{x:0, y:0}, Vec2d{x:0, y:-10}), Some(Location{x:0, y:0}));
        assert_eq!(Wrap2d::VERT.wrapped_add(Dims{width:1, height:5}, Location{x:0, y:0}, Vec2d{x:0, y:-8}), Some(Location{x:0, y:2}));

        assert_eq!(Wrap2d::NEITHER.wrapped_add(Dims{width:1, height:5}, Location{x:0, y:0}, Vec2d{x:0, y:-1}), None);
        assert_eq!(Wrap2d::NEITHER.wrapped_add(Dims{width:1, height:5}, Location{x:0, y:0}, Vec2d{x:0, y:-10}), None);
        assert_eq!(Wrap2d::NEITHER.wrapped_add(Dims{width:1, height:5}, Location{x:0, y:0}, Vec2d{x:0, y:-8}), None);
    }

    #[test]
    fn test_wrapped_add_1x1() {
        let dims = Dims{width: 1, height: 1};
        let loc = Location::new(0,0);

        let results_horiz: [Option<Location>; 8] = [
        // Vec2d { x: -1, y: -1 },
            None,
        // Vec2d { x: -1, y:  0 },
            Some(Location{x:0, y:0}),
        // Vec2d { x: -1, y:  1 },
            None,
        // Vec2d { x:  0, y: -1 },
            None,
        // Vec2d { x:  0, y:  1 },
            None,
        // Vec2d { x:  1, y: -1 },
            None,
        // Vec2d { x:  1, y:  0 },
            Some(Location{x:0, y:0}),
        // Vec2d { x:  1, y:  1}
            None
        ];

        let results_vert: [Option<Location>; 8] = [
        // Vec2d { x: -1, y: -1 },
            None,
        // Vec2d { x: -1, y:  0 },
            None,
        // Vec2d { x: -1, y:  1 },
            None,
        // Vec2d { x:  0, y: -1 },
            Some(Location{x:0, y:0}),
        // Vec2d { x:  0, y:  1 },
            Some(Location{x:0, y:0}),
        // Vec2d { x:  1, y: -1 },
            None,
        // Vec2d { x:  1, y:  0 },
            None,
        // Vec2d { x:  1, y:  1}
            None
        ];

        for (i, rel_neighb) in RELATIVE_NEIGHBORS.iter().enumerate() {
            assert_eq!( Wrap2d::BOTH.wrapped_add(dims, loc, *rel_neighb),    Some(Location{x:0, y:0}) );
            assert_eq!( Wrap2d::HORIZ.wrapped_add(dims, loc, *rel_neighb),   results_horiz  [i] );
            assert_eq!( Wrap2d::VERT.wrapped_add(dims, loc, *rel_neighb),    results_vert   [i] );
            assert_eq!( Wrap2d::NEITHER.wrapped_add(dims, loc, *rel_neighb), None );
        }

    }

    #[test]
    fn test_wrapped_sub_1d() {

        /* 0xxx1x */

        let dimension_width = 6;
        let coord0 = 0;
        let coord1 = 4;
        assert_eq!(Wrap::Wrapping.wrapped_sub(dimension_width, coord0, coord1), Ok(-2));
        assert_eq!(Wrap::NonWrapping.wrapped_sub(dimension_width, coord0, coord1), Ok(4));
        assert_eq!(Wrap::Wrapping.wrapped_sub(dimension_width, coord1, coord0), Ok(2));
        assert_eq!(Wrap::NonWrapping.wrapped_sub(dimension_width, coord1, coord0), Ok(-4));

        assert!(Wrap::Wrapping.wrapped_sub(5, 5, 0).is_err());
        assert!(Wrap::Wrapping.wrapped_sub(5, 0, 5).is_err());
        assert!(Wrap::Wrapping.wrapped_sub(5, 5, 5).is_err());
        assert!(Wrap::Wrapping.wrapped_sub(5, 0, 0).is_ok());
    }
    

    #[test]
    fn test_wrapped_sub_2d() {
    /*
        xxxxx 5
        xxx1x 4
        xxxxx 3
        xxxxx 2
        xxxxx 1
        0xxxx 0

        01234
    */
        let dims = Dims{width: 5, height: 6};
        let loc0 = Location::new(0, 0);
        let loc1 = Location::new(3, 4);

        let bad0 = Location::new(dims.width, 0);
        let bad1 = Location::new(0, dims.height);
        let bad2 = Location::new(dims.width, dims.height);


        assert_eq!(Wrap2d::BOTH.wrapped_sub(dims, loc0, loc1), Ok(Vec2d::new(-2, -2)));
        assert_eq!(Wrap2d::HORIZ.wrapped_sub(dims, loc0, loc1), Ok(Vec2d::new(-2, 4)));
        assert_eq!(Wrap2d::VERT.wrapped_sub(dims, loc0, loc1), Ok(Vec2d::new(3, -2)));
        assert_eq!(Wrap2d::NEITHER.wrapped_sub(dims, loc0, loc1), Ok(Vec2d::new(3, 4)));

        assert_eq!(Wrap2d::BOTH.wrapped_sub(dims, loc1, loc0), Ok(Vec2d::new(2, 2)));
        assert_eq!(Wrap2d::HORIZ.wrapped_sub(dims, loc1, loc0), Ok(Vec2d::new(2, -4)));
        assert_eq!(Wrap2d::VERT.wrapped_sub(dims, loc1, loc0), Ok(Vec2d::new(-3, 2)));
        assert_eq!(Wrap2d::NEITHER.wrapped_sub(dims, loc1, loc0), Ok(Vec2d::new(-3, -4)));
        
        for wrapping in Wrap2d::values().iter() {
            for loc in [loc0, loc1].iter().cloned() {

                for loc_ in [loc0, loc1].iter().cloned() {
                    assert!(wrapping.wrapped_sub(dims, loc, loc_).is_ok());
                    assert!(wrapping.wrapped_sub(dims, loc_, loc).is_ok());
                }

                for bad in [bad0, bad1, bad2].iter().cloned() {
                    assert!(wrapping.wrapped_sub(dims, bad, loc).is_err());
                    assert!(wrapping.wrapped_sub(dims, loc, bad).is_err());
                }
            }
        }
    }

    #[test]
    fn test_iter_locs_column_major() {
        let dims = Dims::new(3, 5);
        let mut it = dims.iter_locs_column_major();
        assert_eq!(it.next(), Some(Location::new(0, 0)));
        assert_eq!(it.next(), Some(Location::new(0, 1)));
        assert_eq!(it.next(), Some(Location::new(0, 2)));
        assert_eq!(it.next(), Some(Location::new(0, 3)));
        assert_eq!(it.next(), Some(Location::new(0, 4)));
        assert_eq!(it.next(), Some(Location::new(1, 0)));
        assert_eq!(it.next(), Some(Location::new(1, 1)));
        assert_eq!(it.next(), Some(Location::new(1, 2)));
        assert_eq!(it.next(), Some(Location::new(1, 3)));
        assert_eq!(it.next(), Some(Location::new(1, 4)));
        assert_eq!(it.next(), Some(Location::new(2, 0)));
        assert_eq!(it.next(), Some(Location::new(2, 1)));
        assert_eq!(it.next(), Some(Location::new(2, 2)));
        assert_eq!(it.next(), Some(Location::new(2, 3)));
        assert_eq!(it.next(), Some(Location::new(2, 4)));
        assert_eq!(it.next(), None);
    }
}
