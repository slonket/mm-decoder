#![no_std]

mod buildable_u8;
pub(crate) mod mm_address_lut;
mod mm_packet;
mod mm_machine;

// State machines
pub use mm_machine::{MmLocoMachine, MmAccMachine};

// Re-export MmMachine so rustdoc can generate type.impl data for type alias pages.
pub use mm_machine::MmMachine;

// Loco packet types
pub use mm_packet::{MmLocoPacket, MmLocoCommand, MmSpeed};

// Accessory packet types
pub use mm_packet::{MmRawAccPacket, MmAccPacket, MmAccCommand, MmFuncPacket};
