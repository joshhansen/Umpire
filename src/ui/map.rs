use std::io::Write;
use std::rc::Rc;

use termion::color::{Color,Fg, Bg, White, Black};
use termion::cursor::Hide;
use termion::style::{Blink,Bold,Invert,Italic,Underline};

use color::{PairColorized,Palette,PaletteT,BLACK,WHITE};
use game::{AlignedMaybe,Game};
use game::obs::Obs;
use map::{LocationGrid,Tile};
use ui::{Component,Draw};
use ui::scroll::{ScrollableComponent};
use ui::style::StrongReset;
use unit::orders::Orders;
use util::{Dims,Location,Rect,Vec2d};

fn nonnegative_mod(x: i32, max: u16) -> u16 {
    let mut result = x;
    let max = i32::from(max);

    while result < 0 {
        result += max;
    }

    (result % max) as u16
}

pub fn viewport_to_map_coords(map_dims: Dims, viewport_loc: Location, viewport_offset: Vec2d<u16>) -> Location {
    Location {
        x: (viewport_loc.x + viewport_offset.x) % map_dims.width, // mod implements wrapping,
        y: (viewport_loc.y + viewport_offset.y) % map_dims.height // mod implements wrapping
    }
}

/*

map_coord: 0
viewport_offset: 0
viewport_width: 4
map_width: 10

..>..<....


*/
fn map_to_viewport_coord(map_coord: u16, viewport_offset: u16, viewport_width: u16, map_dimension_width: u16) -> Result<Option<u16>,String> {
    if viewport_width > map_dimension_width {
        return Err(String::from("Viewport width is larger than map width"));
    }

    if map_coord >= map_dimension_width {
        return Err(format!("Map coordinate {} is larger than map dimension size {}", map_coord, map_dimension_width));
    }

    let unoffset_coord: i32 = i32::from(map_coord) - i32::from(viewport_offset);
    let wrapped_coord = if unoffset_coord < 0 {
        i32::from(map_dimension_width) + unoffset_coord
    } else {
        unoffset_coord
    } as u16;

    Ok(if wrapped_coord < viewport_width {
        Some(wrapped_coord)
    } else {
        None
    })
}

/// Returns None if the map location is not currently in the viewport
/// Otherwise, it returns the coordinates at which that location is plotted
/*
In one dimension, there are two cases. First, the viewport covers a contiguous region of the
dimension:

..xxx.....

Second, the viewport covers two disjoint regions of the dimension:

xx......xx

In two dimensions, these two cases combine into four cases. First, both dimensions cover
contiguous regions:
..........
..xxxx....
..x..x....
..xxxx....
..........

Second, the viewport covers two disjoint regions, split horizontally:
..........
xx......xx
.x......x.
xx......xx
..........

Third, the viewport covers two disjoint regions, split vertically:
..xxxx....
..........
..........
..xxxx....
..x..x....

Fourth, the viewport covers four disjoint regions, split horizontally and vertically:
xx......xx
..........
..........
xx......xx
.x......x.
*/
pub fn map_to_viewport_coords(map_loc: Location, viewport_offset: Vec2d<u16>, viewport_dims: Dims, map_dims: Dims) -> Option<Location> {
    if let Some(viewport_x) = map_to_viewport_coord(map_loc.x, viewport_offset.x, viewport_dims.width, map_dims.width).unwrap() {
        if let Some(viewport_y) = map_to_viewport_coord(map_loc.y, viewport_offset.y, viewport_dims.height, map_dims.height).unwrap() {
            return Some(Location {
                x: viewport_x,
                y: viewport_y
            });
        }
    }
    None
}

/// The map widget
pub struct Map<C:Color+Copy> {
    rect: Rect,
    map_dims: Dims,
    old_viewport_offset: Vec2d<u16>,
    viewport_offset: Vec2d<u16>,
    displayed_tiles: LocationGrid<Option<Tile>>,
    palette: Rc<Palette<C>>,
}
impl <C:Color+Copy> Map<C> {
    pub fn new(rect: Rect, map_dims: Dims, palette: Rc<Palette<C>>) -> Self {
        let displayed_tiles = LocationGrid::new(rect.dims(), |_loc| None);
        Map{
            rect,
            map_dims,
            old_viewport_offset: Vec2d::new(0, 0),
            viewport_offset: Vec2d::new(rect.width / 2, rect.height / 2),
            displayed_tiles,
            palette
        }
    }

    pub fn shift_viewport(&mut self, shift: Vec2d<i32>) {
        let mut new_x_offset:i32 = ( i32::from(self.viewport_offset.x) ) + shift.x;
        let mut new_y_offset:i32 = ( i32::from(self.viewport_offset.y) ) + shift.y;

        while new_x_offset < 0 {
            new_x_offset += i32::from(self.map_dims.width);
        }
        while new_y_offset < 0 {
            new_y_offset += i32::from(self.map_dims.height);
        }

        let new_viewport_offset = Vec2d{
            x: (new_x_offset as u16) % self.map_dims.width,
            y: (new_y_offset as u16) % self.map_dims.height
        };

        self.set_viewport_offset(new_viewport_offset);
    }

    fn set_viewport_offset(&mut self, new_viewport_offset: Vec2d<u16>) {
        self.old_viewport_offset = self.viewport_offset;
        self.viewport_offset = new_viewport_offset;
    }

    pub fn map_to_viewport_coords(&self, map_loc: Location, viewport_dims: Dims) -> Option<Location> {
        map_to_viewport_coords(map_loc, self.viewport_offset, viewport_dims, self.map_dims)
    }

    pub fn center_viewport(&mut self, map_location: Location) {
        let new_viewport_offset = Vec2d {
            x: nonnegative_mod(
                i32::from(map_location.x) - (i32::from(self.rect.width) / 2),
                self.map_dims.width
            ),
            y: nonnegative_mod(
                i32::from(map_location.y) - (i32::from(self.rect.height) / 2),
                self.map_dims.height
            )
        };

        self.set_viewport_offset(new_viewport_offset);
    }

    pub fn draw_tile<W:Write>(&mut self,
            game: &Game,
            stdout: &mut W,
            viewport_loc: Location,
            highlight: bool,// Highlighting as for a cursor
            unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders
            symbol: Option<&'static str>) {

        let tile_loc = viewport_to_map_coords(game.map_dims(), viewport_loc, self.viewport_offset);

        if tile_loc.y == game.map_dims().height - 1 {
            write!(stdout, "{}", Underline).unwrap();
        }

        write!(stdout, "{}", self.goto(viewport_loc.x, viewport_loc.y)).unwrap();
        

        let currently_observed = game.current_player_obs(tile_loc) == Some(&Obs::Current);

        if let Some(tile) = game.current_player_tile(tile_loc) {
            if highlight {
                write!(stdout, "{}", Invert).unwrap();
            }

            if unit_active {
                write!(stdout, "{}{}", Blink, Bold).unwrap();
            }

            if let Some(fg_color) = tile.color_pair(&self.palette) {
                write!(stdout, "{}", Fg(fg_color.get(currently_observed))).unwrap();
            }

            if let Some(ref unit) = tile.unit {
                if let Some(orders) = unit.orders() {
                    if *orders == Orders::Sentry {
                        write!(stdout, "{}", Italic).unwrap();
                    }
                }
            }

            write!(stdout, "{}{}",
                Bg(tile.terrain.color_pair(&self.palette).unwrap().get(currently_observed)),
                symbol.unwrap_or(tile.sym())
            ).unwrap();

            self.displayed_tiles[viewport_loc] = Some(tile.clone());
        } else {
            if highlight {
                write!(stdout, "{}{}", Bg(White), Bg(WHITE)).unwrap();// Use ansi white AND rgb white. Terminals supporting rgb will get a brighter white
            } else {
                write!(stdout, "{}{}", Bg(Black), Bg(BLACK)).unwrap();
            }
            write!(stdout, " ").unwrap();
            self.displayed_tiles[viewport_loc] = None;
        }

        write!(stdout, "{}", StrongReset).unwrap();
        stdout.flush().unwrap();
    }

    pub fn current_player_tile<'a>(&self, game: &'a Game, viewport_loc: Location) -> Option<&'a Tile> {
        let tile_loc = viewport_to_map_coords(game.map_dims(), viewport_loc, self.viewport_offset);
        game.current_player_tile(tile_loc)
    }
}

impl <C:Color+Copy> ScrollableComponent for Map<C> {
    fn scroll_relative(&mut self, offset: Vec2d<i32>) {
        self.shift_viewport(offset);
    }

    fn offset(&self) -> Vec2d<u16> { self.viewport_offset }
}

impl <C:Color+Copy> Component for Map<C> {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn is_done(&self) -> bool { false }
}

impl <C:Color+Copy> Draw for Map<C> {
    fn draw<W:Write>(&mut self, game: &Game, stdout: &mut W) {
        let mut viewport_loc = Location{x: 0, y: 0};
        for viewport_x in 0..self.rect.width {
            viewport_loc.x = viewport_x;
            for viewport_y in 0..self.rect.height {
                viewport_loc.y = viewport_y;

                let should_draw_tile = {
                    let old_map_loc = viewport_to_map_coords(game.map_dims(), viewport_loc, self.old_viewport_offset);
                    let new_map_loc = viewport_to_map_coords(game.map_dims(), viewport_loc, self.viewport_offset);

                    let old_tile = self.displayed_tiles[viewport_loc].as_ref();
                    let new_tile = &game.current_player_tile(new_map_loc);

                    (old_tile.is_some() && new_tile.is_none()) ||
                    (old_tile.is_none() && new_tile.is_some()) ||
                    (old_tile.is_some() && new_tile.is_some() && {
                        let old = old_tile.unwrap();
                        let new = new_tile.unwrap();
                        let redraw_for_mismatch = !(
                            old.terrain==new.terrain &&
                            old.sym() == new.sym() &&
                            old.alignment_maybe() == new.alignment_maybe()
                        );
                        redraw_for_mismatch
                    }) || {
                        let redraw_for_border =
                        old_map_loc.y != new_map_loc.y && (
                            old_map_loc.y == game.map_dims().height - 1 ||
                            new_map_loc.y == game.map_dims().height - 1
                        );
                        redraw_for_border
                    }
                };

                if should_draw_tile {
                    self.draw_tile(game, stdout, viewport_loc, false, false, None);
                }

            }
        }

        write!(stdout, "{}{}", StrongReset, Hide).unwrap();
        stdout.flush().unwrap();
    }
}


#[cfg(test)]
mod test {
    use ui::map::map_to_viewport_coord;

    #[test]
    fn test_map_to_viewport_coord() {
        assert_eq!(map_to_viewport_coord(0, 0, 10, 100), Ok(Some(0)));
        assert_eq!(map_to_viewport_coord(5, 0, 10, 100), Ok(Some(5)));
        assert_eq!(map_to_viewport_coord(9, 0, 10, 100), Ok(Some(9)));
        assert_eq!(map_to_viewport_coord(10, 0, 10, 100), Ok(None));

        assert_eq!(map_to_viewport_coord(0, 5, 10, 100), Ok(None));
        assert_eq!(map_to_viewport_coord(4, 5, 10, 100), Ok(None));
        assert_eq!(map_to_viewport_coord(5, 5, 10, 100), Ok(Some(0)));
        assert_eq!(map_to_viewport_coord(10, 5, 10, 100), Ok(Some(5)));
        assert_eq!(map_to_viewport_coord(14, 5, 10, 100), Ok(Some(9)));
        assert_eq!(map_to_viewport_coord(15, 5, 10, 100), Ok(None));

        assert_eq!(map_to_viewport_coord(0, 90, 10, 100), Ok(None));
        assert_eq!(map_to_viewport_coord(89, 90, 10, 100), Ok(None));
        assert_eq!(map_to_viewport_coord(90, 90, 10, 100), Ok(Some(0)));
        assert_eq!(map_to_viewport_coord(95, 90, 10, 100), Ok(Some(5)));
        assert_eq!(map_to_viewport_coord(99, 90, 10, 100), Ok(Some(9)));
        assert_eq!(map_to_viewport_coord(100, 90, 10, 100), Err(String::from("Map coordinate 100 is larger than map dimension size 100")));

        assert_eq!(map_to_viewport_coord(94, 95, 10, 100), Ok(None));
        assert_eq!(map_to_viewport_coord(95, 95, 10, 100), Ok(Some(0)));
        assert_eq!(map_to_viewport_coord(100, 95, 10, 100), Err(String::from("Map coordinate 100 is larger than map dimension size 100")));
        assert_eq!(map_to_viewport_coord(0, 95, 10, 100), Ok(Some(5)));
        assert_eq!(map_to_viewport_coord(4, 95, 10, 100), Ok(Some(9)));
        assert_eq!(map_to_viewport_coord(5, 95, 10, 100), Ok(None));
    }
}
