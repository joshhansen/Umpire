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

#[derive(Clone,Copy)]
pub struct Vec2d<T> {
    pub x: T,
    pub y: T
}
