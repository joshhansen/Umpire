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

#[derive(Clone,Copy)]
pub struct Rect {
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16
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

#[test]
fn test_distance() {
    let a = Location{x: 0, y: 0};
    let b = Location{x:2, y: 2};
    let dist = a.distance(&b);
    assert_eq!(dist, 4);
}
