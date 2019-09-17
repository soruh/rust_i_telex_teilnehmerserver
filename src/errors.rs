use crate::State;
use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct MyError {
    inner: Context<MyErrorKind>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum MyErrorKind {
    #[fail(display = "Failed to write to the client socket.")]
    FailedToWrite,

    #[fail(display = "The remote client closed the connection unexpectedly.")]
    ConnectionCloseUnexpected,

    #[fail(display = "Invalid user input.")]
    UserInputError,

    #[fail(display = "Failed to parse package of type {}.", _0)]
    ParseFailure(u8),

    #[fail(display = "Invalid state. Expected {:?} was {:?}", _0, _1)]
    InvalidState(State, State),
}

impl Fail for MyError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl MyError {
    pub fn kind(&self) -> MyErrorKind {
        *self.inner.get_context()
    }
}

impl From<MyErrorKind> for MyError {
    fn from(kind: MyErrorKind) -> MyError {
        MyError {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<MyErrorKind>> for MyError {
    fn from(inner: Context<MyErrorKind>) -> MyError {
        MyError { inner: inner }
    }
}
