use crate::game::PlayerNum;

#[derive(Copy, Clone, Debug)]
pub enum Colors {
    /// The background behind everything else
    Background,

    /// Dry land
    Land,

    /// The ocean
    Ocean,

    /// The neutral "player"'s color
    Neutral,

    /// A player's color
    Player(PlayerNum),

    /// The default text color
    Text,

    /// A message that needs to be extra noticeable
    Notice,

    /// The cursor
    Cursor,

    /// Messages or effects about combat
    Combat,

    /// Scroll percentage indicators
    ScrollMarks,
}

pub trait Colorized {
    fn color(&self) -> Option<Colors>;
}
