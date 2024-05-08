/// Implementation for foreign types
#[cfg(feature = "std")]
use std::{vec::Vec, collections::VecDeque, io::Cursor};
#[cfg(all(feature = "alloc",not(feature = "std")))]
use alloc::{vec::Vec, collections::VecDeque};

#[allow(unused_imports)]
use super::*;

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl SerWrite for Vec<u8> {
    type Error = SerError;

    #[inline]
    fn write(&mut self, buf: &[u8]) -> SerResult<()> {
        self.extend_from_slice(buf);
        Ok(())
    }
    #[inline]
    fn write_byte(&mut self, byte: u8) -> SerResult<()> {
        self.push(byte);
        Ok(())
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl SerWrite for VecDeque<u8> {
    type Error = SerError;

    #[inline]
    fn write(&mut self, buf: &[u8]) -> SerResult<()> {
        self.extend(buf.into_iter().copied());
        Ok(())
    }
    #[inline]
    fn write_byte(&mut self, byte: u8) -> SerResult<()> {
        self.push_back(byte);
        Ok(())
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T> SerWrite for Cursor<T>
    where Cursor<T>: std::io::Write
{
    type Error = SerError;

    #[inline]
    fn write(&mut self, buf: &[u8]) -> SerResult<()> {
        std::io::Write::write_all(self, buf).map_err(|_| SerError::BufferFull)
    }
}

#[cfg(feature = "arrayvec")]
#[cfg_attr(docsrs, doc(cfg(feature = "arrayvec")))]
impl<const CAP: usize> SerWrite for arrayvec::ArrayVec<u8, CAP> {
    type Error = SerError;

    fn write(&mut self, buf: &[u8]) -> SerResult<()> {
        self.try_extend_from_slice(buf).map_err(|_| SerError::BufferFull)
    }
    #[inline]
    fn write_byte(&mut self, byte: u8) -> SerResult<()> {
        self.try_push(byte).map_err(|_| SerError::BufferFull)
    }
}

#[cfg(feature = "heapless")]
#[cfg_attr(docsrs, doc(cfg(feature = "heapless")))]
impl<const CAP: usize> SerWrite for heapless::Vec<u8, CAP> {
    type Error = SerError;

    fn write(&mut self, buf: &[u8]) -> SerResult<()> {
        self.extend_from_slice(buf).map_err(|_| SerError::BufferFull)
    }
    #[inline]
    fn write_byte(&mut self, byte: u8) -> SerResult<()> {
        self.push(byte).map_err(|_| SerError::BufferFull)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_ser_write_vec() {
        let mut writer = Vec::new();
        writer.write(b"Hello World!").unwrap();
        writer.write_byte(b' ').unwrap();
        writer.write_str("Good Bye!").unwrap();
        let expected = b"Hello World! Good Bye!";
        assert_eq!(&writer, expected);
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_ser_write_vec_deque() {
        let mut writer = VecDeque::new();
        writer.write(b"Hello World!").unwrap();
        writer.write_byte(b' ').unwrap();
        writer.write_str("Good Bye!").unwrap();
        let expected = b"Hello World! Good Bye!";
        assert_eq!(&writer, expected);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_ser_write_cursor() {
        let mut writer = Cursor::new([0u8;22]);
        writer.write(b"Hello World!").unwrap();
        writer.write_byte(b' ').unwrap();
        writer.write_str("Good Bye!").unwrap();
        let expected = b"Hello World! Good Bye!";
        assert_eq!(writer.get_ref(), expected);
        assert_eq!(writer.write_byte(b' ').unwrap_err(), SerError::BufferFull);
    }

    #[cfg(feature = "arrayvec")]
    #[test]
    fn test_ser_write_arrayvec() {
        let mut writer = arrayvec::ArrayVec::<u8,22>::new();
        writer.write(b"Hello World!").unwrap();
        writer.write_byte(b' ').unwrap();
        writer.write_str("Good Bye!").unwrap();
        let expected = b"Hello World! Good Bye!";
        assert_eq!(writer.as_slice(), expected);
        assert_eq!(writer.write_byte(b' ').unwrap_err(), SerError::BufferFull);
    }

    #[cfg(feature = "heapless")]
    #[test]
    fn test_ser_write_heapless() {
        let mut writer = heapless::Vec::<u8,22>::new();
        writer.write(b"Hello World!").unwrap();
        writer.write_byte(b' ').unwrap();
        writer.write_str("Good Bye!").unwrap();
        let expected = b"Hello World! Good Bye!";
        assert_eq!(writer.as_slice(), expected);
        assert_eq!(writer.write_byte(b' ').unwrap_err(), SerError::BufferFull);
    }
}
