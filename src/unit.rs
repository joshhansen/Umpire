extern crate termion;

use std::fmt;

use termion::color::AnsiValue;

use util::Location;

pub type PlayerNum = u8;

#[derive(Copy,Clone,PartialEq,Hash,Eq)]
pub enum Alignment {
    NEUTRAL,
    BELLIGERENT { player: PlayerNum }
    // active neutral, chaotic, etc.
}

impl Alignment {
    pub fn color(&self) -> AnsiValue {
        match *self {
            Alignment::NEUTRAL => AnsiValue(8),
            Alignment::BELLIGERENT{player} => AnsiValue(player+9)
        }
    }
}

pub trait Located {
    fn loc(&self) -> Location;
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

#[derive(Clone,Copy,Hash,PartialEq,Eq)]
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

impl UnitType {
    pub fn values() -> Vec<UnitType> {
        vec![
            UnitType::INFANTRY,
            UnitType::ARMOR,
            UnitType::FIGHTER,
            UnitType::BOMBER,
            UnitType::TRANSPORT,
            UnitType::DESTROYER,
            UnitType::SUBMARINE,
            UnitType::CRUISER,
            UnitType::BATTLESHIP,
            UnitType::CARRIER
        ]
    }

    fn max_hp(&self) -> u16 {
        match *self {
            UnitType::INFANTRY => 1,
            UnitType::ARMOR => 2,
            UnitType::FIGHTER => 1,
            UnitType::BOMBER => 1,
            UnitType::TRANSPORT => 3,
            UnitType::DESTROYER => 2,
            UnitType::SUBMARINE => 2,
            UnitType::CRUISER => 4,
            UnitType::BATTLESHIP => 8,
            UnitType::CARRIER => 6
        }
    }

    pub fn cost(&self) -> u16 {
        match *self {
            UnitType::INFANTRY => 6,
            UnitType::ARMOR => 12,//?
            UnitType::FIGHTER => 12,
            UnitType::BOMBER => 12,//?
            UnitType::TRANSPORT => 30,
            UnitType::DESTROYER => 24,
            UnitType::SUBMARINE => 24,
            UnitType::CRUISER => 36,
            UnitType::BATTLESHIP => 60,
            UnitType::CARRIER => 48
        }
    }

    pub fn key(&self) -> char {
        match *self {
            UnitType::INFANTRY => 'i',
            UnitType::ARMOR => 'a',
            UnitType::FIGHTER => 'f',
            UnitType::BOMBER => 'b',
            UnitType::TRANSPORT => 't',
            UnitType::DESTROYER => 'd',
            UnitType::SUBMARINE => 's',
            UnitType::CRUISER => 'c',
            UnitType::BATTLESHIP => 'B',
            UnitType::CARRIER => 'C'
        }
    }

    pub fn from_key(c: &char) -> Option<UnitType> {
        for unit_type in UnitType::values().iter() {
            if unit_type.key() == *c {
                return Some(*unit_type);
            }
        }
        None
    }
}

impl Named for UnitType {
    fn name(&self) -> &'static str {
        match *self {
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

#[derive(Clone)]
pub struct Unit {
    type_: UnitType,
    pub alignment: Alignment,
    hp: u16,
    max_hp: u16,
    loc: Location,
    pub sentry: bool,
    pub moves_remaining: u16
}





impl Unit {
    pub fn new(type_: UnitType, alignment: Alignment, loc: Location) -> Self {
        let max_hp =type_.max_hp();
        Unit {
            type_: type_,
            alignment: alignment,
            hp: max_hp,
            max_hp: max_hp,
            loc: loc,
            sentry: false,
            moves_remaining: 0
        }
    }

    pub fn movement_per_turn(&self) -> u16 {
        match self.type_ {
            UnitType::INFANTRY => 1,
            UnitType::ARMOR => 2,//?
            UnitType::FIGHTER => 5,
            UnitType::BOMBER => 5,//?
            UnitType::TRANSPORT => 2,
            UnitType::DESTROYER => 3,
            UnitType::SUBMARINE => 2,
            UnitType::CRUISER => 2,
            UnitType::BATTLESHIP => 1,
            UnitType::CARRIER => 1
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

impl Located for Unit {
    fn loc(&self) -> Location { self.loc }
}

impl CombatCapable for Unit {
    fn hp(&self) -> u16 { self.hp }
    fn max_hp(&self) -> u16 { self.max_hp }
}

impl Aligned for Unit {
    fn alignment(&self) -> Alignment { self.alignment }
}

#[derive(Clone,Hash,PartialEq,Eq)]
pub struct City {
    pub loc: Location,
    pub alignment: Alignment,
    pub unit_under_production: Option<UnitType>,
    pub production_progress: u16,
}

impl City {
    pub fn new(alignment: Alignment, loc: Location) -> City {
        City {
            loc: loc,
            alignment: alignment,
            unit_under_production: None,
            production_progress: 0
        }
    }
}

impl fmt::Display for City {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "City@{}", self.loc)
    }
}

impl Located for City {
    fn loc(&self) -> Location { self.loc }
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
