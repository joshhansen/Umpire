use rand::{thread_rng,Rng};

use unit::Unit;

#[derive(PartialEq)]
pub enum CombatParticipant {
    Attacker,
    Defender
}

pub struct CombatOutcome {
    victor: CombatParticipant,
    attacker_initial_hp: u16,
    defender_initial_hp: u16,
    received_damage_sequence: Vec<CombatParticipant>
}

impl CombatOutcome {
    fn new(victor: CombatParticipant, attacker_initial_hp: u16, defender_initial_hp: u16, received_damage_sequence: Vec<CombatParticipant>) -> Self {
        CombatOutcome {
            victor: victor,
            attacker_initial_hp: attacker_initial_hp,
            defender_initial_hp: defender_initial_hp,
            received_damage_sequence: received_damage_sequence
        }
    }

    pub fn victor(&self) -> &CombatParticipant {
        &self.victor
    }
}

pub trait CombatCapable {
    fn hp(&self) -> u16;
    fn max_hp(&self) -> u16;

    fn fight<D:CombatCapable>(&self, defender: &D) -> CombatOutcome {

        // let attacker = self.tiles[attacker_loc].unit.unwrap();
        // let defender = self.tiles[defender_loc].unit.unwrap();

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

                // log_listener(format!("Unit {} initiated combat with unit {} and was {}",
                //                         self,
                //                         defender,
                //                         if victor==CombatParticipant::Attacker {"victorious"} else {"vanquished"}
                // ));

                return CombatOutcome::new(victor, attacker_initial_hp, defender_initial_hp, damage_received);
            }
        }

        panic!("For some inexplicable reason, combat failed to produce a victor");
    }
}

impl CombatCapable for Unit {
    fn hp(&self) -> u16 { self.hp }
    fn max_hp(&self) -> u16 { self.max_hp }
}
