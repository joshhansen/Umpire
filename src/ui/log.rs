use std::collections::VecDeque;
use std::io::Write;

use termion::{
    color::{Bg,Fg},
    style::Underline
};

use crate::{
    color::{BLACK,WHITE},
    game::Game,
    log::{Message,MessageSource},
    ui::{
        Component,
        Draw,
        style::StrongReset
    },
    util::{Rect,grapheme_len,grapheme_substr}
};

pub struct LogArea {
    rect: Rect,
    messages: VecDeque<Message>,
    empty_message: Message,
}

impl LogArea {
    pub fn new(rect: Rect) -> Self {
        LogArea {
            rect,
            messages: VecDeque::new(),
            empty_message: Message::from(String::from("")),
        }
    }

    fn max_messages(&self) -> u16 {
        self.rect.height - 1
    }

    pub fn log(&mut self, message: Message) {
        if message.source == Some(MessageSource::Game) {
            return;
        }
        self.messages.push_back(message);
        if self.messages.len() > self.max_messages() as usize {
            self.messages.pop_front();
        }
    }

    #[allow(dead_code)]
    pub fn log_message(&mut self, message: String) {
        self.log(Message::from(message))
    }

    pub fn replace(&mut self, message: Message) {
        // if let Some(item) = self.messages.back_mut() {
        //     *item = message;
        //     return;// TODO maybe when non-lexical lifetimes arrive we can get rid of this awkward return construct
        // }
        self.log(message);
    }

    #[allow(dead_code)]
    pub fn replace_message(&mut self, message: String) {
        self.replace(Message::from(message));
    }

    pub fn draw_lite<W:Write>(&self, stdout: &mut W) {
        write!(*stdout,
            "{}{}Message Log{}",
            self.goto(0, 0),
            Underline,
            StrongReset
        ).unwrap();

        for i in 0..self.rect.height {
            let message: &Message = self.messages.get(i as usize).unwrap_or(&self.empty_message);

            let mut text = grapheme_substr( &message.text, self.rect.width as usize);
            let num_spaces = self.rect.width as usize - grapheme_len(&text);
            for _ in 0..num_spaces {
                text.push(' ');
            }

            let mark = message.mark.unwrap_or(' ');
            let fg_color = message.fg_color.unwrap_or(WHITE);
            let bg_color = message.bg_color.unwrap_or(BLACK);

            write!(*stdout, "{}┃{}{}{}{}", self.goto(0, i as u16+1), mark, Fg(fg_color), Bg(bg_color), text).unwrap();
        }

        stdout.flush().unwrap();
    }
}

impl Draw for LogArea {
    fn draw<W:Write>(&mut self, _game: &Game, stdout: &mut W) {
        self.draw_lite(stdout);
    }
}

impl Component for LogArea {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}
