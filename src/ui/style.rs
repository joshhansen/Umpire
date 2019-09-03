use std::fmt;

use termion::color::Bg;
use termion::style::Reset;

use color::BLACK;

#[derive(Clone,Copy)]
pub struct StrongReset;

impl fmt::Display for StrongReset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", Reset)
        .and(write!(f, "{}", Bg(BLACK)))
    }
}
