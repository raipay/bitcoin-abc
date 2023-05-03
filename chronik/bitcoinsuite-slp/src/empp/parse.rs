use std::ops::Range;

use bitcoinsuite_core::{
    error,
    script::{opcode::*, Op, Script},
};
use bytes::Bytes;
use thiserror::Error;

pub type EmppData = Vec<Bytes>;

const PUSH_RANGE: Range<usize> = 1..76;
const PUSHDATA1_RANGE: Range<usize> = 76..0x100;
const PUSHDATA2_RANGE: Range<usize> = 0x100..0x10000;

/// Errors when parsing a eMPP tx failed.
#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("Failed parsing script: {0}")]
    DataError(#[from] error::DataError),

    #[error("Empty script")]
    EmptyScript,

    #[error("Missing OP_RETURN")]
    MissingOpReturn,

    #[error("Empty OP_RETURN")]
    EmptyOpReturn,

    #[error("Missing OP_RESERVED")]
    MissingOpReserved,

    #[error(
        "Invalid push opcode {0}: OP_0, OP_1NEGATE, OP_RESERVED, OP_1, .. \
         OP_16 not allowed"
    )]
    InvalidPushOpcode(Opcode),

    #[error("Invalid non-push opcode {0}")]
    InvalidNonPushOpcode(Opcode),

    #[error("Invalid payload size {1} for opcode {0}")]
    InvalidOpPayloadSize(Opcode, usize),
}

use self::ParseError::*;

pub fn parse(script: &Script) -> Result<EmppData, ParseError> {
    let mut ops = script.iter_ops();
    let op_return = ops.next().ok_or(EmptyScript)??;
    if !matches!(op_return, Op::Code(OP_RETURN)) {
        return Err(MissingOpReturn);
    }
    let op_reserved = ops.next().ok_or(EmptyOpReturn)??;
    if !matches!(op_reserved, Op::Code(OP_RESERVED)) {
        return Err(MissingOpReserved);
    }
    let mut empp_data = EmppData::new();
    for pushop in ops {
        let payload = match pushop? {
            Op::Code(OP_0) => return Err(InvalidPushOpcode(OP_0)),
            Op::Code(opcode @ Opcode(OP_1NEGATE::N..=OP_16::N)) => {
                return Err(InvalidPushOpcode(opcode))
            }
            Op::Code(opcode) => return Err(InvalidNonPushOpcode(opcode)),
            Op::Push(opcode @ Opcode(1..=0x4b), payload)
                if !PUSH_RANGE.contains(&payload.len()) =>
            {
                return Err(InvalidOpPayloadSize(opcode, payload.len()))
            }
            Op::Push(opcode @ OP_PUSHDATA1, payload)
                if !PUSHDATA1_RANGE.contains(&payload.len()) =>
            {
                return Err(InvalidOpPayloadSize(opcode, payload.len()))
            }
            Op::Push(opcode @ OP_PUSHDATA2, payload)
                if !PUSHDATA2_RANGE.contains(&payload.len()) =>
            {
                return Err(InvalidOpPayloadSize(opcode, payload.len()))
            }
            Op::Push(opcode @ OP_PUSHDATA4, _) => {
                return Err(InvalidPushOpcode(opcode))
            }
            Op::Push(_, payload) => payload,
        };
        empp_data.push(payload);
    }
    Ok(empp_data)
}
