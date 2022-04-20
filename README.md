# MICS-VZ-89TE Driver

This driver can be used to read CO2 and VOC measurements of the MICS-VZ-89TE sensor.

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/teamplayer3/mics-vz-89te)

# Example usage

```rust
let mut delay = ...; // delay struct from board
let i2c = ...; // I2C bus to use

let mut device = MicsVz89Te::new(i2c);
let measurements = device.read_status(&mut delay).unwrap();

let co2 = measurements.co2;
let voc = measurements.voc;
```

## TODO

* Add function to retrieve date code.