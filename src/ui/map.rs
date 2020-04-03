use std::{
    io::{Stdout,Write},
    rc::Rc,
};

use crossterm::{
    cursor::Hide,
    QueueableCommand,
    style::{
        Attribute,
        Print,
        SetAttribute,
        SetBackgroundColor,
        SetForegroundColor,
    },
};

use crate::{
    color::{Colors,Colorized,Palette},
    game::{
        AlignedMaybe,
        city::City,
        map::{LocationGrid,Tile},
        obs::Obs,
        player::PlayerTurnControl,
        unit::{
            orders::Orders,
            Unit,
        },
    },
    ui::{
        Component,
        Draw,
        scroll::ScrollableComponent,
        sym::Sym,
    },
    util::{
        Dims,
        Location,
        Rect,
        Vec2d,
    }
};

fn nonnegative_mod(x: i32, max: u16) -> u16 {
    let mut result = x;
    let max = i32::from(max);

    while result < 0 {
        result += max;
    }

    (result % max) as u16
}

// #[deprecated]
// fn viewport_to_map_coords(map_dims: Dims, viewport_loc: Location, viewport_offset: Vec2d<u16>) -> Location {
//     Location {
//         x: (viewport_loc.x + viewport_offset.x) % map_dims.width, // mod implements wrapping,
//         y: (viewport_loc.y + viewport_offset.y) % map_dims.height // mod implements wrapping
//     }
// }

/*

map_coord: 0
viewport_offset: 3
viewport_width: 4
map_width: 10

..>..<....
None



map_coord: 0
viewport_offset: 0
viewport_width: 10
map_width: 4

>        <
....
0



map_coord: 0
viewport_offset: -2
viewport_width: 10
map_width: 4
>        <
  ....
2

*/
fn map_to_viewport_coord(map_coord: u16, viewport_offset: u16, viewport_width: u16, map_dimension_width: u16) -> Result<Option<u16>,String> {
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
fn map_to_viewport_coords(map_loc: Location, viewport_offset: Vec2d<u16>, viewport_dims: Dims, map_dims: Dims) -> Option<Location> {
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
pub(in crate::ui) struct Map {
    rect: Rect,
    map_dims: Dims,
    old_viewport_offset: Vec2d<u16>,
    viewport_offset: Vec2d<u16>,
    displayed_tiles: LocationGrid<Option<Tile>>,
    displayed_tile_currentness: LocationGrid<Option<bool>>,
    palette: Rc<Palette>,
    unicode: bool,
}
impl Map {
    pub(in crate::ui) fn new(rect: Rect, map_dims: Dims, palette: Rc<Palette>, unicode: bool) -> Self {
        let displayed_tiles = LocationGrid::new(rect.dims(), |_loc| None);
        let displayed_tile_currentness = LocationGrid::new(rect.dims(), |_loc| None);
        Map{
            rect,
            map_dims,
            old_viewport_offset: Vec2d::new(0, 0),
            viewport_offset: Vec2d::new(rect.width / 2, rect.height / 2),
            displayed_tiles,
            displayed_tile_currentness,
            palette,
            unicode,
        }
    }

    fn viewport_dims(&self) -> Dims {
        self.rect.dims()
    }

    #[deprecated = "Replace with ScrollableComponent::scroll_relative"]
    pub fn shift_viewport<V:Into<Vec2d<i32>>>(&mut self, shift: V) {
        let shift: Vec2d<i32> = shift.into();
        
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

    pub fn map_to_viewport_coords(&self, map_loc: Location) -> Option<Location> {
        map_to_viewport_coords(map_loc, self.viewport_offset, self.viewport_dims(), self.map_dims)
    }

    /// If the viewport location given is within the currently visible view and a map location corresponds thereto,
    /// return the map location; otherwise return None.
    pub fn viewport_to_map_coords(&self, game: &PlayerTurnControl, viewport_loc: Location) -> Option<Location> {
        self.viewport_to_map_coords_by_offset(game, viewport_loc, self.viewport_offset)
    }

    fn viewport_to_map_coords_by_offset(&self, game: &PlayerTurnControl, viewport_loc: Location, offset: Vec2d<u16>) -> Option<Location> {
        if self.viewport_dims().contain(viewport_loc) {
            let offset = Vec2d{ x: offset.x as i32, y: offset.y as i32 };
            return game.wrapping().wrapped_add(game.dims(), viewport_loc, offset);
            // let map_loc: Location = viewport_loc + offset;
            // if game.dims().contain(map_loc) {
            //     return Some(map_loc)
            // }
        }

        None
    }

    /// Center the viewport around the tile corresponding to map location `map_loc`.
    pub fn center_viewport(&mut self, map_loc: Location) {
        let new_viewport_offset = Vec2d {
            x: nonnegative_mod(
                i32::from(map_loc.x) - (i32::from(self.rect.width) / 2),
                self.map_dims.width
            ),
            y: nonnegative_mod(
                i32::from(map_loc.y) - (i32::from(self.rect.height) / 2),
                self.map_dims.height
            )
        };

        self.set_viewport_offset(new_viewport_offset);
    }

    /// Center the viewport around the tile corresponding to map location `map_loc` if it is not visible;
    /// otherwise, do nothing.
    pub fn center_viewport_if_not_visible(&mut self, map_loc: Location) {
        if self.map_to_viewport_coords(map_loc).is_none() {
            // The map location is not currently visible in the viewport
            self.center_viewport(map_loc);
        }
    }

    /// Renders a particular location in the viewport
    ///
    /// Flushes stdout for convenience
    pub fn draw_tile_and_flush(&mut self,
            game: &PlayerTurnControl,
            stdout: &mut Stdout,
            viewport_loc: Location,
            highlight: bool,// Highlighting as for a cursor
            unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders

            // Pretend the given city is actually at this location (instead of what might really be there)
            city_override: Option<Option<&City>>,

            // Pretend the given unit is actually at this location (instead of what might really be there)
            unit_override: Option<Option<&Unit>>,
            
            // A symbol to display instead of what's really here
            symbol_override: Option<&'static str>,
        
            // Override the entire observation that would be at this location, instead of using the current player's
            // observations.
            obs_override: Option<&Obs>,
        ) {
        self.draw_tile_no_flush(game, stdout, viewport_loc, highlight, unit_active, city_override, unit_override,
                                symbol_override, obs_override);
        stdout.flush().unwrap();
    }

    /// Renders a particular location in the viewport
    pub fn draw_tile_no_flush(&mut self,
            game: &PlayerTurnControl,
            stdout: &mut Stdout,
            viewport_loc: Location,
            highlight: bool,// Highlighting as for a cursor
            unit_active: bool,// Indicate that the unit (if present) is active, i.e. ready to respond to orders

            // Pretend the given city is actually at this location (instead of what might really be there)
            city_override: Option<Option<&City>>,

            // Pretend the given unit is actually at this location (instead of what might really be there)
            unit_override: Option<Option<&Unit>>,
            
            // A symbol to display instead of what's really here
            symbol_override: Option<&'static str>,
        
            // Override the entire observation that would be at this location, instead of using the current player's
            // observations.
            obs_override: Option<&Obs>,
        ) {



        stdout.queue(SetAttribute(Attribute::Reset)).unwrap();
        stdout.queue(SetBackgroundColor(self.palette.get_single(Colors::Background))).unwrap();


        stdout.queue(self.goto(viewport_loc.x, viewport_loc.y)).unwrap();


        let should_clear = if let Some(tile_loc) = self.viewport_to_map_coords(game, viewport_loc) {

            if tile_loc.y == game.dims().height - 1 {
                stdout.queue(SetAttribute(Attribute::Underlined)).unwrap();
            }

            let obs = obs_override.unwrap_or_else(|| game.current_player_obs(tile_loc));

            if let Obs::Observed{tile, current, ..} = obs {
                if highlight {
                    stdout.queue(SetAttribute(Attribute::Reverse)).unwrap();
                }

                if unit_active {
                    stdout.queue(SetAttribute(Attribute::SlowBlink)).unwrap();
                    stdout.queue(SetAttribute(Attribute::Bold)).unwrap();
                }

                let city: Option<&City> = if let Some(city_override) = city_override {
                    city_override
                } else {
                    tile.city.as_ref()
                };

                let unit: Option<&Unit> = if let Some(unit_override) = unit_override {
                    unit_override
                } else {
                    tile.unit.as_ref()
                };

                 // Incorporate the city and unit overrides (if any) into the tile we store for future reference
                 let tile = {
                     let mut tile = tile.clone();//CLONE

                    if city_override.is_some() {
                        tile.city = city.map(|city| city.clone());//CLONE
                    }
                    if unit_override.is_some() {
                        tile.unit = unit.map(|unit| unit.clone());//CLONE
                    }
                    tile
                };

                let (sym, fg_color, bg_color) = if let Some(ref unit) = unit {
                    if let Some(orders) = unit.orders {
                        if orders == Orders::Sentry {
                            stdout.queue(SetAttribute(Attribute::Italic)).unwrap();
                        }
                    }

                    (unit.sym(self.unicode), unit.color(), tile.terrain.color())
                } else if let Some(ref city) = city {
                    (city.sym(self.unicode), city.alignment.color(), tile.terrain.color())
                } else {
                    (tile.sym(self.unicode), None, tile.terrain.color())
                };

                if let Some(fg_color) = fg_color {
                    stdout.queue(SetForegroundColor(self.palette.get(fg_color, *current))).unwrap();
                }
                if let Some(bg_color) = bg_color {
                    stdout.queue(SetBackgroundColor(self.palette.get(bg_color, *current))).unwrap();
                }
                stdout.queue(Print(String::from(symbol_override.unwrap_or(sym)))).unwrap();

               

                self.displayed_tiles[viewport_loc] = Some(tile);
                self.displayed_tile_currentness[viewport_loc] = Some(*current);

                false
            } else {
                true
            }
        } else {
            true
        };

        if should_clear {
            if highlight {
                stdout.queue(SetBackgroundColor(self.palette.get_single(Colors::Cursor))).unwrap();
            }
            stdout.queue(Print(String::from(" "))).unwrap();
            self.displayed_tiles[viewport_loc] = None;
            self.displayed_tile_currentness[viewport_loc] = None;
        }

        // write!(stdout, "{}", StrongReset::new(&self.palette)).unwrap();
        stdout.queue(SetAttribute(Attribute::Reset)).unwrap();
        stdout.queue(SetBackgroundColor(self.palette.get_single(Colors::Background))).unwrap();
        // stdout.flush().unwrap();
    }

    pub fn current_player_tile<'a>(&self, game: &'a PlayerTurnControl, viewport_loc: Location) -> Option<&'a Tile> {
        // let tile_loc = viewport_to_map_coords(game.dims(), viewport_loc, self.viewport_offset);
        // game.current_player_tile(tile_loc)
        self.viewport_to_map_coords(game, viewport_loc).and_then(|map_loc| game.current_player_tile(map_loc))
    }
}

impl ScrollableComponent for Map {
    fn scroll_relative<V:Into<Vec2d<i32>>>(&mut self, offset: V) {
        self.shift_viewport(offset);
    }

    fn offset(&self) -> Vec2d<u16> { self.viewport_offset }
}

impl Component for Map {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn is_done(&self) -> bool { false }
}

impl Draw for Map {
    fn draw_no_flush(&mut self, game: &PlayerTurnControl, stdout: &mut Stdout, _palette: &Palette) {
        
        for viewport_loc in self.viewport_dims().iter_locs() {

            let should_draw_tile = {
                // let old_map_loc = viewport_to_map_coords(game.dims(), viewport_loc, self.old_viewport_offset);
                // let new_map_loc = viewport_to_map_coords(game.dims(), viewport_loc, self.viewport_offset);

                let old_map_loc: Option<Location> = self.viewport_to_map_coords_by_offset(game, viewport_loc, self.old_viewport_offset);
                let new_map_loc: Option<Location> = self.viewport_to_map_coords(game, viewport_loc);


                // let new_obs = game.current_player_obs(new_map_loc);
                let new_obs = new_map_loc.map(|new_map_loc| game.current_player_obs(new_map_loc));

                let old_currentness = self.displayed_tile_currentness[viewport_loc];
                // let new_currentness = if let Obs::Observed{current,..} = new_obs {
                //     Some(*current)
                // } else {
                //     None
                // };
                let new_currentness = if let Some(Obs::Observed{current,..}) = new_obs {
                    Some(*current)
                } else {
                    None
                };

                

                let old_tile = self.displayed_tiles[viewport_loc].as_ref();
                // let new_tile = &game.current_player_tile(new_map_loc);
                let new_tile = new_map_loc.and_then(|new_map_loc| game.current_player_tile(new_map_loc));
                // let new_tile = &new_obs.tile;

                (old_currentness != new_currentness) ||
                (old_tile.is_some() && new_tile.is_none()) ||
                (old_tile.is_none() && new_tile.is_some()) ||
                (old_tile.is_some() && new_tile.is_some() && {
                    let old = old_tile.unwrap();
                    let new = new_tile.unwrap();
                    let redraw_for_mismatch = !(
                        old.terrain==new.terrain &&
                        old.sym(self.unicode) == new.sym(self.unicode) &&
                        old.alignment_maybe() == new.alignment_maybe()
                    );
                    redraw_for_mismatch
                }) || {
                    let redraw_for_border = if let Some(old_map_loc) = old_map_loc {
                        if let Some(new_map_loc) = new_map_loc {
                            old_map_loc.y != new_map_loc.y && (
                                old_map_loc.y == game.dims().height - 1 ||
                                new_map_loc.y == game.dims().height - 1
                            )
                        } else {
                            false
                        }
                    } else {
                        false
                    };


                    // let redraw_for_border =
                    // old_map_loc.y != new_map_loc.y && (
                    //     old_map_loc.y == game.dims().height - 1 ||
                    //     new_map_loc.y == game.dims().height - 1
                    // );
                    redraw_for_border
                }
            };

            if should_draw_tile {
                self.draw_tile_no_flush(game, stdout, viewport_loc, false, false,
                     None, None, None, None);
            }
        }

        // write!(stdout, "{}{}", StrongReset::new(&self.palette), Hide).unwrap();
        stdout.queue(SetAttribute(Attribute::Reset)).unwrap();
        stdout.queue(SetBackgroundColor(self.palette.get_single(Colors::Background))).unwrap();
        stdout.queue(Hide).unwrap();
    }
}


#[cfg(test)]
mod test {
    use std::rc::Rc;

    use crate::{
        color::{
            Palette,
            palette16,
        },
        test_support::game1,
        ui::map::map_to_viewport_coord,
        util::{
            Dims,
            Location,
            Rect,
            Vec2d,
        },
    };

    use super::Map;

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

    #[test]
    fn test_viewport_to_map_coords_by_offset() {
        let palette = Rc::new(palette16());
        _test_viewport_to_map_coords(Dims::new(20, 20), palette.clone());

    }

    fn _test_viewport_to_map_coords(map_dims: Dims, palette: Rc<Palette>) {
        // pub(in crate::ui) fn new(rect: Rect, map_dims: Dims, palette: Rc<Palette>, unicode: bool) -> Self {

        let mut game = game1();
        let ctrl = game.player_turn_control(0);

        let rect = Rect{
            left: 0,
            top: 0,
            width: map_dims.width,
            height: map_dims.height,
        };
        let mut map = Map::new(rect, map_dims, palette, false);// offset 0,0

        // fn viewport_to_map_coords_by_offset(&self, game: &Game, viewport_loc: Location, offset: Vec2d<u16>) -> Option<Location> {

        assert_eq!(map.viewport_to_map_coords(&ctrl, Location::new(0,0)), Some(Location::new(0,0)));

        map.set_viewport_offset(Vec2d{x: 5, y: 6});

        assert_eq!(map.viewport_to_map_coords(&ctrl, Location::new(0,0)), Some(Location::new(5,6)));

        

    }
}
