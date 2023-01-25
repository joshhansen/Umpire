use std::{
    collections::HashSet,
    io::{Stdout, Write},
};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{style, PrintStyledContent},
};

use crate::{color::Palette, game::player::PlayerTurnControl, ui::Draw, util::Rect};

/// A buffer to help with smooth drawing of rectangular regions
///
/// Instead of having different bits of code clear and write over the same region, leading to a flickering effect,
/// we make one piece of code responsible for drawing on one region. Clients update the buffer contents and tell
/// the buffer to draw, but the buffer works out the most efficient way to do the drawing without flickering.
///
/// TODO: Implement Component?
pub(in crate::ui) struct RectBuffer {
    rect: Rect,
    rows: Vec<Option<String>>,
    dirty_rows: HashSet<usize>,
    blank_row: String,
}
impl RectBuffer {
    pub fn new(rect: Rect) -> Self {
        let blank_row: String = (0..rect.width).map(|_| " ").collect();
        Self {
            rect,
            rows: (0..rect.height).map(|_| None).collect(),
            dirty_rows: HashSet::new(),
            blank_row,
        }
    }

    pub fn set_row(&mut self, row_idx: usize, row: String) {
        self.set(row_idx, Some(row));
    }

    pub fn clear(&mut self) {
        for row_idx in 0..self.rect.height {
            self.clear_row(row_idx as usize);
        }
    }

    pub fn clear_row(&mut self, row_idx: usize) {
        self.set(row_idx, None);
    }

    // /// Draw an individual row
    // ///
    // /// The row will then be marked clean
    // pub fn draw_row(&mut self, row_idx: usize, stdout: &mut Stdout) {
    //     if self.dirty_rows.contains(&row_idx) {
    //         self._draw_row(row_idx, stdout);
    //         self.dirty_rows.remove(&row_idx);
    //     }
    // }

    fn _draw_row(&self, row_idx: usize, stdout: &mut Stdout) {
        queue!(
            stdout,
            MoveTo(self.rect.left, self.rect.top + row_idx as u16),
            PrintStyledContent(style(if let Some(ref row) = self.rows[row_idx] {
                row.clone()
            } else {
                self.blank_row.clone()
            }))
        )
        .unwrap();
    }

    fn set(&mut self, row_idx: usize, maybe_row: Option<String>) {
        self.rows[row_idx] = maybe_row.map(|mut row| {
            while row.len() < self.blank_row.len() {
                row.push(' ');
            }
            row
        });
        // self.dirty = true;
        // self.rows_dirty[row_idx] = true;
        self.dirty_rows.insert(row_idx);
    }
}

impl Draw for RectBuffer {
    fn draw_no_flush(
        &mut self,
        _game: &PlayerTurnControl,
        stdout: &mut Stdout,
        _palette: &Palette,
    ) {
        for dirty_row_idx in &self.dirty_rows {
            self._draw_row(*dirty_row_idx, stdout);
        }
        self.dirty_rows.clear();
        // if self.dirty {
        //     for (row_idx, row) in self.rows.iter().enumerate() {
        //         if self.rows_dirty[row_idx] {
        //             queue!(stdout, Goto(self.rect.left, self.rect.top + row_idx as u16), Output(
        //                 if let Some(row) = row {
        //                     row.clone()
        //                 } else {
        //                     self.blank_row.clone()
        //                 }
        //             )).unwrap();
        //             self.rows_dirty[row_idx] = false;
        //         }
        //     }
        //     stdout.flush().unwrap();

        //     self.dirty = false;
        // }
    }
}
