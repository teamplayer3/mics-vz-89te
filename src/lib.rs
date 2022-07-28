#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

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
//! - `time`: Enables compatibility with `time::Date` on struct `RevisionDate`.
//! - `unproven`: Enables ppm calibration and r0 value retrieving.
//! (Correct functionality couldn't be verified.)
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
//!
//! let i2c = device.release(); // destruct driver to use bus with other drivers
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

#[cfg(feature = "time")]
impl TryFrom<time::Date> for RevisionDate {
    type Error = time::Error;

    fn try_from(d: time::Date) -> Result<Self, Self::Error> {
        Ok(RevisionDate {
            year: u16::try_from(d.year())
                .map_err(|_| time::Error::ConversionRange(time::error::ConversionRange))?,
            month: u8::from(d.month()),
            day: u8::from(d.day()),
        })
    }
}

#[cfg(feature = "time")]
impl TryFrom<RevisionDate> for time::Date {
    type Error = time::Error;

    fn try_from(rd: RevisionDate) -> Result<Self, Self::Error> {
        time::Date::from_calendar_date(i32::from(rd.year), time::Month::try_from(rd.month)?, rd.day)
            .map_err(|e| time::Error::ComponentRange(e))
    }
}

/// Returned measurements by the sensor
#[derive(Debug, Clone, Copy)]
pub struct Measurements {
    pub co2: f32,
    pub voc: f32,
}

impl Measurements {
    fn from_response(response: &[u8; 7]) -> Self {
        let co2 = f32::from(response[1].saturating_sub(13)) * (1600.0 / 229.0) + 400.0; // ppm: 400 .. 2000
        let voc = f32::from(response[0].saturating_sub(13)) * (1000.0 / 229.0); // ppb: 0 .. 1000
        Self { co2, voc }
    }
}

/// Driver for MICS-VZ-89TE sensor
pub struct MicsVz89Te<I2C> {
    i2c: I2C,
}

impl<I2C, E> MicsVz89Te<I2C>
where
    I2C: Read<Error = E> + Write<Error = E>,
{
    /// Time (in millis) to wait until the sensor response should be valid.
    pub const WAIT_ON_RESPONSE_TIME: u16 = 100;

    /// Create new driver on the supplied i2c bus.
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    /// Read measurements from sensor.
    ///
    /// This function blocks a minimum time of [MicsVz89Te::WAIT_ON_RESPONSE_TIME].
    pub fn read_measurements(
        &mut self,
        delay: &mut impl DelayMs<u16>,
    ) -> Result<Measurements, PacketParseError<E>> {
        let response =
            self.request_data(&[MICS_VZ_89TE_ADDR_CMD_GETSTATUS, 0, 0, 0, 0, 0xF3], delay)?;
        Ok(Measurements::from_response(&response))
    }

    /// This function starts a measurement request and can be used in context where the delay on a response
    /// has an specific implementation. For example in an async/await manner.
    ///
    /// To get a valid measurement result, a delay of [MicsVz89Te::WAIT_ON_RESPONSE_TIME] milliseconds should be implemented,
    /// after calling this function.
    ///
    /// # Example Usage
    /// implementation with [smol Timer](https://docs.rs/smol/latest/smol/struct.Timer.html)
    /// ```ignore
    /// driver.start_measurement().unwrap();
    /// Timer::after(Duration::from_millis(u64::from(MicsVz89Te::WAIT_ON_RESPONSE_TIME))).await;
    /// let measurements = driver.get_measurement_result().unwrap();
    /// ```
    pub fn start_measurement(&mut self) -> Result<(), PacketParseError<E>> {
        self.send_request(&[MICS_VZ_89TE_ADDR_CMD_GETSTATUS, 0, 0, 0, 0, 0xF3])
    }

    /// Get the before requested measurements. To see an example, see [MicsVz89Te::start_measurement()].
    pub fn get_measurement_result(&mut self) -> Result<Measurements, PacketParseError<E>> {
        let response = self.receive_response()?;
        Ok(Measurements::from_response(&response))
    }

    /// Read revision date of the sensor.
    ///
    /// This function blocks a minimum time of [MicsVz89Te::WAIT_ON_RESPONSE_TIME].
    pub fn read_revision(
        &mut self,
        delay: &mut impl DelayMs<u16>,
    ) -> Result<RevisionDate, PacketParseError<E>> {
        let response = self.request_data(&[MICS_VZ_89TE_DATE_CODE, 0, 0, 0, 0, 0xF2], delay)?;
        let date = RevisionDate {
            year: u16::from(response[0]) + 2000,
            month: response[1],
            day: response[2],
        };
        Ok(date)
    }

    #[cfg(any(feature = "unproven", doc, test))]
    #[cfg_attr(docsrs, doc(cfg(feature = "unproven")))]
    /// Read the calibration value R0 of the sensor in kOhms.
    ///
    /// This function blocks a minimum time of [MicsVz89Te::WAIT_ON_RESPONSE_TIME].
    pub fn read_calibration_r0(
        &mut self,
        delay: &mut impl DelayMs<u16>,
    ) -> Result<u16, PacketParseError<E>> {
        let response =
            self.request_data(&[MICS_VZ_89TE_GET_CALIBR_VAL, 0, 0, 0, 0, 0xEF], delay)?;
        Ok(u16::from_le_bytes([response[0], response[1]]))
    }

    #[cfg(any(feature = "unproven", doc, test))]
    #[cfg_attr(docsrs, doc(cfg(feature = "unproven")))]
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
            .map_err(PacketParseError::from)
    }

    fn request_data(
        &mut self,
        cmd_buffer: &[u8; 6],
        delay: &mut impl DelayMs<u16>,
    ) -> Result<[u8; 7], PacketParseError<E>> {
        self.send_request(cmd_buffer)?;
        delay.delay_ms(Self::WAIT_ON_RESPONSE_TIME);
        self.receive_response()
    }

    fn send_request(&mut self, cmd_buffer: &[u8; 6]) -> Result<(), PacketParseError<E>> {
        self.i2c
            .write(MICS_VZ_89TE_ADDR, cmd_buffer)
            .map_err(PacketParseError::from)
    }

    fn receive_response(&mut self) -> Result<[u8; 7], PacketParseError<E>> {
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

    use crate::{error::PacketParseError, RevisionDate};

    use super::MicsVz89Te;
    use assert_matches::assert_matches;
    use core::assert_eq;
    use embedded_hal_mock::{
        delay::MockNoop as DelayMock,
        i2c::{Mock as I2cMock, Transaction as I2cTransaction},
    };
    use std::vec;

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

        assert!(measurements.is_ok());
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

        assert_matches!(
                revision, Ok(r) if r == RevisionDate { year: 2016, month: 3, day: 17 }
        );
    }

    #[test]
    fn test_write_calibration_ppm() {
        let expectations = [I2cTransaction::write(0x70, vec![0x08, 0x62, 0, 0, 0, 0x95])];
        let i2c = I2cMock::new(&expectations);

        let mut device = MicsVz89Te::new(i2c);
        let res = device.write_calibration_ppm(1000.0);

        assert!(res.is_ok());
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

        assert_matches!(value, Ok(v) if v == 507);
    }
}
