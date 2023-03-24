use failure::Fail;

use serde::{Deserialize, Serialize};

use crate::{
    game::{
        city::CityID,
        move_::MoveError,
        player::PlayerNum,
        unit::{TransportMode, UnitID},
    },
    util::Location,
};

use super::alignment::Alignment;

#[derive(Debug, Deserialize, Fail, PartialEq, Serialize)]
pub enum GameError {
    #[fail(display = "No player slots available; the game is full")]
    NoPlayerSlotsAvailable,

    #[fail(display = "There is no player {}", player)]
    NoSuchPlayer { player: PlayerNum },

    #[fail(display = "It isn't player {}'s turn", player)]
    NotPlayersTurn { player: PlayerNum },

    #[fail(display = "There is no player identified by the given secret")]
    NoPlayerIdentifiedBySecret,

    #[fail(display = "No unit with ID {:?} exists", id)]
    NoSuchUnit { id: UnitID },

    #[fail(display = "No unit at location {} exists", loc)]
    NoUnitAtLocation { loc: Location },

    #[fail(display = "No city with ID {:?} exists", id)]
    NoSuchCity { id: CityID },

    #[fail(display = "No city at location {} exists", loc)]
    NoCityAtLocation { loc: Location },

    #[fail(display = "No tile at location {} exists", loc)]
    NoTileAtLocation { loc: Location },

    #[fail(display = "Specified unit is not controlled by the current player")]
    UnitNotControlledByCurrentPlayer,

    #[fail(display = "The unit with ID {:?} has no carrying space", carrier_id)]
    UnitHasNoCarryingSpace { carrier_id: UnitID },

    #[fail(
        display = "The relevant carrying space cannot carry the unit with ID {:?} because its transport mode {:?} is
                      incompatible with the carrier's accepted transport mode {:?}",
        carried_id, carried_transport_mode, carrier_transport_mode
    )]
    WrongTransportMode {
        carried_id: UnitID,
        carrier_transport_mode: TransportMode,
        carried_transport_mode: TransportMode,
    },

    #[fail(
        display = "The relevant carrying space cannot carry the unit with ID {:?} due insufficient space.",
        carried_id
    )]
    InsufficientCarryingSpace { carried_id: UnitID },

    #[fail(
        display = "The relevant carrying space cannot carry the unit with ID {:?} because its alignment {:?} differs
                      from the space owner's alignment {:?}.",
        carried_id, carried_alignment, carrier_alignment
    )]
    OnlyAlliesCarry {
        carried_id: UnitID,
        carrier_alignment: Alignment,
        carried_alignment: Alignment,
    },

    #[fail(
        display = "The unit with ID {:?} cannot occupy the city with ID {:?} because the unit with ID {:?} is still
                      garrisoned there. The garrison must be destroyed prior to occupation.",
        occupier_unit_id, city_id, garrisoned_unit_id
    )]
    CannotOccupyGarrisonedCity {
        occupier_unit_id: UnitID,
        city_id: CityID,
        garrisoned_unit_id: UnitID,
    },

    #[fail(display = "There was a problem moving the unit: {}", 0)]
    MoveError(MoveError),

    #[fail(display = "Requirements for ending turn not met for player {}", player)]
    TurnEndRequirementsNotMet { player: PlayerNum },
}
