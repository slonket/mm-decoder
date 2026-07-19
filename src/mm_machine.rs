use crate::buildable_u8::BuildableU8;
use crate::mm_packet::{MmLocoPacket, MmRawAccPacket};

// Public Types
pub type MmLocoMachine = MmMachine<MmLocoTiming>;
pub type MmAccMachine = MmMachine<MmAccTiming>;

// MmTiming
//
// The state machines are the same for locomotive and accessory MM packets, but the timing is different.
// Because of this, the state machine is made generic over the two pulse timing sets and their respective packet types.
pub trait MmTiming {
    const SH_MIN: u16;
    const SH_MAX: u16;
    const LG_MIN: u16;
    const LG_MAX: u16;

    type Packet: PartialEq;
    fn make_packet(address: u8, middle: bool, data: u8) -> Self::Packet;
}

pub struct MmLocoTiming;
impl MmTiming for MmLocoTiming {
    const SH_MIN: u16 = 26 - 5;
    const SH_MAX: u16 = 26 + 5;
    const LG_MIN: u16 = (7 * 26) - 10;
    const LG_MAX: u16 = (7 * 26) + 10;

    type Packet = MmLocoPacket;
    fn make_packet(address: u8, middle: bool, data: u8) -> Self::Packet {
        MmLocoPacket::new(address, middle, data)
    }
}

pub struct MmAccTiming;
impl MmTiming for MmAccTiming {
    const SH_MIN: u16 = 13 - 5;
    const SH_MAX: u16 = 13 + 5;
    const LG_MIN: u16 = (7 * 13) - 10;
    const LG_MAX: u16 = (7 * 13) + 10;

    type Packet = MmRawAccPacket;
    fn make_packet(address: u8, middle: bool, data: u8) -> Self::Packet {
        MmRawAccPacket::new(address, middle, data)
    }
}



// MmPulse
#[derive(Default, PartialEq, Clone, Copy)]
enum MmPulse {
    Short,
    Long,
    IdlePrev,
    #[default]
    Invalid,
}

impl MmPulse {
    fn classify<T: MmTiming>(value: u16) -> Self {
        match value {
            v if v >= T::SH_MIN && v <= T::SH_MAX => MmPulse::Short,
            v if v >= T::LG_MIN && v <= T::LG_MAX => MmPulse::Long,
            _ => MmPulse::Invalid,
        }
    }
}


// MmBit
#[derive(PartialEq)]
enum MmBit {
    Zero,
    One,
}

impl From<MmBit> for u8 {
    fn from(mm_bit: MmBit) -> Self {
        match mm_bit {
            MmBit::Zero => 0x00,
            MmBit::One => 0x01,
        }
    }
}



// MmMachine
enum MmState {
    Address(BuildableU8),
    Middle(u8),
    Data(BuildableU8),
}

/// A Marklin-Motorola protocol decoder. Feed a contiguous series of microsecond-resolution track pulses. Any invalid pulse will reset the machine.
pub struct MmMachine<T: MmTiming> {
    // state machine values
    state: MmState,
    prev_pulse: MmPulse,
    // packet data (the data field is contained in Data state)
    address: u8,
    middle: bool,
    // for packet comparison (double packeting)
    prev_packet: Option<T::Packet>,
    _timing: core::marker::PhantomData<T>,
}

impl<T: MmTiming> MmMachine<T> {

    /// Create a new MmMachine.
    /// Can be static.
    pub const fn new() -> Self {
        Self {
            state: MmState::Address(BuildableU8::new()),
            prev_pulse: MmPulse::Invalid,
            address: 0,
            middle: false,
            prev_packet: None,
            _timing: core::marker::PhantomData,
        }
    }

    /// Parse a pulse (in microseconds) through the MmMachine.
    /// Will produce a packet after a series of contiguous pulses representing a complete packet are parsed.
    pub fn advance(&mut self, pulse: u16) -> Option<T::Packet> {

        let mm_pulse = MmPulse::classify::<T>(pulse);

        // when a non-MM pulse is encountered, the MM pulse chain has been interrupted
        // the state machine needs to be reset for a new packet. This ensures orientation for when
        // a packet begins; the first valid pulse (Short or Long) will start a packet.
        if mm_pulse == MmPulse::Invalid {
            self.reset();
            return None;
        }

        // MM bit resolver
        // Actions for the state machine are taken on the first "half pulse" without waiting for the compliment
        // Instead, there is a check to see if a compliment mismatches, which will cause a reset of the FSM.
        // The reason for this is that the final "bit" will never have a complimentary pulse; rather, there is
        // a long low period until the next packet.
        let mm_bit = match self.prev_pulse {
            MmPulse::IdlePrev => {
                // IdlePrev indicates this pulse is the "leading" half of interest. Store it for
                // mismatch comparison with the next pulse, and proceed to FSM.
                self.prev_pulse = mm_pulse;
                match mm_pulse {
                    MmPulse::Short => MmBit::Zero,
                    MmPulse::Long => MmBit::One,
                    _ => { unreachable!() }
                }
            }
            _ => {
                // A stored pulse means this is the complimentary pulse. Compare to find a mismatch.
                // If a mismatch happens, it means the FSM is disorientated and has read between bits (one pulse
                // out of sync). There is no recovery from this for the current packet so just reset. This case
                // is essentially impossible as every packet starts orientated do to the "Invalid" reset check.
                if self.prev_pulse == mm_pulse {
                    self.reset();
                }
                // Even if there's no mismatch (normal case) we must reset the previous pulse to idle (waiting for
                // leading pulse of interest) and return None - no FSM action taken on complimentary pulse.
                self.prev_pulse = MmPulse::IdlePrev;
                return None;
            }
        };

        // the state machine - it is guaranteed that an MM bit is resolved
        match &mut self.state {
            MmState::Address(address) => {
                match address.push(mm_bit.into()) {
                    Some(byte) => {
                        self.address = byte;
                        self.state = MmState::Middle(0);
                    }
                    None => {}
                }
            }
            MmState::Middle(bit_count) => {
                // both middle bits are always the same, so we only need to record one
                *bit_count += 1;
                if *bit_count >= 2 {
                    self.middle = mm_bit != MmBit::Zero;
                    self.state = MmState::Data(BuildableU8::new());
                }
            }
            MmState::Data(data) => {
                match data.push(mm_bit.into()) {
                    Some(byte) => {

                        // completed packet - create packet type
                        let new_packet = T::make_packet(self.address, self.middle, byte);
                        
                        // check for matching packet pair
                        let ret = if self.prev_packet.as_ref() == Some(&new_packet) {
                            // matching packets - return the current packet and reset previous packet
                            self.prev_packet = None;
                            Some(new_packet)
                        } else {
                            // packet doesn't match or no previous packet - store current
                            self.prev_packet = Some(new_packet);
                            None
                        };

                        // reset state machine and return
                        self.reset();
                        return ret;
                    }
                    None => {}
                }
            }
        }

        None
    }

    /// Resets the MmMachine. Used when illegal conditions are met.
    #[inline]
    fn reset(&mut self) {
        // the packet doesn't need resetting as there's no indexing variable
        self.state = MmState::Address(BuildableU8::new());
        self.prev_pulse = MmPulse::IdlePrev;
    }
}
