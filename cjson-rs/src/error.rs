//! Error and source-position types.

use thiserror::Error;

/// One-based line and column, plus zero-based byte offset into the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub const START: Position = Position {
        line: 1,
        column: 1,
        offset: 0,
    };
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum Error {
    #[error("unexpected character {ch:?} at line {}, column {}", pos.line, pos.column)]
    UnexpectedChar { ch: char, pos: Position },

    #[error("unexpected end of input at line {}, column {}", pos.line, pos.column)]
    UnexpectedEof { pos: Position },

    #[error("invalid escape sequence at line {}, column {}", pos.line, pos.column)]
    InvalidEscape { pos: Position },

    #[error("invalid unicode escape at line {}, column {}", pos.line, pos.column)]
    InvalidUnicodeEscape { pos: Position },

    #[error("invalid surrogate pair at line {}, column {}", pos.line, pos.column)]
    InvalidSurrogatePair { pos: Position },

    #[error("invalid number at line {}, column {}", pos.line, pos.column)]
    InvalidNumber { pos: Position },

    #[error("number out of range at line {}, column {}", pos.line, pos.column)]
    NumberOutOfRange { pos: Position },

    #[error("invalid literal at line {}, column {}", pos.line, pos.column)]
    InvalidLiteral { pos: Position },

    #[error("nesting limit exceeded at line {}, column {}", pos.line, pos.column)]
    NestingLimitExceeded { pos: Position },

    #[error("invalid UTF-8 byte sequence at line {}, column {}", pos.line, pos.column)]
    InvalidUtf8 { pos: Position },

    #[error("trailing data after value at line {}, column {}", pos.line, pos.column)]
    TrailingData { pos: Position },

    #[error("unexpected token at line {}, column {}", pos.line, pos.column)]
    UnexpectedToken { pos: Position },
}

impl Error {
    /// Return the source position the error refers to.
    pub fn position(&self) -> Position {
        match self {
            Error::UnexpectedChar { pos, .. }
            | Error::UnexpectedEof { pos }
            | Error::InvalidEscape { pos }
            | Error::InvalidUnicodeEscape { pos }
            | Error::InvalidSurrogatePair { pos }
            | Error::InvalidNumber { pos }
            | Error::NumberOutOfRange { pos }
            | Error::InvalidLiteral { pos }
            | Error::NestingLimitExceeded { pos }
            | Error::InvalidUtf8 { pos }
            | Error::TrailingData { pos }
            | Error::UnexpectedToken { pos } => *pos,
        }
    }
}
