//!
//! Utility functions and structs
//!

extern crate termion;

use std::io::{Write,stdout};

use termion::color::{Fg, AnsiValue};
use termion::raw::IntoRawMode;

/// 0-indexed variant of Goto
pub fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

pub fn draw_scroll_mark(x: u16, y: u16, sym: char) {
    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();
    write!(stdout, "{}{}{}{}", termion::style::Reset, goto(x,y), Fg(AnsiValue(11)), sym);
}

pub fn erase(x: u16, y: u16) {
    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();
    write!(stdout, "{}{} ", termion::style::Reset, goto(x,y));
}

// pub fn safe_minus_one(x:u16) -> u16 {
//     if x > 0 { x - 1}
//     else { 0 }
// }
//
// pub fn safe_plus_one(x:u16, max:u16) -> u16 {
//     if x < max { x + 1 }
//     else { max }
// }


#[derive(Copy,Clone)]
pub struct Dims {
    pub width: u16,
    pub height: u16
}

#[derive(Copy,Clone)]
pub struct Vec2d<T> {
    pub x: T,
    pub y: T
}
