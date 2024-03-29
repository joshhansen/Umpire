use std::fmt;

use serde::{Deserialize, Serialize};

use crate::colors::{Colorized, Colors};

#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
pub enum Terrain {
    Water,
    Land,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

impl Colorized for Terrain {
    fn color(&self) -> Option<Colors> {
        Some(match *self {
            Terrain::Water => Colors::Ocean,
            Terrain::Land => Colors::Land,
        })
    }
}

impl fmt::Display for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Terrain::Water => "Water",
                Terrain::Land => "Land",
            }
        )
    }
}

impl fmt::Debug for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
