//! An abstract logging interface.
//!
//! This is one channel over which the game engine and various UIs can communicate.

use crate::color::Colors;

#[derive(PartialEq)]
pub enum MessageSource {
    // Main,
    Game,
    UI,
    Mode
}

/// A loggable message, along with some presentation details such as foreground and background
/// colors, and a sigil or mark.
pub struct Message {
    pub text: String,
    pub mark: Option<char>,
    pub fg_color: Option<Colors>,
    pub bg_color: Option<Colors>,
    pub source: Option<MessageSource>
}

impl From<String> for Message {
    fn from(s: String) -> Self {
        Message {
            text: s,
            mark: None,
            fg_color: None,
            bg_color: None,
            source: None
        }
    }
}

impl From<&str> for Message {
    fn from(s: &str) -> Self {
        Self::from(String::from(s))
    }
}

/// A valid target to which messages can be logged.
pub trait LogTarget {
    fn log_message<T>(&mut self, message: T) where Message:From<T>;
    fn replace_message<T>(&mut self, message: T) where Message:From<T>;
}
