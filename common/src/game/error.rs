use serde::{Deserialize, Serialize};

use thiserror::Error;

use crate::{
    game::{
        city::CityID,
        move_::MoveError,
        player::PlayerNum,
        unit::{TransportMode, UnitID},
    },
    util::Location,
};

use super::{alignment::Alignment, TurnNum, TurnPhase};

#[derive(Debug, Deserialize, Error, PartialEq, Serialize)]
pub enum GameError {
    #[error("Player {player} turn {turn} was unexepctedly in phase {phase:?}")]
    WrongPhase {
        player: PlayerNum,
        turn: TurnNum,
        phase: TurnPhase,
    },

    #[error("No player slots available; the game is full")]
    NoPlayerSlotsAvailable,

    #[error("There is no player {player}")]
    NoSuchPlayer { player: PlayerNum },

    #[error("It isn't player {player}'s turn")]
    NotPlayersTurn { player: PlayerNum },

    #[error("There is no player identified by the given secret")]
    NoPlayerIdentifiedBySecret,

    #[error("No unit with ID {id:?} exists")]
    NoSuchUnit { id: UnitID },

    #[error("No unit at location {loc} exists")]
    NoUnitAtLocation { loc: Location },

    #[error("No city with ID {id:?} exists")]
    NoSuchCity { id: CityID },

    #[error("No city at location {loc} exists")]
    NoCityAtLocation { loc: Location },

    #[error("No tile at location {loc} exists")]
    NoTileAtLocation { loc: Location },

    #[error("Specified unit is not controlled by the current player")]
    UnitNotControlledByCurrentPlayer,

    #[error("The unit with ID {carrier_id:?} has no carrying space")]
    UnitHasNoCarryingSpace { carrier_id: UnitID },

    #[error("The relevant carrying space cannot carry the unit with ID {carried_id:?} because its transport mode {carried_transport_mode:?} is
                      incompatible with the carrier's accepted transport mode {carrier_transport_mode:?}"
    )]
    WrongTransportMode {
        carried_id: UnitID,
        carrier_transport_mode: TransportMode,
        carried_transport_mode: TransportMode,
    },

    #[error("The relevant carrying space cannot carry the unit with ID {carried_id:?} due insufficient space.")]
    InsufficientCarryingSpace { carried_id: UnitID },

    #[error("The relevant carrying space cannot carry the unit with ID {carried_id:?} because its alignment {carried_alignment:?} differs
                      from the space owner's alignment {carrier_alignment:?}.")]
    OnlyAlliesCarry {
        carried_id: UnitID,
        carrier_alignment: Alignment,
        carried_alignment: Alignment,
    },

    #[error("The unit with ID {occupier_unit_id:?} cannot occupy the city with ID {city_id:?} because the unit with ID {garrisoned_unit_id:?} is still
                      garrisoned there. The garrison must be destroyed prior to occupation.")]
    CannotOccupyGarrisonedCity {
        occupier_unit_id: UnitID,
        city_id: CityID,
        garrisoned_unit_id: UnitID,
    },

    #[error("There was a problem moving the unit: {0}")]
    MoveError(MoveError),

    #[error("Requirements for ending turn not met for player {player}")]
    TurnEndRequirementsNotMet { player: PlayerNum },
}
