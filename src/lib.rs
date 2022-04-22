#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![feature(assert_matches)]
#![feature(doc_cfg)]

//! # Driver for MICS-VZ-89TE sensor
//!
//! This driver can be used to read CO2 and voc measurements of the MICS-VZ-89TE sensor.
//! CO2 is measured in ppm in range from 400 to 2000. VOC is measured in ppb in range from 0 to 1000.
//! At startup the sensor needs around 15 minutes to deliver a valid CO2 value, as noted in the datasheet.
//!
//! To use this driver, the I2C bus has to be set to a max baudrate of 100_000.
//!
//! ## Feature flags
//!
//! - `std`: Enables error handling with `std::error::Error`.
//! - `chrono`: Enables compatibility with `chrono::NaiveDate` on struct `RevisionDate`.
//! - `unproven`: Enables ppm calibration and r0 value retrieving. (Correct functionality couldn't be verified.)
//!
//! # Example Usage
//! ```ignore
//! let mut delay = ...; // delay struct from board
//! let i2c = ...; // I2C bus to use
//!
//! let mut device = MicsVz89Te::new(i2c);
//! let measurements = device.read_measurements(&mut delay).unwrap();
//!
//! let co2 = measurements.co2;
//! let voc = measurements.voc;
//! ```

pub mod error;

use embedded_hal::blocking::{
    delay::DelayMs,
    i2c::{Read, Write},
};
use error::PacketParseError;

const MICS_VZ_89TE_ADDR: u8 = 0x70;

const MICS_VZ_89TE_ADDR_CMD_GETSTATUS: u8 = 0x0C;
const MICS_VZ_89TE_DATE_CODE: u8 = 0x0D;
#[cfg(any(feature = "unproven", test))]
const MICS_VZ_89TE_GET_CALIBR_VAL: u8 = 0x10;
#[cfg(any(feature = "unproven", test))]
const MICS_VZ_89TE_SET_CALIBR_PPM: u8 = 0x08;

/// Represents the date of revision of the sensor.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RevisionDate {
    year: u8,
    pub month: u8,
    pub day: u8,
}

impl RevisionDate {
    /// Access the year of revision.
    pub fn year(&self) -> u16 {
        self.year as u16 + 2000
    }
}

impl core::fmt::Debug for RevisionDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevisionDate")
            .field("year", &((self.year as u16) + 2000))
            .field("month", &self.month)
            .field("day", &self.day)
            .finish()
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDate> for RevisionDate {
    fn from(d: chrono::NaiveDate) -> Self {
        use chrono::Datelike;
        let year = d.year() - 2000;
        let year = if year < 0 { 0 } else { year as u8 };
        RevisionDate {
            year: year,
            month: d.month() as u8,
            day: d.day() as u8,
        }
    }
}

/// Returned measurements by the sensor
#[derive(Debug, Clone, Copy)]
pub struct Measurements {
    pub co2: f32,
    pub voc: f32,
}

/// Driver for MICS-VZ-89TE sensor
pub struct MicsVz89Te<I2C> {
    i2c: I2C,
}

impl<I2C, E> MicsVz89Te<I2C>
where
    I2C: Read<Error = E> + Write<Error = E>,
{
    /// Create new driver on the supplied i2c bus.
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    /// Read measurements from sensor.
    pub fn read_measurements(
        &mut self,
        delay: &mut impl DelayMs<u16>,
    ) -> Result<Measurements, PacketParseError<E>> {
        let response =
            self.request_data(&[MICS_VZ_89TE_ADDR_CMD_GETSTATUS, 0, 0, 0, 0, 0xF3], delay)?;

        let co2 = (response[1] - 13) as f32 * (1600.0 / 229.0) + 400.0; // ppm: 400 .. 2000
        let voc = (response[0] - 13) as f32 * (1000.0 / 229.0); // ppb: 0 .. 1000

        Ok(Measurements { co2, voc })
    }

    /// Read revision date of the sensor.
    pub fn read_revision(
        &mut self,
        delay: &mut impl DelayMs<u16>,
    ) -> Result<RevisionDate, PacketParseError<E>> {
        let response = self.request_data(&[MICS_VZ_89TE_DATE_CODE, 0, 0, 0, 0, 0xF2], delay)?;
        let date = RevisionDate {
            year: response[0],
            month: response[1],
            day: response[2],
        };
        Ok(date)
    }

    #[cfg(any(feature = "unproven", doc, test))]
    #[doc(cfg(feature = "unproven"))]
    /// Read the calibration value R0 of the sensor in kOhms.
    pub fn read_calibration_r0(
        &mut self,
        delay: &mut impl DelayMs<u16>,
    ) -> Result<u16, PacketParseError<E>> {
        let response =
            self.request_data(&[MICS_VZ_89TE_GET_CALIBR_VAL, 0, 0, 0, 0, 0xEF], delay)?;
        let value = (response[1] as u16) << 8 | response[0] as u16;
        Ok(value)
    }

    #[cfg(any(feature = "unproven", doc, test))]
    #[doc(cfg(feature = "unproven"))]
    /// Writes the calibration CO2 value in ppm in range from 400 to 2000 measured by another device.
    pub fn write_calibration_ppm(&mut self, ppm: f32) -> Result<(), PacketParseError<E>> {
        debug_assert!(
            ppm > 400.0 && ppm < 2000.0,
            "ppm must be in range from 400 to 2000"
        );
        let send_ppm = ((ppm - 400.0) / (1600.0 / 229.0) + 13.0) as u8;
        let mut cmd_array = [MICS_VZ_89TE_SET_CALIBR_PPM, send_ppm, 0, 0, 0, 0];
        cmd_array[5] = gen_checksum(&cmd_array[..5]);
        self.i2c
            .write(MICS_VZ_89TE_ADDR, &cmd_array)
            .map_err(PacketParseError::from)?;
        Ok(())
    }

    fn request_data(
        &mut self,
        cmd_buffer: &[u8],
        delay: &mut impl DelayMs<u16>,
    ) -> Result<[u8; 7], PacketParseError<E>> {
        debug_assert_eq!(cmd_buffer.len(), 6);
        self.i2c
            .write(MICS_VZ_89TE_ADDR, cmd_buffer)
            .map_err(PacketParseError::from)?;
        delay.delay_ms(100);

        let mut buffer = [0u8; 7];
        self.i2c.read(MICS_VZ_89TE_ADDR, &mut buffer)?;

        let check = gen_checksum(&buffer[..5]);
        if buffer[6].ne(&check) {
            return Err(PacketParseError::WrongChecksum);
        }

        Ok(buffer)
    }
}

impl<I2C> MicsVz89Te<I2C> {
    /// Releases the underlying I2C bus and destroys the driver.
    ///
    /// # Example Usage
    /// ```ignore
    /// let i2c = ...; // I2C bus to use
    /// let driver = MicsVz89Te::new(i2c);
    /// ...; // read measurements from sensor
    /// let i2c = driver.release();
    /// ```
    pub fn release(self) -> I2C {
        self.i2c
    }
}

fn gen_checksum(byte_array: &[u8]) -> u8 {
    let sum = byte_array.iter().fold(0u16, |a, v| a + (*v as u16));
    0xFF - (sum as u8 + (sum / 0x0100) as u8)
}

#[cfg(test)]
mod test {

    use crate::RevisionDate;

    use super::{MicsVz89Te, PacketParseError};
    use core::assert_eq;
    use embedded_hal_mock::{
        delay::MockNoop as DelayMock,
        i2c::{Mock as I2cMock, Transaction as I2cTransaction},
    };
    use std::{assert_matches::assert_matches, vec};

    #[test]
    fn test_read_measurements() {
        let expectations = [
            I2cTransaction::write(0x70, vec![0x0C, 0, 0, 0, 0, 0xF3]),
            I2cTransaction::read(0x70, vec![0x27, 0x3C, 0, 0xBA, 0xBA, 0, 0x27]),
        ];
        let i2c = I2cMock::new(&expectations);
        let mut delay = DelayMock::new();

        let mut device = MicsVz89Te::new(i2c);
        let measurements = device.read_measurements(&mut delay);

        assert_matches!(measurements, Ok(_));
        let measurements = measurements.unwrap();

        assert_eq!(measurements.co2 as u32, 728);
        assert_eq!(measurements.voc as u32, 113);
    }

    #[test]
    fn test_read_measurements_wrong_checksum() {
        let expectations = [
            I2cTransaction::write(0x70, vec![0x0C, 0, 0, 0, 0, 0xF3]),
            I2cTransaction::read(0x70, vec![0x27, 0x3C, 0, 0xBA, 0xBA, 0, 0x26]),
        ];
        let i2c = I2cMock::new(&expectations);
        let mut delay = DelayMock::new();

        let mut device = MicsVz89Te::new(i2c);
        let measurements = device.read_measurements(&mut delay);

        assert_matches!(measurements, Err(PacketParseError::WrongChecksum));
    }

    #[test]
    fn test_read_revision_date() {
        let expectations = [
            I2cTransaction::write(0x70, vec![0x0D, 0, 0, 0, 0, 0xF2]),
            I2cTransaction::read(0x70, vec![0x10, 0x03, 0x11, 0x48, 00, 0, 0x93]),
        ];
        let i2c = I2cMock::new(&expectations);
        let mut delay = DelayMock::new();

        let mut device = MicsVz89Te::new(i2c);
        let revision = device.read_revision(&mut delay);

        assert_matches!(revision, Ok(_));
        let revision = revision.unwrap();

        assert_eq!(
            revision,
            RevisionDate {
                year: 16,
                month: 3,
                day: 17
            }
        );
    }

    #[test]
    fn test_write_calibration_ppm() {
        let expectations = [I2cTransaction::write(0x70, vec![0x08, 0x62, 0, 0, 0, 0x95])];
        let i2c = I2cMock::new(&expectations);

        let mut device = MicsVz89Te::new(i2c);
        let res = device.write_calibration_ppm(1000.0);

        assert_matches!(res, Ok(_));
    }

    #[test]
    fn test_read_calibration_r0() {
        let expectations = [
            I2cTransaction::write(0x70, vec![0x10, 0, 0, 0, 0, 0xEF]),
            I2cTransaction::read(0x70, vec![0xFB, 0x01, 0, 0, 0, 0, 0x03]),
        ];
        let i2c = I2cMock::new(&expectations);
        let mut delay = DelayMock::new();

        let mut device = MicsVz89Te::new(i2c);
        let value = device.read_calibration_r0(&mut delay);

        assert_matches!(value, Ok(_));
        let value = value.unwrap();

        assert_eq!(value, 507);
    }
}
