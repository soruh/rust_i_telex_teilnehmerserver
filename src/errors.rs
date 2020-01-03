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

    #[error("Failed to serialize package of type {}.", _0)]
    SerializeFailure(u8),

    #[error("Invalid state. Expected {:?} was {:?}", _0, _1)]
    InvalidState(State, State),

    #[error("Client timed out.")]
    Timeout,

    // TODO: remove
    #[cfg(debug_assertions)]
    #[error("Not Yet Implemented: {}:{}:{}", _0, _1, _2)]
    Unimplemented(&'static str, u32, u32),
}


// TODO: remove
macro_rules! err_unimplemented {
    () => {MyErrorKind::Unimplemented(file!(), line!(), column!())}
}
