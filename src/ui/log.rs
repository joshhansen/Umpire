use std::{
    collections::VecDeque,
    io::{Stdout,Write},
};

use crossterm::{
    Attribute,
    Color,
    Output,
    SetAttr,
    SetBg,
    SetFg,
    queue,
};

use crate::{
    color::{Colors,Palette},
    game::Game,
    log::{Message},
    ui::{
        Component,
        Draw,
    },
    util::{Rect,grapheme_len,grapheme_substr}
};

pub(in crate::ui) struct LogArea {
    rect: Rect,
    messages: VecDeque<Message>,
    empty_message: Message,
    
}

impl LogArea {
    pub(in crate::ui) fn new(rect: Rect) -> Self {
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
        // if message.source == Some(MessageSource::Game) {
        //     return;
        // }
        self.messages.push_back(message);
        if self.messages.len() > self.max_messages() as usize {
            self.messages.pop_front();
        }
    }

    pub fn replace(&mut self, message: Message) {
        // if let Some(item) = self.messages.back_mut() {
        //     *item = message;
        //     return;// TODO maybe when non-lexical lifetimes arrive we can get rid of this awkward return construct
        // }
        // self.log(message);
        if let Some(item) = self.messages.back_mut() {
            *item = message;
        } else {
            self.log(message);
        }
    }

    pub fn draw_lite(&self, stdout: &mut Stdout, palette: &Palette) {
        // write!(*stdout,
        //     "{}{}Message Log{}",
        //     self.goto(0, 0),
        //     Underline,
        //     StrongReset::new(palette),
        // ).unwrap();

        queue!(*stdout,
            self.goto(0, 0),
            SetAttr(Attribute::Underlined),
            Output(String::from("Message Log")),
            SetAttr(Attribute::Reset),
            SetBg(palette.get_single(Colors::Background))
        ).unwrap();

        for i in 0..self.rect.height {
            let message: &Message = self.messages.get(i as usize).unwrap_or(&self.empty_message);

            let mut text = grapheme_substr( &message.text, self.rect.width as usize);
            let num_spaces = self.rect.width as usize - grapheme_len(&text);
            for _ in 0..num_spaces {
                text.push(' ');
            }

            let mark = message.mark.unwrap_or(' ');
            let fg_color: Color = message.fg_color.map_or_else(
                || palette.get_single(Colors::Text),
                |fg_color| palette.get_single(fg_color)
            );

            let bg_color: Color = message.bg_color.map_or_else(
                || palette.get_single(Colors::Background),
                |bg_color| palette.get_single(bg_color)
            );

            // write!(*stdout, "{}┃{}{}{}{}", self.goto(0, i as u16+1), mark, Fg(fg_color), Bg(bg_color), text).unwrap();
            queue!(*stdout,
                self.goto(0, i as u16+1),
                Output(format!("|{}", mark)),
                SetFg(fg_color),
                SetBg(bg_color),
                Output(text)
            ).unwrap();
        }

        stdout.flush().unwrap();
    }
}

impl Draw for LogArea {
    fn draw(&mut self, _game: &Game, stdout: &mut Stdout, palette: &Palette) {
        self.draw_lite(stdout, palette);
    }
}

impl Component for LogArea {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}
