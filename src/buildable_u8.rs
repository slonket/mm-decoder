/// A bit-by-bit byte builder. Shifts bits in from LSB until 8 bits have been pushed,
/// then returns the completed byte.
pub struct BuildableU8 {
    data: u8,
    index: u8,
}

impl BuildableU8 {

    pub const fn new() -> Self {
        Self {
            data: 0,
            index: 0,
        }
    }

    pub fn push(&mut self, bit: u8) -> Option<u8> {

        self.data = (self.data << 1) | (bit & 0x01);
        self.index += 1;

        if self.index >= 8 {
            self.index = 0;
            return Some(self.data);
        }

        None
    }
}
