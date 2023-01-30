use std::fmt;

use serde::{Deserialize, Serialize};

use crate::colors::{Colorized, Colors};

use super::PlayerNum;

#[derive(Copy, Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Alignment {
    Neutral,
    Belligerent { player: PlayerNum }, // active neutral, chaotic, etc.
}

impl Alignment {
    pub fn is_friendly_to(self, other: Alignment) -> bool {
        self == other
    }

    pub fn is_friendly_to_player(self, player: PlayerNum) -> bool {
        self == Alignment::Belligerent { player }
    }

    pub fn is_enemy_of(self, other: Alignment) -> bool {
        !self.is_friendly_to(other)
    }

    pub fn is_enemy_of_player(self, player: PlayerNum) -> bool {
        !self.is_friendly_to_player(player)
    }

    pub fn is_neutral(self) -> bool {
        self == Alignment::Neutral
    }

    pub fn is_belligerent(self) -> bool {
        if let Alignment::Belligerent { .. } = self {
            true
        } else {
            false
        }
    }
}

impl Colorized for Alignment {
    fn color(&self) -> Option<Colors> {
        Some(match self {
            Alignment::Neutral => Colors::Neutral,
            Alignment::Belligerent { player } => Colors::Player(*player),
        })
    }
}

impl fmt::Display for Alignment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Alignment::Neutral => write!(f, "Neutral"),
            Alignment::Belligerent { player } => write!(f, "Player {}", player),
        }
    }
}

pub trait Aligned: AlignedMaybe {
    fn alignment(&self) -> Alignment;

    fn is_friendly_to<A: Aligned>(&self, other: &A) -> bool {
        self.alignment().is_friendly_to(other.alignment())
    }

    fn is_friendly_to_player(&self, player: PlayerNum) -> bool {
        self.alignment().is_friendly_to_player(player)
    }

    fn is_enemy_of<A: Aligned>(&self, other: &A) -> bool {
        self.alignment().is_enemy_of(other.alignment())
    }

    fn is_enemy_of_player(&self, player: PlayerNum) -> bool {
        self.alignment().is_enemy_of_player(player)
    }

    fn is_neutral(&self) -> bool {
        self.alignment().is_neutral()
    }

    fn is_belligerent(&self) -> bool {
        self.alignment().is_belligerent()
    }
}

pub trait AlignedMaybe {
    fn alignment_maybe(&self) -> Option<Alignment>;

    fn belongs_to_player(&self, player: PlayerNum) -> bool {
        if let Some(alignment) = self.alignment_maybe() {
            if let Alignment::Belligerent { player: player_ } = alignment {
                player == player_
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl<T: Aligned> AlignedMaybe for T {
    fn alignment_maybe(&self) -> Option<Alignment> {
        Some(self.alignment())
    }
}
