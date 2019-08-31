use rand::{thread_rng,Rng};

use unit::{City,Unit,CITY_MAX_HP};

#[derive(Debug,PartialEq)]
pub enum CombatParticipant {
    Attacker,
    Defender
}

#[derive(Debug,PartialEq)]
pub struct CombatOutcome<A:CombatCapable,D:CombatCapable> {
    victor: CombatParticipant,
    attacker: A,
    defender: D,
    received_damage_sequence: Vec<CombatParticipant>
}

impl <A:CombatCapable,D:CombatCapable> CombatOutcome<A,D> {
    fn new(victor: CombatParticipant, attacker: A, defender: D, received_damage_sequence: Vec<CombatParticipant>) -> Self {
        CombatOutcome {
            victor,
            attacker,
            defender,
            received_damage_sequence
        }
    }

    pub fn victor(&self) -> &CombatParticipant {
        &self.victor
    }

    pub fn received_damage_sequence(&self) -> &Vec<CombatParticipant> {
        &self.received_damage_sequence
    }

    pub fn attacker(&self) -> &A {
        &self.attacker
    }

    pub fn defender(&self) -> &D {
        &self.defender
    }

    /// Was the unit initiating combat destroyed?
    pub fn destroyed(&self) -> bool {
        *self.victor() == CombatParticipant::Defender
    }

    /// Was the unit initiating combat victorious?
    pub fn victorious(&self) -> bool {
        *self.victor() == CombatParticipant::Attacker
    }
}

pub trait CombatCapable {
    fn hp(&self) -> u16;
    fn max_hp(&self) -> u16;

    fn fight<D:CombatCapable+Clone>(&self, defender: &D) -> CombatOutcome<Self,D>
            where Self: Clone+Sized {

        let mut damage_received: Vec<CombatParticipant> = Vec::new();

        let attacker_initial_hp = self.hp();
        let defender_initial_hp = defender.hp();

        let mut attacker_hp = attacker_initial_hp;
        let mut defender_hp = defender_initial_hp;

        let mut rng = thread_rng();
        while attacker_hp > 0 && defender_hp > 0 {
            let attacker_received_damage = rng.gen::<bool>();
            if attacker_received_damage {
                damage_received.push(CombatParticipant::Attacker);
                attacker_hp -= 1;
            } else {
                damage_received.push(CombatParticipant::Defender);
                defender_hp -= 1;
            }

            if attacker_hp == 0 || defender_hp == 0 {
                let victor = if attacker_hp == 0 { CombatParticipant::Defender } else { CombatParticipant::Attacker };

                //FIXME These clones could be pretty expensive
                return CombatOutcome::new(victor, self.clone(), defender.clone(), damage_received);
            }
        }

        panic!("For some inexplicable reason, combat failed to produce a victor");
    }
}

impl CombatCapable for Unit {
    fn hp(&self) -> u16 { self.hp }
    fn max_hp(&self) -> u16 { self.max_hp }
}

impl CombatCapable for City {
    fn hp(&self) -> u16 { self.hp }
    fn max_hp(&self) -> u16 { CITY_MAX_HP }
}
