#![no_std]

//! # Driver for MICS-VZ-89TE sensor
//!
//! This driver can be used to read co2 and voc measurements of the MICS-VZ-89TE sensor. Co2 is measured in ppm in the range from 400 to 2000.
//! Voc is measured in ppb in the range from 0 to 1000. At startup the sensor needs around 15 minutes to deliver a co2 value, noted in the datasheet.
//!
//! To use this driver, the i2c bus has to be set to a max baudrate of 100_000.
//!
//! # Example Usage
//! ```
//! let driver = MicsVz89Te::new(i2c);
//!
//! ```
//!

use embedded_hal::blocking::{
    delay::DelayMs,
    i2c::{Read, Write},
};

static MICS_VZ_89TE_ADDR: u8 = 0x70; //0x70 default I2C address

static MICS_VZ_89TE_ADDR_CMD_GETSTATUS: u8 = 0x0C; // This command is used to read the VZ89 status coded with 6 bytes:
#[allow(dead_code)]
static MICS_VZ_89TE_DATE_CODE: u8 = 0x0D;

#[derive(Debug, Clone, Copy)]
pub struct Measurements {
    pub co2: f32,
    pub voc: f32,
}

/// Baudrate must be set to max 100_000 Hz
pub struct MicsVz89Te<I2C> {
    i2c: I2C,
    buffer: [u8; 7],
}

impl<I2C, E> MicsVz89Te<I2C>
where
    I2C: Read<Error = E> + Write<Error = E>,
{
    pub fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            buffer: [0u8; 7],
        }
    }

    pub fn read_status(&mut self, delay: &mut impl DelayMs<u16>) -> Result<Measurements, E> {
        self.i2c.write(
            MICS_VZ_89TE_ADDR,
            &[MICS_VZ_89TE_ADDR_CMD_GETSTATUS, 0, 0, 0, 0, 0xF3],
        )?;
        delay.delay_ms(100);
        self.i2c.read(MICS_VZ_89TE_ADDR, &mut self.buffer)?;

        let co2 = (self.buffer[1] - 13) as f32 * (1600.0 / 229.0) + 400.0; // ppm: 400 .. 2000
        let voc = (self.buffer[0] - 13) as f32 * (1000.0 / 229.0); // ppb: 0 .. 1000

        Ok(Measurements { co2, voc })
    }
}

#[allow(dead_code)]
fn gen_checksum(byte_array: &[u8]) -> u8 {
    let sum = byte_array.iter().fold(0u16, |a, v| a + (*v as u16));
    0xFF - (sum as u8 + (sum / 0x0100) as u8)
}
