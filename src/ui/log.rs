use std::{
    collections::VecDeque,
    io::{Stdout,Write},
};

use crossterm::{
    queue,
    style::{
        Attribute,
        Color,
        Print,
        SetAttribute,
        SetBackgroundColor,
        SetForegroundColor,
    },
};

use crate::{
    color::{Colors,Palette},
    game::Game,
    log::{
        LogTarget,
        Message,
    },
    ui::{
        Component,
        Draw,
    },
    util::{Rect,grapheme_len,grapheme_substr}
};

//TODO Use a RectBuffer to improve draw performance
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

    fn draw_log_line_no_flush(&self, stdout: &mut Stdout, palette: &Palette, i: usize) {
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
            SetForegroundColor(fg_color),
            SetBackgroundColor(bg_color),
            Print(format!("|{}", mark)),
            Print(text)
        ).unwrap();
    }

    pub fn pop_message(&mut self) -> Option<Message> {
        self.messages.pop_back()
    }
}

impl LogTarget for LogArea {
    fn log_message<M>(&mut self, message: M) where Message:From<M> {
        // if message.source == Some(MessageSource::Game) {
        //     return;
        // }
        self.messages.push_back(message.into());
        if self.messages.len() > self.max_messages() as usize {
            self.messages.pop_front();
        }
    }

    fn replace_message<M>(&mut self, message: M) where Message:From<M> {
        // if let Some(item) = self.messages.back_mut() {
        //     *item = message;
        //     return;// TODO maybe when non-lexical lifetimes arrive we can get rid of this awkward return construct
        // }
        // self.log(message);
        if let Some(item) = self.messages.back_mut() {
            *item = message.into();
        } else {
            self.log_message(message);
        }
    }
}

impl Draw for LogArea {
    fn draw_no_flush(&mut self, _game: &Game, stdout: &mut Stdout, palette: &Palette) {
        // write!(*stdout,
        //     "{}{}Message Log{}",
        //     self.goto(0, 0),
        //     Underline,
        //     StrongReset::new(palette),
        // ).unwrap();

        queue!(*stdout,
            self.goto(0, 0),
            SetAttribute(Attribute::Underlined),
            Print(String::from("Message Log")),
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(palette.get_single(Colors::Background))
        ).unwrap();

        for i in 0..self.rect.height {
            self.draw_log_line_no_flush(stdout, palette, i as usize);
        }
    }
}

impl Component for LogArea {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect { self.rect }

    fn is_done(&self) -> bool { false }
}
