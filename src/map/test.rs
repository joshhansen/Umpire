use map::{Terrain,Tile};
use unit::{Alignment,Unit,UnitType};
use util::Location;


#[test]
fn test_tile() {
    let loc = Location{x: 10, y: 10};
    let terrain = Terrain::LAND;

    let tile = Tile::new(terrain, loc);

    if let None = tile.unit {
        assert!(true);
    } else {
        assert!(false);
    }

    let mut tile = tile;

    let unit = Unit::new(UnitType::INFANTRY, Alignment::NEUTRAL);
    tile.set_unit(unit);
    if let Some(unit2) = tile.unit {
        assert_eq!(unit, unit2);
    } else {
        assert!(false);
    }
}
