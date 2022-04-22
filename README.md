# MICS-VZ-89TE Driver

This driver can be used to read CO2 and VOC measurements of the MICS-VZ-89TE sensor.

[![Build Status](https://github.com/teamplayer3/mics-vz-89te/workflows/Rust/badge.svg)](https://github.com/teamplayer3/mics-vz-89te/actions?query=workflow%3ARust)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/teamplayer3/mics-vz-89te)
[![Crates.io](https://img.shields.io/crates/v/mics-vz-89te.svg)](https://crates.io/crates/mics-vz-89te)
[![Documentation](https://docs.rs/mics-vz-89te/badge.svg)](https://docs.rs/mics-vz-89te)

# Example usage

Example shows how to read CO2 and VOC from sensor.

```rust
let mut delay = ...; // delay struct from board
let i2c = ...; // I2C bus to use

let mut device = MicsVz89Te::new(i2c);
let measurements = device.read_measurements(&mut delay).unwrap();

let co2 = measurements.co2;
let voc = measurements.voc;

let i2c = device.release(); // destruct driver to use bus with other drivers
```