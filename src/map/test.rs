use map::{Terrain,Tile};
use unit::{Alignment,Unit,UnitType};
use util::Location;


#[test]
fn test_tile() {
    let loc = Location{x: 10, y: 10};
    let terrain = Terrain::LAND;

    let tile = Tile::new(terrain, loc);

    assert_eq!(tile.unit, None);

    let mut tile = tile;

    let unit = Unit::new(UnitType::INFANTRY, Alignment::NEUTRAL);
    tile.set_unit(unit);
    assert_eq!(tile.unit, Some(unit));
}