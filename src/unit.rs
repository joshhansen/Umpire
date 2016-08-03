extern crate termion;

use termion::color::AnsiValue;

#[derive(Copy,Clone,PartialEq)]
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

pub trait Located {
    fn x(&self) -> u16;
    fn y(&self) -> u16;
}

pub trait CombatCapable {
    fn hp(&self) -> u16;
    fn max_hp(&self) -> u16;
}

pub trait Aligned {
    fn alignment(&self) -> Alignment;
}

pub trait Named {
    fn name(&self) -> &'static str;
}

pub trait Sym {
    fn sym(&self) -> char;
}

pub enum UnitType {
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
    hp: u16,
    max_hp: u16,
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
}

impl Sym for Unit {
    fn sym(&self) -> char {
        match self.type_ {
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
}

impl Named for Unit {
    fn name(&self) -> &'static str {
        match self.type_ {
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

impl Located for Unit {
    fn x(&self) -> u16 { self.x }
    fn y(&self) -> u16 { self.y }
}

impl CombatCapable for Unit {
    fn hp(&self) -> u16 { self.hp }
    fn max_hp(&self) -> u16 { self.max_hp }
}

impl Aligned for Unit {
    fn alignment(&self) -> Alignment { self.alignment }
}

pub struct City {
    x: u16,
    y: u16,
    alignment: Alignment,
    production_type: Option<UnitType>,
    production_progress: u16,
}

impl City {
    pub fn new(alignment: Alignment, x: u16, y:u16) -> City {
        City {
            x: x,
            y: y,
            alignment: alignment,
            production_type: None,
            production_progress: 0
        }
    }
}

impl Located for City {
    fn x(&self) -> u16 { self.x }
    fn y(&self) -> u16 { self.y }
}

impl Aligned for City {
    fn alignment(&self) -> Alignment { self.alignment }
}

impl Sym for City {
    fn sym(&self) -> char { '#' }
}

impl Named for City {
    fn name(&self) -> &'static str { "City "}
}
