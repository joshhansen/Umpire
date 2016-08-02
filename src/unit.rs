extern crate termion;

use termion::color::AnsiValue;

#[derive(Copy,Clone)]
pub enum Alignment {
    NEUTRAL,
    BELLIGERENT { team: u8 }
    // active neutral, chaotic, etc.
}

pub fn alignment_color(alignment: Alignment) -> AnsiValue {
    match alignment {
        Alignment::NEUTRAL => AnsiValue(8),
        Alignment::BELLIGERENT{team} => AnsiValue(team+9)
    }
}

pub enum UnitType {
    CITY,

    INFANTRY,
    ARMOR,
    FIGHTER,
    BOMBER,
    TRANSPORT,
    DESTROYER,
    SUBMARINE,
    CRUISER,
    BATTLESHIP,
    CARRIER
}

pub struct Unit {
    type_: UnitType,
    pub alignment: Alignment,
    hp: u32,
    max_hp: u32,
    x: u16,
    y: u16
}

impl Unit {
    pub fn infantry(alignment: Alignment, x: u16, y: u16) -> Unit {
        Unit {
            type_: UnitType::INFANTRY,
            alignment: alignment,
            hp: 1,
            max_hp: 1,
            x: x,
            y: y
        }
    }

    pub fn city(alignment: Alignment, x: u16, y:u16) -> Unit {
        Unit {
            type_: UnitType::CITY,
            alignment: alignment,
            hp: 1,
            max_hp: 1,
            x: x,
            y: y
        }
    }

    pub fn symbol(&self) -> char {
        match self.type_ {
            UnitType::CITY => '#',
            UnitType::INFANTRY => '⤲',
            UnitType::ARMOR => 'A',
            UnitType::FIGHTER => '✈',
            UnitType::BOMBER => 'b',
            UnitType::TRANSPORT => 't',
            UnitType::DESTROYER => 'd',
            UnitType::SUBMARINE => '—',
            UnitType::CRUISER => 'c',
            UnitType::BATTLESHIP => 'B',
            UnitType::CARRIER => 'C'
        }
    }

    pub fn name(&self) -> &'static str {
        match self.type_ {
            UnitType::CITY => "City",
            UnitType::INFANTRY => "Infantry",
            UnitType::ARMOR => "Armor",
            UnitType::FIGHTER => "Fighter",
            UnitType::BOMBER => "Bomber",
            UnitType::TRANSPORT => "Transport",
            UnitType::DESTROYER => "Destroyer",
            UnitType::SUBMARINE => "Submarine",
            UnitType::CRUISER => "Cruiser",
            UnitType::BATTLESHIP => "Battleship",
            UnitType::CARRIER => "Carrier"
        }
    }
}
