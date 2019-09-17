//! Utility functions and structs

// pub fn safe_minus_one(x:u16) -> u16 {
//     if x > 0 { x - 1}
//     else { 0 }
// }
//
// pub fn safe_plus_one(x:u16, max:u16) -> u16 {
//     if x < max { x + 1 }
//     else { max }
// }

use std::convert::TryFrom;
use std::fmt;
use std::ops::Add;
use std::thread::sleep;
use std::time::Duration;

use unicode_segmentation::UnicodeSegmentation;

use conf;

#[derive(Clone,Copy,Debug)]
pub struct Rect {
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16
}

impl Rect {
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

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Dims {
    pub width: u16,
    pub height: u16
}

impl Dims {
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    pub fn in_bounds(self, loc: Location) -> bool {
        // loc.x >= 0 && 
        // loc.y >= 0 &&
        loc.x < self.width && loc.y < self.height
    }
}

impl fmt::Display for Dims {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.width.fmt(f)
        .and(write!(f, "x"))
        .and(self.height.fmt(f))
    }
}

#[derive(Clone,Copy,Eq,PartialEq,Hash,PartialOrd,Ord)]
pub struct Vec2d<T> {
    pub x: T,
    pub y: T
}

impl<T> Vec2d<T> {
    pub fn new(x: T, y: T) -> Self {
        Vec2d{ x, y }
    }
}

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

#[derive(Clone,Copy)]
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
}

impl TryFrom<char> for Direction {
    type Error = String;
    fn try_from(c: char) -> Result<Direction,String> {
        match c {
            conf::KEY_UP                        => Ok(Direction::Up),
            conf::KEY_DOWN                      => Ok(Direction::Down),
            conf::KEY_LEFT                      => Ok(Direction::Left),
            conf::KEY_RIGHT                     => Ok(Direction::Right),
            conf::KEY_UP_LEFT                   => Ok(Direction::UpLeft),
            conf::KEY_UP_RIGHT                  => Ok(Direction::UpRight),
            conf::KEY_DOWN_LEFT                 => Ok(Direction::DownLeft),
            conf::KEY_DOWN_RIGHT                => Ok(Direction::DownRight),
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

#[derive(Clone,Copy)]
pub enum Wrap {
    Wrapping,
    NonWrapping
}

#[derive(Clone,Copy)]
pub struct Wrap2d {
    pub horiz: Wrap,
    pub vert: Wrap
}

#[allow(dead_code)]
pub static WRAP_BOTH: Wrap2d = Wrap2d{
    horiz: Wrap::Wrapping,
    vert: Wrap::Wrapping
};

#[allow(dead_code)]
pub static WRAP_HORIZ: Wrap2d = Wrap2d {
    horiz: Wrap::Wrapping,
    vert: Wrap::NonWrapping
};

#[allow(dead_code)]
pub static WRAP_VERT: Wrap2d = Wrap2d {
    horiz: Wrap::NonWrapping,
    vert: Wrap::Wrapping
};

#[allow(dead_code)]
pub static WRAP_NEITHER: Wrap2d = Wrap2d {
    horiz: Wrap::NonWrapping,
    vert: Wrap::NonWrapping
};

///
/// Add `inc` to `loc` respecting the specified wrapping rules in a space defined by `dims`
/// If the result is out of bounds, return None
///
pub fn wrapped_add(loc: Location, inc: Vec2d<i32>, dims: Dims, wrapping: Wrap2d) -> Option<Location> {
    let mut new_x: i32 = i32::from(loc.x) + inc.x;
    if let Wrap::Wrapping = wrapping.horiz {
        if new_x < 0 {
            loop {
                new_x += i32::from(dims.width);
                if new_x >= 0 {
                    break;
                }
            }
        } else {
            new_x %= i32::from(dims.width);
        }
    } else if new_x < 0 || new_x >= i32::from(dims.width) {
        return None;
    }

    let mut new_y: i32 = i32::from(loc.y) + inc.y;
    if let Wrap::Wrapping = wrapping.vert {
        if new_y < 0 {
            loop {
                new_y += i32::from(dims.height);
                if new_y >= 0 {
                    break;
                }
            }
        } else {
            new_y %= i32::from(dims.height);
        }
    } else if new_y < 0 || new_y >= i32::from(dims.height) {
        return None;
    }

    Some(Location {
        x: new_x as u16,
        y: new_y as u16
    })
}

pub type Location = Vec2d<u16>;

impl Location {
    pub fn shift_wrapped(self, dir: Direction, dims: Dims, wrapping: Wrap2d) -> Option<Location> {
        wrapped_add(self, dir.vec2d(), dims, wrapping)
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
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

#[cfg(test)]
mod test {
    use crate::{
        game::map::dijkstra::RELATIVE_NEIGHBORS,
        util::{Dims,Location,Vec2d,wrapped_add,WRAP_NEITHER,WRAP_VERT,WRAP_HORIZ,WRAP_BOTH}
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
            assert_eq!( wrapped_add(loc, *rel_neighb, dims, WRAP_BOTH),    results_both   [i] );
            assert_eq!( wrapped_add(loc, *rel_neighb, dims, WRAP_HORIZ),   results_horiz  [i] );
            assert_eq!( wrapped_add(loc, *rel_neighb, dims, WRAP_VERT),    results_vert   [i] );
            assert_eq!( wrapped_add(loc, *rel_neighb, dims, WRAP_NEITHER), results_neither[i] );
        }

        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:-1, y:0}, Dims{width:5, height:1}, WRAP_HORIZ), Some(Location{x:4, y:0}));
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:-10, y:0}, Dims{width:5, height:1}, WRAP_HORIZ), Some(Location{x:0, y:0}));
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:-8, y:0}, Dims{width:5, height:1}, WRAP_HORIZ), Some(Location{x:2, y:0}));
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:-1, y:0}, Dims{width:5, height:1}, WRAP_NEITHER), None);
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:-10, y:0}, Dims{width:5, height:1}, WRAP_NEITHER), None);
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:-8, y:0}, Dims{width:5, height:1}, WRAP_NEITHER), None);

        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:0, y:-1}, Dims{width:1, height:5}, WRAP_VERT), Some(Location{x:0, y:4}));
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:0, y:-10}, Dims{width:1, height:5}, WRAP_VERT), Some(Location{x:0, y:0}));
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:0, y:-8}, Dims{width:1, height:5}, WRAP_VERT), Some(Location{x:0, y:2}));
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:0, y:-1}, Dims{width:1, height:5}, WRAP_NEITHER), None);
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:0, y:-10}, Dims{width:1, height:5}, WRAP_NEITHER), None);
        assert_eq!(wrapped_add(Location{x:0, y:0}, Vec2d{x:0, y:-8}, Dims{width:1, height:5}, WRAP_NEITHER), None);
    }
}
