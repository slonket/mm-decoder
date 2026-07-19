# mm-decoder

### A no-std Rust crate for decoding Märklin-Motorola protocol.

## State Machine Usage

A state machine can be created using `MmLocoMachine::new();` for locomotive-frequency transmission and `MmAccMachine::new();` for accessory-frequency transmission. Both can be initialised as `static`. The following can be capture by these two machines:
- `MmLocoMachine` - MM1 locomotive and MM2 locomotive and function packets.
- `MmLocoMachine` - MM1 function packets and MM1/MM2 accessory packets.

The state machines must be fed a series of pulse durations representing track data using `advance(u16 pulse)`, where pulse is a unit of track data. The machines are "polarity insensitive"; data must be a contiguous series of high and low pulse durations. The unit for these pulses is microseconds. For example, a locomotive-frequency motorola '1' would be respresented as two values `26` and `182`.

Short pulse durations have a tolerance of ±5 and long pulse durations have a tolerance of ±10. If your transduced data has some shift from nominal values, this should be corrected for outside of the state machines. The tolerance does not account for transducer issues.

## Example Usage

Typical usage involves retrieving pulses from some buffer. These can be pulse durations captured using a dual-edge sensitive timer in capture mode. Pulses are retrieved and then passed through one (or more) state machines which may produce a packet. See below:

```Rust
static mut MM_LOCO_MACHINE: MmLocoMachine = MmLocoMachine::new();
const ADDRESS: u8 = 1;

loop {

    // retrieve pulses from some buffer
    if let Ok(pulse) = pulse_cons.get() {

    // processing MM loco protocol
    if let Some(packet) = MM_LOCO_MACHINE.advance(pulse) {

        let address = packet.ext_address();

        if address == ADDRESS {

            // f0 is present in every packet
            let f0 = packet.f0();

            // get remaining packet information
            match packet.command() {
                MmLocoCommand::OldSpeed(MmSpeed::Speed(speed)) => {
                    // do something with speed
                }
                MmLocoCommand::OldSpeed(MmSpeed::Reverse) => {
                    // change direction
                }
                MmLocoCommand::NewSpeed { speed: MmSpeed::Speed(speed), direction } => {
                    // do something with speed and direction
                }
                MmLocoCommand::Function { speed: MmSpeed::Speed(speed), function, state } => {
                    // do something with speed and function
                    // example function matching
                    match function {
                        1 => {},
                        2 => {},
                        3 => {},
                        4 => {},
                        _ => {}
                    }
                }
                _ => {} // MM2 reverse commands are ignored; MM2 has absolute direction
            }
        }
    }
}
```

## Disorientation

In the case that your calling code loses track of the contiguous pulses, you can parse any invalid value for the state machines to reset. The easiest and most consistent is `0`; this will always be invalid. Parsing `0` a good way to handle timer overcapture errors.