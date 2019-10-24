use crate::State;
use thiserror::Error;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Error)]
pub enum MyErrorKind {
    #[error("Failed to write to the client socket.")]
    FailedToWrite,

    #[error("The remote client closed the connection unexpectedly.")]
    ConnectionCloseUnexpected,

    #[error("Invalid user input.")]
    UserInputError,

    #[error("Failed to parse package of type {}.", _0)]
    ParseFailure(u8),

    #[error("Invalid state. Expected {:?} was {:?}", _0, _1)]
    InvalidState(State, State),
}
