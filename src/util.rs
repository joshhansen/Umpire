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

use std::fmt;

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

#[derive(Clone,Copy)]
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


pub type Location = Vec2d<u16>;
impl Location {
    fn dist_u16(x: u16, y: u16) -> u16 {
        if x > y {
            return x - y;
        }
        y - x
    }

    /// Manhattan distance
    /// The number of moves it would take a unit to move from this location to the other location
    pub fn distance(&self, other: &Location) -> u16 {

        let x_dist = Location::dist_u16(self.x, other.x);
        let y_dist = Location::dist_u16(self.y, other.y);
        x_dist + y_dist
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[test]
fn test_distance() {
    let a = Location{x: 0, y: 0};
    let b = Location{x:2, y: 2};
    let dist = a.distance(&b);
    assert_eq!(dist, 4);
}
