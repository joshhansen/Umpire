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

use std::convert::TryFrom;
use std::fmt;
use std::ops::Add;
use std::thread::sleep;
use std::time::Duration;

use conf;
use map::dijkstra::RELATIVE_NEIGHBORS;

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

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Dims {
    pub width: u16,
    pub height: u16
}

impl fmt::Display for Dims {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.width.fmt(f)
        .and(write!(f, "x"))
        .and(self.height.fmt(f))
    }
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

pub struct Wrap2d {
    pub horiz: Wrap,
    pub vert: Wrap
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

///
/// Add `inc` to `loc` respecting the specified wrapping rules in a space defined by `dims`
/// If the result is out of bounds, return None
///
pub fn wrapped_add(loc: &Location, inc: &Vec2d<i16>, dims: &Dims, wrapping: &Wrap2d) -> Option<Location> {
    let mut new_x: i32 = loc.x as i32 + inc.x as i32;
    if let Wrap::Wrapping = wrapping.horiz {
        new_x = if new_x < 0 { dims.width as i32 + new_x } else { new_x % dims.width as i32 };
    } else if new_x >= dims.width as i32 {
        return None;
    }

    let mut new_y: i32 = loc.y as i32 + inc.y as i32;
    if let Wrap::Wrapping = wrapping.vert {
        new_y = if new_y < 0 { dims.height as i32 + new_y } else { new_y % dims.height as i32 };
    } else if new_y >= dims.height as i32 {
        return None;
    }

    Some(Location {
        x: new_x as u16,
        y: new_y as u16
    })
}

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
        assert_eq!( wrapped_add(&loc, &rel_neighb, &dims, &WRAP_BOTH),    results_both   [i] );
        assert_eq!( wrapped_add(&loc, &rel_neighb, &dims, &WRAP_HORIZ),   results_horiz  [i] );
        assert_eq!( wrapped_add(&loc, &rel_neighb, &dims, &WRAP_VERT),    results_vert   [i] );
        assert_eq!( wrapped_add(&loc, &rel_neighb, &dims, &WRAP_NEITHER), results_neither[i] );
    }
}

pub type Location = Vec2d<u16>;

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

pub fn sleep_millis(millis: u64) {
    sleep(Duration::from_millis(millis));
}
