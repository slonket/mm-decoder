use crate::mm_address_lut::MM_ADDRESS_LUT;

// wrapper to contain speed or change direction
pub enum MmSpeed {
    Reverse, // change direction
    Speed(u8) // actual speed (0-14)
}

/// `MmLocoCommand` - MM loco packet instruction abstracted from the data bits.
/// 
/// The data section of an MM packet is complex due to MM2's protocol extension. In MM1, the 8 bits of data
/// are actually 4 paired bits (every two bits are the same). This makes for only 16 values, and thus, 16 different
/// speed instructions. Many MM1 decoders only read the leading bit of each pair leaving the remaining 4 bits
/// functionally unused.
/// 
/// MM2 exploits this by encoding additional data in the remaining 4 bits. If the remaining four bits differ from
/// their leading counterparts, the packet is considered of type MM2, while remaining compatible with decoders of
/// MM1. The meaning of the remaining four bits is quite complex and can differ depending on the leading "speed" bits
/// of each pair.
/// 
/// This type is a superset of every possible set of information contained in a data packet. The simplest is `OldSpeed`
/// which contains MM1 type speed information only. `NewSpeed` contains both MM1 speed information and absolute direction.
/// `Function` contains the MM1 speed information along with one function F1-F4 status.
pub enum MmLocoCommand {
    OldSpeed(MmSpeed), // MM1 (old) speed
    NewSpeed{ speed: MmSpeed, direction: bool }, // MM2 (new) speed with absolute directon
    Function{ speed: MmSpeed, function: u8, state: bool } // MM2 (new) speed with function
}

/// MmLocoPacket
#[derive(PartialEq)]
pub struct MmLocoPacket {
    address: u8,
    middle: bool,
    data: u8,
}

impl MmLocoPacket {
    pub fn new(address: u8, middle: bool, data: u8) -> Self {
        Self {
            address,
            middle,
            data,
        }
    }

    /// Returns a decimal reprenstation of the trinary address.
    /// This function currently only allows for the 80 addresses as in the original Marklin-motorola protocol and ignores
    /// extended addresses that have since been implemented. As such, addresses that are considered illegal will return None.
    /// This includes the idle packet address.
    pub fn address(&self) -> Option<u8> {
        let mut ret_address: u8 = 0;

        // compute the trits individually (MST is LAST: A1-A2-A3-A4)
        for i in 0..4 {
            let trit = (self.address >> (i * 2)) & 0b0000_0011;
            let bin_trit = match trit {
                0b00 => 0,
                0b11 => 1,
                0b10 => 2,
                _ => return None, // 0b01 is currently considered invalid
            };
            ret_address = (ret_address * 3) + bin_trit;
        }

        // the transmitted address 0 is actually 80, and transmitted 80 is the idle packet address
        match ret_address {
            80 => return None,
            0 => return Some(80),
            _ => return Some(ret_address),
        }
    }

    /// Returns an address in the range of 0-255 (u8) using the extended motorola address encoding.
    /// This encoding uses the trinary-illegal values to encode the addresses 81-255. An address of 0 represents the idle address.
    pub fn ext_address(&self) -> u8 {
        MM_ADDRESS_LUT[self.address as usize]
    }

    /// Returns the headlight function (middle bit) value.
    pub fn f0(&self) -> bool {
        self.middle
    }

    /// Exclusively decodes information from MM1 (old).
    pub fn speed(&self) -> MmSpeed {
        let d = self.data;
        let dcba: u8 = 
            (d & 0b1000_0000) >> 7 |
            (d & 0b0010_0000) >> 4 |
            (d & 0b0000_1000) >> 1 |
            (d & 0b0000_0010) << 2;
        match dcba {
            0 => MmSpeed::Speed(0),
            1 => MmSpeed::Reverse,
            _ => MmSpeed::Speed(dcba - 1),
        }
    }

    /// Decodes information from both MM1 (old) and MM2 (new).
    pub fn command(&self) -> MmLocoCommand {
        let d = self.data;

        // speed bits (MM1)
        let dcba: u8 = 
            (d & 0b1000_0000) >> 7 |
            (d & 0b0010_0000) >> 4 |
            (d & 0b0000_1000) >> 1 |
            (d & 0b0000_0010) << 2;

        // sub-command bits (MM2) - reversed order to match dcba
        let hgfe: u8 = 
            (d & 0b0100_0000) >> 6 |
            (d & 0b0001_0000) >> 3 |
            (d & 0b0000_0100) |
            (d & 0b0000_0001) << 3;

        // MM1 speed information
        let speed = match dcba {
            0 => MmSpeed::Speed(0),
            1 => MmSpeed::Reverse,
            _ => MmSpeed::Speed(dcba - 1),
        };

        // simplest case is MM1 speed type:
        if dcba == hgfe {
            return MmLocoCommand::OldSpeed(speed);
        }

        // checking for MM2 speed information - some varaints are instead functions
        match hgfe {
            0b0101 => { // speeds -14 to -7 (includes function exceptions)
                match dcba {
                    3 => return MmLocoCommand::Function { speed, function: 1, state: false }, // speed 2
                    4 => return MmLocoCommand::Function { speed, function: 2, state: false }, // speed 3
                    6 => return MmLocoCommand::Function { speed, function: 3, state: false }, // speed 5
                    7 => return MmLocoCommand::Function { speed, function: 4, state: false }, // speed 6
                    _ => return MmLocoCommand::NewSpeed { speed, direction: false },
                }
            }
            0b1101 => { // speeds -6 to -0
                return MmLocoCommand::NewSpeed { speed, direction: false }
            }
            0b1010 => { // speeds +0 to +6 (includes function exceptions)
                match dcba {
                    11 => return MmLocoCommand::Function { speed, function: 1, state: true }, // speed 2
                    12 => return MmLocoCommand::Function { speed, function: 2, state: true }, // speed 3
                    14 => return MmLocoCommand::Function { speed, function: 3, state: true }, // speed 5
                    15 => return MmLocoCommand::Function { speed, function: 4, state: true }, // speed 6
                    _ => return MmLocoCommand::NewSpeed { speed, direction: true }
                }
            }
            0b0010 => { // speeds +7 to +14
                return MmLocoCommand::NewSpeed { speed, direction: true };
            }
            _ => {}
        };

        // not a speed command, thus must be a normal function command
        let gfe = hgfe & 0b0000_0111;
        let h = hgfe & 0b0000_1000 != 0;

        match gfe {
            0b011 => MmLocoCommand::Function { speed, function: 1, state: h },
            0b100 => MmLocoCommand::Function { speed, function: 2, state: h },
            0b110 => MmLocoCommand::Function { speed, function: 3, state: h },
            0b111 => MmLocoCommand::Function { speed, function: 4, state: h },
            _ => MmLocoCommand::OldSpeed(speed), // the new protocol is not exhaustive - this is a safety net
        }
    }
}

/// `MmAccCommand` - MM accessory packet instruction abstracted from the data bits.
/// 
/// The higher frequency MM accessory packets are also used for locomotive functions in MM1. This is determined
/// by the middle bits; 1 = accessory, 0 = loco function.
pub enum MmAccCommand {
    Acc(MmAccPacket),
    Func(MmFuncPacket),
}

// MmAccPacket
#[derive(PartialEq)]
pub struct MmRawAccPacket {
    address: u8,
    middle: bool,
    data: u8,
}

impl MmRawAccPacket {

    pub fn new(address: u8, middle: bool, data: u8) -> Self {
        Self {
            address,
            middle,
            data,
        }
    }

    // Resolves the type as either accessory or loco functions
    pub fn get_type(self) -> MmAccCommand {
        match self.middle {
            false => MmAccCommand::Acc(MmAccPacket { address: self.address, data: self.data }),
            true => MmAccCommand::Func(MmFuncPacket { address: self.address, data: self.data }),
        }
    }
}

pub struct MmAccPacket {
    address: u8,
    data: u8,
}

impl MmAccPacket {
    /// Returns a decimal reprenstation of the trinary address.
    /// This function currently only allows for the 80 addresses as in the original Marklin-motorola protocol and ignores
    /// extended addresses that have since been implemented. As such, addresses that are considered illegal will return None.
    /// This includes the idle packet address.
    pub fn address(&self) -> Option<u8> {
        let mut ret_address: u8 = 0;

        // compute the trits individually (MST is LAST - A1-A2-A3-A4)
        for i in 0..4 {
            let trit = (self.address >> (i * 2)) & 0b0000_0011;
            let bin_trit = match trit {
                0b00 => 0,
                0b11 => 1,
                0b10 => 2,
                _ => return None, // 0b01 is currently considered invalid
            };
            ret_address = (ret_address * 3) + bin_trit;
        }

        // the transmitted address 0 is actually 80, and transmitted 80 is the idle packet address
        match ret_address {
            80 => return None,
            0 => return Some(80),
            _ => return Some(ret_address),
        }
    }

    /// Returns an address in the range of 0-255 (u8) using the extended motorola address encoding.
    /// This encoding uses the trinary-illegal values to encode the addresses 81-255. An address of 0 represents the idle address.
    pub fn ext_address(&self) -> u8 {
        MM_ADDRESS_LUT[self.address as usize]
    }

    // Provides the port (0..7) and its corresponding state
    pub fn output(&self) -> (u8, bool) {
        let d = self.data;

        let port = 
            (d & 0b1000_0000) >> 7 |
            (d & 0b0010_0000) >> 4 |
            (d & 0b0000_0100);

        let state = d & 0b0000_0011 != 0;

        (port, state)
    }
}

pub struct MmFuncPacket {
    address: u8,
    data: u8,
}

impl MmFuncPacket {
    /// Returns a decimal reprenstation of the trinary address.
    /// This function currently only allows for the 80 addresses as in the original Marklin-motorola protocol and ignores
    /// extended addresses that have since been implemented. As such, addresses that are considered illegal will return None.
    /// This includes the idle packet address.
    pub fn address(&self) -> Option<u8> {
        let mut ret_address: u8 = 0;

        // compute the trits individually (MST is LAST - A1-A2-A3-A4)
        for i in 0..4 {
            let trit = (self.address >> (i * 2)) & 0b0000_0011;
            let bin_trit = match trit {
                0b00 => 0,
                0b11 => 1,
                0b10 => 2,
                _ => return None, // 0b01 is currently considered invalid
            };
            ret_address = (ret_address * 3) + bin_trit;
        }

        // the transmitted address 0 is actually 80, and transmitted 80 is the idle packet address
        match ret_address {
            80 => return None,
            0 => return Some(80),
            _ => return Some(ret_address),
        }
    }

    /// Returns an address in the range of 0-255 (u8) using the extended motorola address encoding.
    /// This encoding uses the trinary-illegal values to encode the addresses 81-255. An address of 0 represents the idle address.
    pub fn ext_address(&self) -> u8 {
        MM_ADDRESS_LUT[self.address as usize]
    }

    // Boolean array for functions F1-F4. Index 0 = F1.
    pub fn states(&self) -> [bool; 4] {
        let mut states = [false; 4];
        states[0] = (self.data & 0b1100_0000) != 0;
        states[1] = (self.data & 0b0011_0000) != 0;
        states[2] = (self.data & 0b0000_1100) != 0;
        states[3] = (self.data & 0b0000_0011) != 0;
        states
    }
}
