use crate::common::LokadId;

pub const SLPV2_LOKAD_ID: LokadId = *b"SLP2";
pub const STANDARD_TOKEN_TYPE: u8 = 0;

/// Max. number of inputs we can handle per tx.
/// With current consensus rules, no valid tx can have this many inputs,
/// but we don't want to depend on this.
pub const MAX_TX_INPUTS: usize = 32767;
