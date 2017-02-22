//!
//! Utility functions and structs
//!

// pub fn safe_minus_one(x:u16) -> u16 {
//     if x > 0 { x - 1}
//     else { 0 }
// }
//
// pub fn safe_plus_one(x:u16, max:u16) -> u16 {
//     if x < max { x + 1 }
//     else { max }
// }

use std::cmp::{Ordering,min};
use std::convert::TryFrom;
use std::fmt;
use std::ops::Add;

use conf;

#[derive(Clone,Copy)]
pub struct Rect {
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16
}

impl Rect {
    pub fn left(&self) -> u16 { self.left }
    pub fn right(&self) -> u16 { self.left + self.width }
    pub fn top(&self) -> u16 { self.top }
    pub fn bottom(&self) -> u16 { self.top + self.height }
    pub fn width(&self) -> u16 { self.width }
    pub fn height(&self) -> u16 { self.height }
}

#[derive(Clone,Copy,Debug)]
pub struct Dims {
    pub width: u16,
    pub height: u16
}

#[derive(Clone,Copy,Eq,PartialEq,Hash)]
pub struct Vec2d<T> {
    pub x: T,
    pub y: T
}

impl<T> Vec2d<T> {
    pub fn new(x: T, y: T) -> Self {
        Vec2d{ x: x, y: y }
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
    pub fn vec2d(&self) -> Vec2d<i32> {
        match *self {
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
    type Err = String;
    fn try_from(c: char) -> Result<Direction,String> {
        match c {
            conf::KEY_UP         => Ok(Direction::Up),
            conf::KEY_DOWN       => Ok(Direction::Down),
            conf::KEY_LEFT       => Ok(Direction::Left),
            conf::KEY_RIGHT      => Ok(Direction::Right),
            conf::KEY_UP_LEFT    => Ok(Direction::UpLeft),
            conf::KEY_UP_RIGHT   => Ok(Direction::UpRight),
            conf::KEY_DOWN_LEFT  => Ok(Direction::DownLeft),
            conf::KEY_DOWN_RIGHT => Ok(Direction::DownRight),
            _                    => Err(format!("{} doesn't indicate a direction", c))
        }
    }
}

pub enum Wrap {
    Wrapping,
    NonWrapping
}

impl Wrap {
    fn wrap_usize(&self, idx: usize, width: usize) -> usize {
        match *self {
            Wrap::Wrapping => idx % width,
            Wrap::NonWrapping => idx
        }
    }

    fn wrap_u16(&self, idx: u16, width: u16) -> u16 {
        match *self {
            Wrap::Wrapping => idx % width,
            Wrap::NonWrapping => idx
        }
    }


}

pub struct Wrap2d {
    pub horiz: Wrap,
    pub vert: Wrap
}

impl Wrap2d {
    fn wrap_loc(&self, loc: Location, dims: Dims) -> Location {
        Location {
            x: self.horiz.wrap_u16(loc.x, dims.width),
            y: self.vert.wrap_u16(loc.y, dims.height)
        }
    }
}

pub static WRAP_BOTH: Wrap2d = Wrap2d{
    horiz: Wrap::Wrapping,
    vert: Wrap::Wrapping
};

pub static WRAP_HORIZ: Wrap2d = Wrap2d {
    horiz: Wrap::Wrapping,
    vert: Wrap::NonWrapping
};

pub static WRAP_VERT: Wrap2d = Wrap2d {
    horiz: Wrap::NonWrapping,
    vert: Wrap::Wrapping
};

pub static WRAP_NEITHER: Wrap2d = Wrap2d {
    horiz: Wrap::NonWrapping,
    vert: Wrap::NonWrapping
};

pub type Location = Vec2d<u16>;
impl Location {
    fn dist_u16(x: u16, y: u16) -> u16 {
        if x > y {
            return x - y;
        }
        y - x
    }

    fn dist_u16_wrapping(x: u16, y: u16, width: u16) -> u16 {
        match x.cmp(&y) {
            Ordering::Equal => 0,
            Ordering::Greater => Location::dist_u16_wrapping(y, x, width),
            Ordering::Less => {

                let dist_via_middle = y - x;
                let dist_via_wrap = x + width - y;

                min(dist_via_middle, dist_via_wrap)
            }
        }
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[test]
fn test_dist_u16_wrapping() {
    let width = 5;
    assert_eq!(Location::dist_u16_wrapping(1, 3, width), 2);
    assert_eq!(Location::dist_u16_wrapping(3, 1, width), 2);
    assert_eq!(Location::dist_u16_wrapping(0, 4, width), 1);
    assert_eq!(Location::dist_u16_wrapping(4, 0, width), 1);
    assert_eq!(Location::dist_u16_wrapping(0, 0, width), 0);
}
