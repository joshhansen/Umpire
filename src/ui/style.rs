// use std::fmt;

// use termion::color::{Bg,Color};
// use termion::style::Reset;

// use crate::color::{Colors,Palette};

// #[deprecated]
// #[derive(Clone,Copy)]
// pub struct StrongReset {
//     background: Color
// }

// impl <C:Color+Copy> StrongReset {
//     pub fn new(palette: &Palette) -> Self {
//         Self {
//             background: palette.get_single(Colors::Background)
//         }
//     }
// }

// impl fmt::Display for StrongReset {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "{}", Reset)
//         .and(write!(f, "{}", Bg(self.background)))
//     }
// }