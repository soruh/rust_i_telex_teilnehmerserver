use super::ClientType;
use crate::State;
use thiserror::Error;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Error)]

pub enum ItelexServerErrorKind {
    #[error("Failed to write to the client socket.")]
    FailedToWrite,

    #[error("The remote client closed the connection unexpectedly.")]
    ConnectionCloseUnexpected,

    #[error("Invalid user input.")]
    UserInputError,

    #[error("Tried to update with an Ipv6 address")]
    Ipv6Address,

    #[error("Invalid client typ. Was {:?}, but must be {:?}", _0, _1)]
    InvalidClientType(ClientType, ClientType),

    #[error("Tried to use a wrong password")]
    PasswordError,

    #[error("Failed to parse package of type {}.", _0)]
    ParseFailure(u8),

    #[error("Failed to serialize package of type {}.", _0)]
    SerializeFailure(u8),

    #[error("Invalid state. Expected {:?} was {:?}", _0, _1)]
    InvalidState(State, State),

    #[error("Client timed out.")]
    Timeout,

    #[cfg(debug_assertions)]
    #[error("Not Yet Implemented: {}:{}:{}", _0, _1, _2)]
    Unimplemented(&'static str, u32, u32),
}

#[allow(unused_macros)]
#[cfg(not(debug_assertions))]
macro_rules! err_unimplemented {
    () => {
        compile_error!("can't use `ItelexServerErrorKind::Unimplemented` in release mode")
    };
}

#[allow(unused_macros)]
#[cfg(debug_assertions)]
macro_rules! err_unimplemented {
    () => {
        errors::ItelexServerErrorKind::Unimplemented(file!(), line!(), column!())
    };
}
