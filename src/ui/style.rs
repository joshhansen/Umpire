use std::fmt;

use termion::color::{Bg,Color};
use termion::style::Reset;

use crate::color::{Colors,Palette};

#[derive(Clone,Copy)]
pub struct StrongReset<C:Copy> {
    background: C
}

impl <C:Color+Copy> StrongReset<C> {
    pub fn new(palette: &Palette<C>) -> Self {
        Self {
            background: palette.get_single(Colors::Background)
        }
    }
}

impl <C:Color+Copy> fmt::Display for StrongReset<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", Reset)
        .and(write!(f, "{}", Bg(self.background)))
    }
}
