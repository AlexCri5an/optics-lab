//! Error types for format parsing and serialization.

use thiserror::Error;

/// Errors that can occur when parsing or processing an optical format file.
#[derive(Debug, Error, PartialEq)]
pub enum FormatParseError {
    /// An operand was recognized but its expected value was missing.
    #[error("Line {line}: missing value for operand '{operand}'")]
    MissingOperandValue {
        /// Line number (1-based).
        line: usize,
        /// Format operand mnemonic.
        operand: &'static str,
    },

    /// An operand contained an invalid floating-point literal.
    #[error("Line {line}: invalid floating-point number '{token}' for operand '{operand}'")]
    InvalidFloat {
        /// Line number (1-based).
        line: usize,
        /// Format operand mnemonic.
        operand: &'static str,
        /// Unparseable token.
        token: String,
    },

    /// An operand contained an invalid integer literal.
    #[error("Line {line}: invalid integer '{token}' for operand '{operand}'")]
    InvalidInteger {
        /// Line number (1-based).
        line: usize,
        /// Format operand mnemonic.
        operand: &'static str,
        /// Unparseable token.
        token: String,
    },

    /// A surface index is out of sequence or negative.
    #[error("Line {line}: invalid surface index {index}")]
    InvalidSurfaceIndex {
        /// Line number (1-based).
        line: usize,
        /// Surface index parsed.
        index: usize,
    },

    /// General line syntax or format error.
    #[error("Line {line}: syntax error on operand '{operand}': {message}")]
    SyntaxError {
        /// Line number (1-based).
        line: usize,
        /// Format operand mnemonic.
        operand: String,
        /// Error description.
        message: String,
    },

    /// The parsed format contains no optical surfaces.
    #[error("Parsed optical system contains no optical surfaces")]
    NoSurfaces,

    /// Text decoding error (e.g. invalid UTF-8 or UTF-16).
    #[error("Failed to decode text: {0}")]
    TextDecodeError(String),
}
