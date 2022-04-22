/// Represents errors which can occur while communicating with the sensor.
#[cfg_attr(feature = "std", derive(std::fmt::Debug))]
#[repr(u8)]
pub enum PacketParseError<E> {
    BusError(E),
    WrongChecksum,
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<E> std::error::Error for PacketParseError<E> where E: std::error::Error {}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<E> std::fmt::Display for PacketParseError<E>
where
    E: std::error::Error,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PacketParseError: {}",
            match self {
                Self::BusError(e) => format!("BusError: {:?}", e),
                Self::WrongChecksum => String::from("WrongChecksum"),
            }
        )
    }
}

#[cfg(not(feature = "std"))]
impl<E: core::fmt::Debug> core::fmt::Debug for PacketParseError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "PacketParseError::{}",
            match self {
                Self::BusError(_) => "BusError",
                Self::WrongChecksum => "WrongChecksum",
            }
        )
    }
}

impl<E> From<E> for PacketParseError<E> {
    fn from(e: E) -> Self {
        Self::BusError(e)
    }
}
