#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "alloc",not(feature = "std")))]
extern crate alloc;

use core::fmt;

mod foreign;

pub type SerResult<T> = Result<T, SerError>;

/// An error returned by [`SerWrite`] and serializers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SerError {
    /// Buffer is full
    BufferFull,
    /// Undetermined map length or too many items
    MapLength,
    /// Undetermined sequence length or too many items
    SeqLength,
    /// Serializer could not determine string size
    StrLength,
    /// Not covered by the above cases
    OtherError,
}

impl serde::de::StdError for SerError {}

impl fmt::Display for SerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerError::BufferFull => f.write_str("buffer is full"),
            SerError::MapLength => f.write_str("unknown or invalid map length"),
            SerError::SeqLength => f.write_str("unknown or invalid sequence length"),
            SerError::StrLength => f.write_str("invalid string length"),
            SerError::OtherError => f.write_str("other serialize error")
        }
    }
}

impl serde::ser::Error for SerError {
    fn custom<T>(_msg: T) -> Self
        where T: fmt::Display
    {
        unreachable!()
    }
}

/// Serializers should write data to the implementations of this trait.
pub trait SerWrite {
    /// Write all bytes from `buf` to the internal buffer.
    ///
    /// When over capacity return `Err(SerError::BufferFull)`.
    fn write(&mut self, buf: &[u8]) -> SerResult<()>;
    /// Write a single `byte` to the internal buffer.
    ///
    /// When over capacity return `Err(SerError::BufferFull)`.
    #[inline]
    fn write_byte(&mut self, byte: u8) -> SerResult<()> {
        self.write(core::slice::from_ref(&byte))
    }
    /// Write a string to the internal buffer.
    ///
    /// When over capacity return `Err(SerError::BufferFull)`.
    #[inline]
    fn write_str(&mut self, s: &str) -> SerResult<()> {
        self.write(s.as_bytes())
    }
}

impl<T: SerWrite> SerWrite for &'_ mut T {
    fn write(&mut self, buf: &[u8]) -> SerResult<()> {
        (*self).write(buf)
    }
}

/// A simple slice writer (example implementation)
#[derive(Debug, PartialEq)]
pub struct SliceWriter<'a> {
    pub buf: &'a mut [u8],
    pub len: usize
}

impl<'a> SliceWriter<'a> {
    /// Create new instance
    pub fn new(buf: &'a mut [u8]) -> Self {
        SliceWriter { buf, len: 0 }
    }
    /// Return populated slice chunk
    pub fn view(&self) -> &[u8] {
        &self.buf[..self.len]
    }
    /// Return populated length
    pub fn len(&self) -> usize {
        self.len
    }
    /// Return total capacity
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }
    /// Return remaining capacity
    pub fn rem_capacity(&self) -> usize {
        self.buf.len() - self.len
    }
    /// Destruct into underlying buffer
    pub fn into_buf(self) -> &'a mut [u8] {
        self.buf
    }
}

impl SerWrite for SliceWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> SerResult<()> {
        let end = self.len + buf.len();
        match self.buf.get_mut(self.len..end) {
            Some(chunk) => {
                chunk.copy_from_slice(buf);
                self.len = end;
                Ok(())
            }
            None => Err(SerError::BufferFull)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_writer() {
        let mut buf = [0u8;22];
        let mut writer = SliceWriter::new(&mut buf[..]);
        writer.write(b"Hello World!").unwrap();
        writer.write_byte(b' ').unwrap();
        writer.write_str("Good Bye!").unwrap();
        let expected = b"Hello World! Good Bye!";
        assert_eq!(writer.view(), expected);
        assert_eq!(writer.write_byte(b' ').unwrap_err(), SerError::BufferFull);
    }
}
