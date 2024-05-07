#![allow(unused_imports)]
use core::cell::Cell;
use std::println;

use serde::forward_to_deserialize_any;
use serde::de::{SeqAccess, MapAccess, DeserializeSeed};
use core::ops::Neg;
use core::slice::{from_raw_parts, from_raw_parts_mut};
use core::str::{Utf8Error, FromStr};
use core::{fmt, str};

use serde::de::{self, Visitor};

/// Deserializes an instance of type `T` from a mutable slice of bytes of JSON text.
///
/// The provided slice must be writable so the deserializer can unescape strings 
/// and parse bytes from arrays or strings in-place.
///
/// __NOTE__: Assume the original slice content will be modified!
///
/// Any `&str` or `&[u8]` in the returned type will contain references to the provided slice.
pub fn from_mut_slice<'a, T>(v: &'a mut [u8]) -> Result<T>
    where T: de::Deserialize<'a>
{
    let mut de = Deserializer::from_mut_slice(v);
    let value = de::Deserialize::deserialize(&mut de)?;
    de.end()?;

    Ok(value)
}

/// Deserialization result
pub type Result<T> = core::result::Result<T, Error>;

pub struct Deserializer<'de> {
    input: &'de mut[u8],
    index: usize
}

impl From<Utf8Error> for Error {
    fn from(_err: Utf8Error) -> Self {
        Error::InvalidUnicodeCodePoint
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
pub enum Error {
    /// EOF while parsing
    UnexpectedEof,
    /// Invalid escape sequence
    InvalidEscapeSequence,
    /// A control ASCII character detected in a string
    StringControlChar,
    /// Expected this character to be a `':'`.
    ExpectedColon,
    /// Expected this character to be either a `','` or a `']'`.
    ExpectedArrayCommaOrEnd,
    /// Array content starts with a leading `,`.
    LeadingArrayComma,
    /// Array content ends with a trailing `,`.
    TrailingArrayComma,
    /// Expected this character to be either a `','` or a `'}'`.
    ExpectedObjectCommaOrEnd,
    /// Object content starts with a leading `,`.
    LeadingObjectComma,
    /// Object content ends with a trailing `,`.
    TrailingObjectComma,
    /// Expected to parse either a `true`, `false`, or a `null`.
    ExpectedToken,
    /// Expected `null`
    ExpectedNull,
    /// Expected `"` character
    ExpectedString,
    /// Expected ']'
    ExpectedArrayEnd,
    /// Expected array
    ExpectedArray,
    /// Expected object
    ExpectedObject,

    /// Expected this character to start a JSON value.
    ExpectedEnumValue,

    /// Expected this character to be `'}'`.
    ExpectedEnumObjectEnd,

    /// Invalid number
    InvalidNumber,

    /// Invalid type
    InvalidType,

    /// Invalid unicode code point
    InvalidUnicodeCodePoint,

    /// Object key is not a string.
    KeyMustBeAString,

    /// JSON has non-whitespace trailing characters after the value.
    TrailingCharacters,

    /// Unexpected character
    UnexpectedChar,

    /// Invalid length
    InvalidLength,

    /// Error with a custom message that we had to discard.
    CustomError
}

impl serde::de::StdError for Error {}

impl de::Error for Error {
    fn custom<T: fmt::Display>(_msg: T) -> Self {
        Error::CustomError
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::UnexpectedEof => "Unexpected end of JSON input",
            Error::InvalidEscapeSequence => "Invalid JSON string escape sequence",
            Error::StringControlChar => "A control ASCII character found in a JSON string",
            Error::ExpectedArrayCommaOrEnd => "Expected `','` or `']'`",
            Error::LeadingArrayComma => "JSON array content starts with a leading `,`",
            Error::TrailingArrayComma => "JSON array content ends with a trailing `,`",
            Error::ExpectedObjectCommaOrEnd => "Expected `','` or `'}'`",
            Error::LeadingObjectComma => "JSON object content starts with a leading `,`",
            Error::TrailingObjectComma => "JSON object content ends with a trailing `,`",
            Error::ExpectedColon => "Expected `':'`",
            Error::ExpectedToken => {
                "Expected either `true`, `false`, or `null`."
            }
            Error::ExpectedNull => "Expected `null`",
            Error::ExpectedString => r#"Expected `"`"#,
            Error::ExpectedArrayEnd => "Expected ']'",
            Error::ExpectedArray => "Expeced a JSON array",
            Error::ExpectedObject => "Expected a JSON object",
            Error::ExpectedEnumValue => "Expected this character to start a JSON value",
            Error::ExpectedEnumObjectEnd => "Expected this character to be `'}'`",
            Error::InvalidNumber => "Invalid number.",
            Error::InvalidType => "Invalid type",
            Error::InvalidUnicodeCodePoint => "Invalid unicode code point",
            Error::KeyMustBeAString => "Object key is not a string.",
            Error::TrailingCharacters => {
                "JSON has non-whitespace trailing characters after the value."
            }
            Error::UnexpectedChar => "Unexpected token while parsing a JSON value",
            Error::InvalidLength => "Invalid length",
            Error::CustomError => "JSON does not match deserializer’s expected format.",
            // _ => "Invalid JSON",
        })
    }
}

// special JSON characters
const SP: u8 = b' ';
const QU: u8 = b'"';
const RS: u8 = b'\\';
const SO: u8 = b'/';
// special JSON string escape characters
const B_: u8 = 0x08; const BB: u8 = b'b'; // \b -> \x08
const T_: u8 = 0x09; const TT: u8 = b't'; // \t -> \x09
const N_: u8 = 0x0A; // const NN: u8 = b'n'; // \n -> \x0A
const F_: u8 = 0x0C; // const FF: u8 = b'f'; // \f => \x0C
const R_: u8 = 0x0D; // const RR: u8 = b'r'; // \r => \x0D
// \uUUUU
const UU: u8 = b'u';
const __: u8 = 0;
// only selected (un)escape codes are permitted
static UNESCAPE: [u8;19] = [
/* \b,  c,  d,  e, \f,  g,  h,  i,  j,  k,  l,  m, \n,  o,  p,  q, \r,  s, \t */
    B_, __, __, __, F_, __, __, __, __, __, __, __, N_, __, __, __, R_, __, T_
];

#[inline(always)]
fn parse_hex_nib(ch: u8) -> Option<u8> {
    match ch {
        n@b'0'..=b'9' => Some(n - b'0'),
        _ => match ch|0x20 {
            n@b'a'..=b'f' => Some(n - b'a' + 10),
            _ => None
        }
    }
}

#[inline(always)]
fn parse_uuuu([a,b,c,d]: [u8;4]) -> Option<u32> {
    Some(u16::from_le_bytes([
        (parse_hex_nib(c)? << 4) + parse_hex_nib(d)?,
        (parse_hex_nib(a)? << 4) + parse_hex_nib(b)?]).into())
}

/// Helper trait for parsing integers
trait NumParseTool: Sized + Copy {
    const ZERO: Self;
    fn try_from_ascii_decimal(code: u8) -> Option<Self>;
    fn checked_mul_ten(self) -> Result<Self>;
    fn checked_add(self, rhs: Self) -> Result<Self>;
}

/// Helper trait for parsing negative integers
trait CheckedSub: Sized + Copy {
    fn checked_sub(self, rhs: Self) -> Result<Self>;
}

macro_rules! impl_parse_tool {
    ($($ty:ty),*) => {$(
        impl NumParseTool for $ty {
            const ZERO: Self = 0;
            #[inline(always)]
            fn try_from_ascii_decimal(code: u8) -> Option<Self> {
                if matches!(code, b'0'..=b'9') {
                    Some((code - b'0') as Self)
                }
                else {
                    None
                }
            }
            #[inline(always)]
            fn checked_mul_ten(self) -> Result<Self> {
                self.checked_mul(10)
                .ok_or_else(|| Error::InvalidNumber)
            }
            #[inline(always)]
            fn checked_add(self, rhs: Self) -> Result<Self> {
                self.checked_add(rhs)
                .ok_or_else(|| Error::InvalidNumber)
            }
        }
    )*};
}

macro_rules! impl_checked_sub {
    ($($ty:ty),*) => {$(
        impl CheckedSub for $ty {
            #[inline(always)]
            fn checked_sub(self, rhs: Self) -> Result<Self> {
                self.checked_sub(rhs)
                .ok_or_else(|| Error::InvalidNumber)
            }
        }
    )*};
}

impl_parse_tool!(u8, u16, u32, u64, i8, i16, i32, i64);
impl_checked_sub!(i8, i16, i32, i64);

impl<'de> Deserializer<'de> {
    /// Provide a mutable slice, so strings can be unescaped in-place
    pub fn from_mut_slice(input: &'de mut[u8]) -> Self {
        Deserializer { input, index: 0 }
    }

    /// Return the next ASCII code
    fn peek(&self) -> Result<u8> {
        self.input.get(self.index).copied()
        .ok_or_else(|| Error::UnexpectedEof)
    }

    /// Eats len characters
    fn eat_some(&mut self, len: usize) {
        self.index += len;
    }

    /// Consume deserializer and check if trailing characters only consist of whitespace
    fn end(mut self) -> Result<()> {
        // println!("end: {}", core::str::from_utf8(&self.input[self.index..]).unwrap());
        self.eat_whitespace().err()
        .map(|_| ())
        .ok_or_else(|| Error::TrailingCharacters)
    }

    /// Eats all the whitespace characters and returns a peek into the next character
    fn eat_whitespace(&mut self) -> Result<u8> {
        let index = self.index;
        self.input[index..].iter()
        .position(|&b| !matches!(b, SP|T_|N_|R_))
        .map(|pos| {
            self.index = index + pos;
            self.input[index + pos]
        })
        .ok_or_else(|| Error::UnexpectedEof)
    }

    /// Splits the input slice at `index + offs` to uphold the mutability borrow contract
    /// and returns the slice between `self.index..index`
    fn split_some(&mut self, index: usize, offs: usize) -> &'de[u8] {
        let len = self.input.len();
        let ptr = self.input.as_mut_ptr();
        let nstart = index + offs;
        let newlen = (len).checked_sub(nstart).unwrap();
        let index0 = self.index;
        let reslen = (index).checked_sub(index0).unwrap();
        self.index = 0;
        // SAFETY: We just checked that `[index0..index]` and `[nstart; newlen]`
        // are not overlapping, because we checked that index0 <= index and nstart = index + offs,
        // so returning a reference is fine.
        // unfortunately we can't use slice::split_at_mut because the returned lifetime
        // have to be preserved
        unsafe {
             self.input = from_raw_parts_mut(ptr.add(nstart), newlen);
             from_raw_parts(ptr.add(index0), reslen)
        }
    }

    #[inline]
    fn parse_positive_number<T: NumParseTool>(&mut self, mut number: T) -> Result<T> {
        let mut pos = 0usize;
        for ch in self.input.get(self.index..)
                    .ok_or_else(|| Error::UnexpectedEof)?
                    .iter().copied()
        {
            match T::try_from_ascii_decimal(ch) {
                Some(n) => {
                    number = number
                        .checked_mul_ten()?
                        .checked_add(n)?
                }
                _ => break
            }
            pos += 1;
        }
        self.eat_some(pos);
        Ok(number)
    }

    #[inline]
    fn parse_negative_number<T: NumParseTool + CheckedSub>(&mut self, mut number: T) -> Result<T> {
        let mut pos = 0usize;
        for ch in self.input.get(self.index..)
                    .ok_or_else(|| Error::UnexpectedEof)?
                    .iter().copied()
        {
            match T::try_from_ascii_decimal(ch) {
                Some(n) => {
                    number = number
                        .checked_mul_ten()?
                        .checked_sub(n)?
                }
                _ => break
            }
            pos += 1;
        }
        self.eat_some(pos);
        Ok(number)
    }

    /// Eats whitespace and then parses a number as an unsigned int
    #[inline]
    fn parse_unsigned<T: NumParseTool>(&mut self) -> Result<T> {
        let peek = self
            .eat_whitespace()?;

        match peek {
            b'-' => Err(Error::InvalidNumber),
            b'0' => {
                self.eat_some(1);
                Ok(T::ZERO)
            }
            _ => if let Some(number) = T::try_from_ascii_decimal(peek) {
                self.eat_some(1);
                self.parse_positive_number(number)
            }
            else {
                Err(Error::InvalidType)
            }
        }
    }

    /// Eats whitespace and then parses a number as a signed int
    #[inline]
    fn parse_signed<T>(&mut self) -> Result<T>
        where T: NumParseTool + CheckedSub + Neg<Output = T>
    {
        let mut peek = self
            .eat_whitespace()?;
        let is_neg = if peek == b'-' {
            self.eat_some(1);
            peek = self.peek()?;
            true
        }
        else {
            false
        };

        match peek {
            b'0' => {
                self.eat_some(1);
                Ok(T::ZERO)
            }
            _ => if let Some(number) = T::try_from_ascii_decimal(peek) {
                self.eat_some(1);
                if is_neg {
                    self.parse_negative_number(number.neg())
                }
                else {
                    self.parse_positive_number(number)
                }
            }
            else {
                Err(Error::InvalidType)
            }
        }
    }

    /// Parses a token, e.g. b"null", b"true", b"false"
    fn parse_token_content(&mut self, token: &[u8]) -> Result<()> {
        let size = token.len();
        if let Some(slice) = self.input.get(self.index..self.index+size) {
            if slice == token {
                self.eat_some(size);
                Ok(())
            }
            else {
                Err(Error::ExpectedToken)
            }
        }
        else {
            Err(Error::UnexpectedEof)
        }
    }

    /// Return a position of a first non-number member character
    #[inline]
    fn match_float(&mut self) -> usize {
        let input = &self.input[self.index..];
        input.iter()
        .position(|&b| !matches!(b, b'0'..=b'9'|b'+'|b'-'|b'.'|b'e'|b'E'))
        .unwrap_or_else(|| input.len())
    }

    /// Eats whitespace and then ignores subsequent number characters
    #[inline]
    fn eat_number(&mut self) -> Result<()> {
        // println!("eat num: {}", core::str::from_utf8(&self.input[self.index..]).unwrap());
        if b'n' == self.eat_whitespace()? {
            self.eat_some(1);
            self.parse_token_content(b"ull")?;
        }
        else {
            let pos = self.match_float();
            self.eat_some(pos);
        }
        Ok(())
    }

    /// Eats whitespace and then parses a number as a float
    #[inline]
    fn parse_float<F: FromStr>(&mut self) -> Result<Option<F>> {
        if b'n' == self.eat_whitespace()? {
            self.eat_some(1);
            self.parse_token_content(b"ull")?;
            return Ok(None)
        }
        let pos = self.match_float();
        let input = &self.input[self.index..self.index + pos];
        // SAFETY: We already checked that it only contains ascii. This is only true if the
        // caller has guaranteed that `pattern` contains only ascii characters.
        let s = unsafe { str::from_utf8_unchecked(input) };
        let v = F::from_str(s).map_err(|_| Error::InvalidNumber)?;
        self.eat_some(input.len());
        Ok(Some(v))
    }

    /// Eats whitespace and checks if the next character is a colon
    fn parse_key_colon(&mut self) -> Result<()> {
        if b':' == self.eat_whitespace()? {
            self.eat_some(1);
            Ok(())
        } else {
            Err(Error::ExpectedColon)
        }
    }

    /// Eats content of a string ignoring escape codes except before '"'
    fn eat_str(&mut self) -> Result<()> {
        let mut start = self.index;
        loop {
            if let Some(found) = self.input.get(start..).and_then(|slice|
                slice.iter().position(|&b| b == QU || b <= 0x1F))
            {
                let end = start + found;
                // note: we ignore any invalid \ escape codes, but we check for control chars
                match self.input[end] {
                    QU => {
                        let count = self.input[start..end].iter().rev()
                            .position(|&b| b != RS)
                            .unwrap_or_else(|| end - start);
                        if count % 2 == 0 { /* even number of '\' */
                            // println!("`{}'", core::str::from_utf8(&self.input[start..end]).unwrap());
                            self.index = end + 1;
                            return Ok(())
                        }
                        /* odd number of '/', continue */
                        start = end + 1;
                    }
                    _ => {
                        break Err(Error::StringControlChar)
                    }
                }
            }
            else {
                break Err(Error::UnexpectedEof)
            }
        }
    }

    /// Parses a string, handles escape sequences using in-place copy, call after eating an opening '"'
    fn parse_str_content(&mut self) -> Result<&'de str> {
        let mut index = self.index;
        let mut dest = index;
        let mut start = index;
        loop {
            // "....{dest}<-{gap}->{index}{start}..{end}..."
            if let Some(found) = self.input.get(start..).and_then(|slice|
                // println!("slice: {:?} {}", slice, core::str::from_utf8(&self.input[start..]).unwrap());
                /* search for either '\', '"' or a control character */
                slice.iter().position(|&b| matches!(b, RS|QU) || b <= 0x1F))
            {
                let end = start + found;
                let gap = index - dest;
                if gap != 0 {
                    self.input.copy_within(index..end, dest);
                }
                match self.input[end] {
                    QU => { /* '"' found */
                        /* return as str and eat a gap with a closing '"' */
                        break Ok(core::str::from_utf8(self.split_some(end - gap, gap + 1))?)
                    }
                    RS => { /* '\' found */
                        dest += end - index;
                        index = end + 1;
                        match self.input.get(index).copied() {
                            Some(QU|RS|SO) => { /* preserve escaped */
                                start = index + 1;
                            }
                            Some(c@(BB..=TT)) => { /* control codes */
                                let unescaped = UNESCAPE[(c-BB) as usize];
                                if unescaped == 0 {
                                    break Err(Error::InvalidEscapeSequence)
                                }
                                self.input[dest] = unescaped;
                                dest += 1;
                                index += 1;
                                start = index;
                            }
                            Some(UU) => { /* u0000 */
                                // let s = core::str::from_utf8(&self.input[index+1..index+5])?;
                                // let code = u32::from_str_radix(s, 16)?;
                                let code = self.input.get(index+1..index+5).ok_or_else(|| Error::UnexpectedEof)?
                                           .try_into().unwrap();
                                let code = parse_uuuu(code).ok_or_else(|| Error::InvalidEscapeSequence)?;
                                let ch = char::from_u32(code).ok_or_else(|| Error::InvalidUnicodeCodePoint)?;
                                dest += ch.encode_utf8(&mut self.input[dest..]).len();
                                index += 5;
                                start = index;
                            }
                            Some(..) => break Err(Error::InvalidEscapeSequence),
                            None => break Err(Error::UnexpectedEof)
                        }
                    }
                    _ => {
                        break Err(Error::StringControlChar)
                    }
                }
            }
            else {
                break Err(Error::UnexpectedEof)
            }
        }
    }

    fn parse_hex_bytes_content(&mut self) -> Result<&'de[u8]> {
        let input = self.input.get_mut(self.index..).ok_or_else(|| Error::UnexpectedEof)?;
        let cells = Cell::from_mut(input).as_slice_of_cells();
        let mut src = cells.chunks_exact(2);
        let mut len = 0;
        let mut iter = src.by_ref().zip(cells.into_iter());
        while let Some(([a, b], t)) = iter.next() {
            if let Some(n) = parse_hex_nib(a.get()) {
                if let Some(m) = parse_hex_nib(b.get()) {
                    t.set((n << 4) + m);
                }
                else {
                    return Err(Error::UnexpectedChar)
                }
            }
            else if a.get() == QU {
                return Ok(self.split_some(self.index + len, len + 1))
            }
            else {
                return Err(Error::UnexpectedChar)
            }
            len = len + 1;
        }
        if let [c] = src.remainder() {
            if c.get() == QU {
                return Ok(self.split_some(self.index + len, len + 1))
            }
        }
        Err(Error::UnexpectedChar)
    }

    fn parse_array_bytes_content(&mut self) -> Result<&'de[u8]> {
        if b']' == self.eat_whitespace()? {
            return Ok(self.split_some(self.index, 1))
        }
        let start = self.index;
        let mut index = start;
        #[allow(unused_variables)]
        let input = {
            #[cfg(debug_assertions)]
            {
                ()
            }
            #[cfg(not(debug_assertions))]
            {
                self.input.as_mut_ptr()
            }
        };
        loop {
            let byte = self.parse_unsigned()?;
            #[cfg(debug_assertions)]
            {
                self.input[index] = byte;
            }
            #[cfg(not(debug_assertions))]
            {
                // SAFETY: depends on parse_unsigned to validate if there is enough room in input
                // any number in ASCII is >= byte
                unsafe { input.add(index).write(byte); }
            }
            index += 1;
            match self.eat_whitespace()? {
                b',' => self.eat_some(1),
                b']' => break,
                _ => return Err(Error::UnexpectedChar)
            }
        }
        let offs = self.index + 1 - index;
        self.index = start;
        Ok(self.split_some(index, offs))
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.eat_whitespace()? {
            b'n' => self.deserialize_unit(visitor),
            b't'|b'f' => self.deserialize_bool(visitor),
            b'"' => self.deserialize_str(visitor),
            b'0'..=b'9'|b'-' => self.deserialize_f64(visitor),
            b'[' => self.deserialize_seq(visitor),
            b'{' => self.deserialize_map(visitor),
            _ => Err(Error::UnexpectedChar),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let boolean = match self.eat_whitespace()? {
            b't' => {
                self.eat_some(1);
                self.parse_token_content(b"rue")?;
                true
            },
            b'f' => {
                self.eat_some(1);
                self.parse_token_content(b"alse")?;
                false
            },
            _ => return Err(Error::UnexpectedChar)
        };
        visitor.visit_bool(boolean)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_i8(self.parse_signed()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_i16(self.parse_signed()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_i32(self.parse_signed()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_i64(self.parse_signed()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_u8(self.parse_unsigned()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_u16(self.parse_unsigned()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_u32(self.parse_unsigned()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_u64(self.parse_unsigned()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_f32(self.parse_float()?.unwrap_or(f32::NAN))
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_f64(self.parse_float()?.unwrap_or(f64::NAN))
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        if b'"' == self.eat_whitespace()? {
            self.eat_some(1);
            let s = self.parse_str_content()?;
            if let Some(ch) = s.chars().next() {
                if ch.len_utf8() == s.len() {
                    return visitor.visit_char(ch)
                }
            }
            Err(Error::InvalidLength)
        }
        else {
            Err(Error::ExpectedString)
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        if b'"' == self.eat_whitespace()? {
            self.eat_some(1);
            visitor.visit_borrowed_str(self.parse_str_content()?)
        }
        else {
            Err(Error::ExpectedString)
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let bytes = match self.eat_whitespace()? {
            b'"' => {
                self.eat_some(1);
                self.parse_hex_bytes_content()?
            }
            b'[' => {
                self.eat_some(1);
                self.parse_array_bytes_content()?
            }
            _ => return Err(Error::UnexpectedChar)
        };
        visitor.visit_borrowed_bytes(bytes)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.eat_whitespace()? {
            b'n' => {
                self.eat_some(1);
                self.parse_token_content(b"ull")?;
                visitor.visit_none()
            },
            _ => visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.eat_whitespace()? {
            b'n' => {
                self.eat_some(1);
                self.parse_token_content(b"ull")?;
                visitor.visit_unit()
            },
            _ => Err(Error::ExpectedNull)
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_unit(visitor)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        if b'[' == self.eat_whitespace()? {
            self.eat_some(1);
            let value = visitor.visit_seq(CommaSeparated::new(self))?;
            if b']' == self.eat_whitespace()? {
                self.eat_some(1);
                Ok(value)
            } else {
                Err(Error::ExpectedArrayEnd)
            }
        } else {
            Err(Error::ExpectedArray)
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        if b'{' == self.eat_whitespace()? {
            self.eat_some(1);
            let value = visitor.visit_map(CommaSeparated::new(self))?;
            if b'}' == self.eat_whitespace()? {
                self.eat_some(1);
                Ok(value)
            } else {
                Err(Error::ExpectedObjectCommaOrEnd)
            }
        } else {
            Err(Error::ExpectedObject)
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.eat_whitespace()? {
            b'"' => visitor.visit_enum(UnitVariantAccess { de: self }),
            b'{' => {
                self.eat_some(1);
                let value = visitor.visit_enum(VariantAccess { de: self })?;
                if b'}' == self.eat_whitespace()? {
                    self.eat_some(1);
                    Ok(value)
                }
                else {
                    Err(Error::ExpectedEnumObjectEnd)
                }
            }
            _ => Err(Error::ExpectedEnumValue)
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.eat_whitespace()? {
            b'n' => self.deserialize_unit(visitor),
            b't'|b'f' => self.deserialize_bool(visitor),
            b'"' => {
                self.eat_some(1);
                self.eat_str()?;
                visitor.visit_unit()
            }
            b'0'..=b'9'|b'-' => {
                self.eat_number()?;
                visitor.visit_unit()
            }
            b'[' => self.deserialize_seq(visitor),
            b'{' => self.deserialize_map(visitor),
            _ => Err(Error::UnexpectedChar),
        }
    }
}

struct CommaSeparated<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    first: bool,
}

impl<'a, 'de> CommaSeparated<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        CommaSeparated {
            de,
            first: true,
        }
    }
}

// `SeqAccess` is provided to the `Visitor` to give it the ability to iterate
// through elements of the sequence.
impl<'de, 'a> SeqAccess<'de> for CommaSeparated<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
        where T: DeserializeSeed<'de>
    {
        match self.de.eat_whitespace()? {
            b']' => return Ok(None),
            b',' => if self.first {
                return Err(Error::LeadingArrayComma)
            }
            else {
                self.de.eat_some(1);
                if b']' == self.de.eat_whitespace()? {
                    return Err(Error::TrailingArrayComma);
                }
            }
            _ => if self.first {
                self.first = false;
            }
            else {
                return Err(Error::ExpectedArrayCommaOrEnd);
            }
        }
        seed.deserialize(&mut *self.de).map(Some)
    }
}

// `MapAccess` is provided to the `Visitor` to give it the ability to iterate
// through entries of the map.
impl<'a, 'de> MapAccess<'de> for CommaSeparated<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
        where K: DeserializeSeed<'de>
    {
        match self.de.eat_whitespace()? {
            b'}' => return Ok(None),
            b',' => if self.first {
                return Err(Error::LeadingObjectComma)
            }
            else {
                self.de.eat_some(1);
                if b'}' == self.de.eat_whitespace()? {
                    return Err(Error::TrailingObjectComma);
                }
            }
            _ => if self.first {
                self.first = false;
            }
            else {
                return Err(Error::ExpectedObjectCommaOrEnd);
            }
        }
        if self.de.peek()? == b'"' {
            seed.deserialize(MapKey { de: &mut *self.de }).map(Some)
        }
        else {
            Err(Error::KeyMustBeAString)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
        where V: DeserializeSeed<'de>
    {
        self.de.parse_key_colon()?;
        seed.deserialize(&mut *self.de)
    }
}

struct MapKey<'a, 'de> {
    de: &'a mut Deserializer<'de>
}

impl<'de, 'a> de::Deserializer<'de> for MapKey<'a, 'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>,
    {
        self.de.deserialize_str(visitor)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char string
        bytes byte_buf enum option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

struct UnitVariantAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> de::EnumAccess<'de> for UnitVariantAccess<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self)>
        where V: de::DeserializeSeed<'de>
    {
        let variant = seed.deserialize(&mut *self.de)?;
        Ok((variant, self))
    }
}

impl<'a, 'de> de::VariantAccess<'de> for UnitVariantAccess<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value>
        where T: de::DeserializeSeed<'de>
    {
        Err(Error::InvalidType)
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de>
    {
        Err(Error::InvalidType)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de>
    {
        Err(Error::InvalidType)
    }
}

struct VariantAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> de::EnumAccess<'de> for VariantAccess<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self)>
        where V: de::DeserializeSeed<'de>
    {
        let variant = seed.deserialize(&mut *self.de)?;
        self.de.parse_key_colon()?;
        Ok((variant, self))
    }
}

impl<'a, 'de> de::VariantAccess<'de> for VariantAccess<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(Error::InvalidType)
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
        where T: de::DeserializeSeed<'de>
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de>
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de>
    {
        de::Deserializer::deserialize_struct(self.de, "", fields, visitor)
    }
}

#[cfg(test)]
mod tests {
    use std::{vec, vec::Vec};
    use serde::Deserialize;
    use super::*;

    #[test]
    fn test_parse_str_content() {
        let mut test = [0;1];
        test.copy_from_slice(br#"""#);
        let mut deser = Deserializer::from_mut_slice(&mut test);
        assert_eq!(deser.parse_str_content().unwrap(), "");

        let mut test = [0;13];
        test.copy_from_slice(br#"Hello World!""#);
        let mut deser = Deserializer::from_mut_slice(&mut test);
        assert_eq!(deser.parse_str_content().unwrap(), "Hello World!");
        assert!(deser.input.is_empty());
        assert_eq!(deser.index, 0);

        let mut test = [0;46];
        test.copy_from_slice(br#"\u0020Hello\r\\ \b\nW\tor\fld\u007Fy\u0306!\"""#);
        let mut deser = Deserializer::from_mut_slice(&mut test);
        assert_eq!(deser.parse_str_content().unwrap(), " Hello\r\\ \x08\nW\tor\x0cld\x7fy̆!\"");
        assert!(deser.input.is_empty());
        assert_eq!(deser.index, 0);

        let mut test = [0;13];
        test.copy_from_slice(br#"Hello World!""#);
        let mut deser = Deserializer::from_mut_slice(&mut test);
        assert_eq!(deser.parse_str_content().unwrap(), "Hello World!");
        assert!(deser.input.is_empty());
        assert_eq!(deser.index, 0);

        let mut test = [0;2];
        test.copy_from_slice(b"\n\"");
        let mut deser = Deserializer::from_mut_slice(&mut test);
        assert_eq!(deser.parse_str_content(), Err(Error::StringControlChar));
    }

    #[test]
    fn test_deserializer() {
        let mut test = Vec::new();
        let s: &str = {
            test.clear();
            test.extend_from_slice(br#""Hello World!""#);
            from_mut_slice(&mut test).unwrap()
        };
        assert_eq!(s, "Hello World!");
        let s: &str = {
            test.clear();
            test.extend_from_slice(br#" "Hello\tWorld!\r\n" "#);
            from_mut_slice(&mut test).unwrap()
        };
        assert_eq!(s, "Hello\tWorld!\r\n");
        let tup: (i8, u32, i64, f32, f64) = {
            test.clear();
            test.extend_from_slice(br#" [ 0 , 4294967295, -9223372036854775808 ,3.14 , 1.2e+8 ] "#);
            from_mut_slice(&mut test).unwrap()
        };
        assert_eq!(tup, (0i8,4294967295u32,-9223372036854775808i64,3.14f32,1.2e+8));
        let ary: [&str;3] = {
            test.clear();
            test.extend_from_slice(br#" ["one\u0031", "\u0032two", "\u003333"] "#);
            from_mut_slice(&mut test).unwrap()
        };
        assert_eq!(ary, ["one1", "2two", "333"]);
    }

    #[test]
    fn test_de_bytes() {
        use serde::Serialize;

        let mut vec = Vec::new();
        vec.extend_from_slice(b"[]");
        let bytes: &[u8] = from_mut_slice(&mut vec).unwrap();
        assert_eq!(bytes, []);

        vec.clear(); vec.extend_from_slice(br#""""#);
        let bytes: &[u8] = from_mut_slice(&mut vec).unwrap();
        assert_eq!(bytes, []);

        vec.clear(); vec.extend_from_slice(b"[0]");
        let bytes: &[u8] = from_mut_slice(&mut vec).unwrap();
        assert_eq!(bytes, [0]);

        vec.clear(); vec.extend_from_slice(br#""FF""#);
        let bytes: &[u8] = from_mut_slice(&mut vec).unwrap();
        assert_eq!(bytes, [255]);

        vec.clear(); vec.extend_from_slice(b"");
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(br#"""#);
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(br#""0""#);
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(br#""ABC""#);
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(br#""Xy""#);
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(b"[");
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(b"[-1]");
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(b"[256]");
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(b"[,]");
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());
        vec.clear(); vec.extend_from_slice(b"[0,]");
        assert!(from_mut_slice::<&[u8]>(&mut vec).is_err());

        #[derive(Default, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        struct Test<'a> {
            #[serde(with = "serde_bytes", skip_serializing_if = "Option::is_none")]
            owned: Option<Vec<u8>>,
            #[serde(with = "serde_bytes", skip_serializing_if = "Option::is_none")]
            borrowed: Option<&'a[u8]>,
            #[serde(skip_serializing_if = "Option::is_none")]
            tail: Option<bool>,
        }

        vec.clear();
        let mut test = Test { owned: Some(vec![0,10,11,12,13,14,15,16,17,18,19,255]), ..Test::default() };
        let expected = br#"{"owned":[0,10,11,12,13,14,15,16,17,18,19,255]}"#;
        crate::to_writer(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);
        assert_eq!(from_mut_slice::<Test>(&mut vec).unwrap(), test);

        vec.clear();
        vec.extend_from_slice(br#" { "owned" : [  255, 127, 128, 0  ] } "#);
        assert_eq!(
            from_mut_slice::<Test>(&mut vec).unwrap(),
            Test { owned: Some(vec![255,127,128,0]), ..Test::default() }
        );

        vec.clear();
        test.tail = Some(false);
        let expected = br#"{"owned":"000A0B0C0D0E0F10111213FF","tail":false}"#;
        crate::to_writer_hex_bytes(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);
        assert_eq!(from_mut_slice::<Test>(&mut vec).unwrap(), test);

        vec.clear();
        vec.extend_from_slice(br#" { "tail" :true ,"owned": "DEADBACA9970" } "#);
        assert_eq!(
            from_mut_slice::<Test>(&mut vec).unwrap(),
            Test { tail: Some(true), owned: Some(vec![0xde,0xad,0xba,0xca,0x99,0x70]), ..Test::default() }
        );

        vec.clear();
        let mut test = Test { borrowed: Some(&[0,10,11,12,13,14,15,16,17,18,19,255]), ..Test::default() };
        let expected = br#"{"borrowed":[0,10,11,12,13,14,15,16,17,18,19,255]}"#;
        crate::to_writer(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);
        assert_eq!(from_mut_slice::<Test>(&mut vec).unwrap(), test);

        vec.clear();
        vec.extend_from_slice(br#" { "borrowed" : [  255, 127, 128, 0  ] ,"tail"  :false}"#);
        assert_eq!(
            from_mut_slice::<Test>(&mut vec).unwrap(),
            Test { borrowed: Some(&[255,127,128,0]), tail: Some(false), ..Test::default() }
        );

        vec.clear();
        vec.extend_from_slice(br#" { "borrowed" : "DEADBACA9970" ,"tail"  :null, "owned":null } "#);
        assert_eq!(
            from_mut_slice::<Test>(&mut vec).unwrap(),
            Test { borrowed: Some(&[0xde,0xad,0xba,0xca,0x99,0x70]), ..Test::default() }
        );

        vec.clear();
        test.tail = Some(true);
        let expected = br#"{"borrowed":"000A0B0C0D0E0F10111213FF","tail":true}"#;
        crate::to_writer_hex_bytes(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);
        assert_eq!(from_mut_slice::<Test>(&mut vec).unwrap(), test);

        vec.clear();
        vec.extend_from_slice(br#" { "borrowed": [  ] , "tail" :  false ,  "owned"   :  "" }  "#);
        assert_eq!(
            from_mut_slice::<Test>(&mut vec).unwrap(),
            Test { borrowed: Some(&[]), tail: Some(false), owned: Some(vec![]) }
        );

        vec.clear();
        vec.extend_from_slice(br#"{"tail":null,"owned":[],"borrowed":""}"#);
        assert_eq!(
            from_mut_slice::<Test>(&mut vec).unwrap(),
            Test { borrowed: Some(&[]), tail: None, owned: Some(vec![]) }
        );

        vec.clear();
        vec.extend_from_slice(br#" {   }  "#);
        assert_eq!(
            from_mut_slice::<Test>(&mut vec).unwrap(),
            Test::default()
        );
    }

    #[derive(Debug, Deserialize, PartialEq)]
    enum Type {
        #[serde(rename = "boolean")]
        Boolean,
        #[serde(rename = "number")]
        Number,
        #[serde(rename = "thing")]
        Thing,
    }

    fn from_str<T>(s: &str) -> Result<(T, usize)>
        where for<'a> T: de::Deserialize<'a>
    {
        let mut vec = Vec::with_capacity(s.len());
        vec.extend_from_slice(s.as_bytes());
        let res: T = from_mut_slice(&mut vec)?;
        Ok((res, s.len()))
    }

    fn from_bufstr<'a, T>(buf: &'a mut Vec<u8>, s: &str) -> Result<(T, usize)>
        where T: de::Deserialize<'a>
    {
        buf.clear();
        buf.extend_from_slice(s.as_bytes());
        let res: T = from_mut_slice(buf)?;
        Ok((res, s.len()))
    }

    #[test]
    fn test_de_array() {
        assert_eq!(from_str::<[i32; 0]>("[]"), Ok(([], 2)));
        assert_eq!(from_str("[0, 1, 2]"), Ok(([0, 1, 2], 9)));

        // errors
        assert_eq!(from_str::<[i32; 2]>("{}"), Err(Error::ExpectedArray));
        assert_eq!(from_str::<[i32; 2]>("[0, 1,]"), Err(Error::ExpectedArrayEnd));
        assert_eq!(from_str::<[i32; 3]>("[0, 1,]"), Err(Error::TrailingArrayComma));
        assert_eq!(from_str::<[i32; 2]>("[,]"), Err(Error::LeadingArrayComma));
        assert_eq!(from_str::<[i32; 2]>("[, 0]"), Err(Error::LeadingArrayComma));
    }

    #[test]
    fn test_de_bool() {
        assert_eq!(from_str("true"), Ok((true, 4)));
        assert_eq!(from_str(" true"), Ok((true, 5)));
        assert_eq!(from_str("true "), Ok((true, 5)));

        assert_eq!(from_str("false"), Ok((false, 5)));
        assert_eq!(from_str(" false"), Ok((false, 6)));
        assert_eq!(from_str("false "), Ok((false, 6)));

        // errors
        assert!(from_str::<bool>("true false").is_err());
        assert!(from_str::<bool>("tru").is_err());
    }

    #[test]
    fn test_de_floating_point() {
        assert_eq!(from_str("5.0"), Ok((5.0, 3)));
        assert_eq!(from_str("1"), Ok((1.0, 1)));
        assert_eq!(from_str("-999.9"), Ok((-999.9, 6)));
        assert_eq!(from_str("1e5"), Ok((1e5, 3)));
        let (f, len): (f32, _) = from_str("null").unwrap();
        assert_eq!(len, 4);
        assert!(f.is_nan());
        let (f, len): (f64, _) = from_str("null").unwrap();
        assert_eq!(len, 4);
        assert!(f.is_nan());
        assert!(from_str::<f32>("a").is_err());
        assert!(from_str::<f64>(",").is_err());
    }

    #[test]
    fn test_de_integer() {
        assert_eq!(from_str("5"), Ok((5, 1)));
        assert_eq!(from_str("101"), Ok((101u8, 3)));
        assert_eq!(from_str("101"), Ok((101u16, 3)));
        assert_eq!(from_str("101"), Ok((101u32, 3)));
        assert_eq!(from_str("101"), Ok((101u64, 3)));
        assert_eq!(from_str("-101"), Ok((-101i8, 4)));
        assert_eq!(from_str("-101"), Ok((-101i16, 4)));
        assert_eq!(from_str("-101"), Ok((-101i32, 4)));
        assert_eq!(from_str("-101"), Ok((-101i64, 4)));
        assert!(from_str::<u16>("-01").is_err());
        assert!(from_str::<u16>("00").is_err());
        assert!(from_str::<u16>("-1").is_err());
        assert!(from_str::<u16>("1e5").is_err());
        assert!(from_str::<u8>("256").is_err());
        assert!(from_str::<i8>("-129").is_err());
        assert!(from_str::<f32>(",").is_err());
    }

    #[test]
    fn test_de_enum_clike() {
        assert_eq!(from_str(r#" "boolean" "#), Ok((Type::Boolean, 11)));
        assert_eq!(from_str(r#" "number" "#), Ok((Type::Number, 10)));
        assert_eq!(from_str(r#" "thing" "#), Ok((Type::Thing, 9)));
        assert_eq!(from_str::<Type>(r#" "" "#), Err(Error::CustomError));
        assert_eq!(from_str::<Type>(r#" "xyz" "#), Err(Error::CustomError));
        assert_eq!(from_str::<Type>(r#" {} "#), Err(Error::ExpectedString));
        assert_eq!(from_str::<Type>(r#" [] "#), Err(Error::ExpectedEnumValue));
    }

    #[test]
    fn test_de_str() {
        let buf = &mut Vec::new();
        assert_eq!(from_bufstr(buf, r#" "hello" "#), Ok(("hello", 9)));
        assert_eq!(from_bufstr(buf, r#" "" "#), Ok(("", 4)));
        assert_eq!(from_bufstr(buf, r#" " " "#), Ok((" ", 5)));
        assert_eq!(from_bufstr(buf, r#" "👏" "#), Ok(("👏", 8)));

        assert_eq!(from_bufstr(buf, r#" "hel\tlo" "#), Ok(("hel\tlo", 11)));
        assert_eq!(from_bufstr(buf, r#" "hello \\" "#), Ok(("hello \\", 12)));

        // escaped " in the string content
        assert_eq!(from_bufstr(buf, r#" "foo\"bar" "#), Ok((r#"foo"bar"#, 12)));
        assert_eq!(
            from_bufstr(buf, r#" "foo\\\"bar" "#),
            Ok((r#"foo\"bar"#, 14))
        );
        assert_eq!(
            from_bufstr(buf, r#" "foo\"\"bar" "#),
            Ok((r#"foo""bar"#, 14))
        );
        assert_eq!(from_bufstr(buf, r#" "\"bar" "#), Ok((r#""bar"#, 9)));
        assert_eq!(from_bufstr(buf, r#" "foo\"" "#), Ok((r#"foo""#, 9)));
        assert_eq!(from_bufstr(buf, r#" "\"" "#), Ok((r#"""#, 6)));

        // non-excaped " preceded by backslashes
        assert_eq!(
            from_bufstr(buf, r#" "foo bar\\" "#),
            Ok((r#"foo bar\"#, 13))
        );
        assert_eq!(
            from_bufstr(buf, r#" "foo bar\\\\" "#),
            Ok((r#"foo bar\\"#, 15))
        );
        assert_eq!(
            from_bufstr(buf, r#" "foo bar\\\\\\" "#),
            Ok((r#"foo bar\\\"#, 17))
        );
        assert_eq!(
            from_bufstr(buf, r#" "foo bar\\\\\\\\" "#),
            Ok((r#"foo bar\\\\"#, 19))
        );
        assert_eq!(from_bufstr(buf, r#" "\\" "#), Ok((r#"\"#, 6)));
        assert_eq!(from_bufstr::<&str>(buf, r#" "\x" "#), Err(Error::InvalidEscapeSequence));
        assert_eq!(from_bufstr::<&str>(buf, r#" "\c" "#), Err(Error::InvalidEscapeSequence));
        assert_eq!(from_bufstr::<&str>(buf, r#" "\u000" "#), Err(Error::InvalidEscapeSequence));
        assert_eq!(from_bufstr::<&str>(buf, r#" "\uD800" "#), Err(Error::InvalidUnicodeCodePoint));
        assert_eq!(from_bufstr::<&str>(buf, r#" "\uDFFF" "#), Err(Error::InvalidUnicodeCodePoint));
    }

    #[test]
    fn test_de_struct() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Test {
            foo: i8,
        }
        assert_eq!(
            from_str(r#"{ "foo": 0 }"#),
            Ok((Test { foo: 0 }, 12))
        );
        assert_eq!(
            from_str(r#"{"foo":-1}"#),
            Ok((Test {foo:-1}, 10))
        );
        // errors
        assert_eq!(from_str::<Test>("[]"), Err(Error::ExpectedObject));
        assert_eq!(from_str::<Test>(r#"{"foo":0]"#), Err(Error::ExpectedObjectCommaOrEnd));
        assert_eq!(from_str::<Test>(r#"{"foo":0,}"#), Err(Error::TrailingObjectComma));
        assert_eq!(from_str::<Test>(r#"{,}"#), Err(Error::LeadingObjectComma));
        assert_eq!(from_str::<Test>(r#"{,"foo":0}"#), Err(Error::LeadingObjectComma));
    }

    #[test]
    fn test_de_struct_bool() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Led {
            led: bool,
        }

        assert_eq!(
            from_str(r#"{ "led": true }"#),
            Ok((Led { led: true }, 15))
        );
        assert_eq!(
            from_str(r#"{ "led": false }"#),
            Ok((Led { led: false }, 16))
        );
    }

    #[test]
    fn test_de_struct_i8() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Temperature {
            temperature: i8,
        }

        assert_eq!(
            from_str(r#"{ "temperature": -17 }"#),
            Ok((Temperature { temperature: -17 }, 22))
        );

        assert_eq!(
            from_str(r#"{ "temperature": -0 }"#),
            Ok((Temperature { temperature: -0 }, 21))
        );

        assert_eq!(
            from_str(r#"{ "temperature": 0 }"#),
            Ok((Temperature { temperature: 0 }, 20))
        );

        // out of range
        assert!(from_str::<Temperature>(r#"{ "temperature": 128 }"#).is_err());
        assert!(from_str::<Temperature>(r#"{ "temperature": -129 }"#).is_err());
    }

    #[test]
    fn test_de_struct_f32() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Temperature {
            temperature: f32,
        }

        assert_eq!(
            from_str(r#"{ "temperature": -17.2 }"#),
            Ok((Temperature { temperature: -17.2 }, 24))
        );

        assert_eq!(
            from_str(r#"{ "temperature": -0.0 }"#),
            Ok((Temperature { temperature: -0. }, 23))
        );

        assert_eq!(
            from_str(r#"{ "temperature": -2.1e-3 }"#),
            Ok((
                Temperature {
                    temperature: -2.1e-3
                },
                26
            ))
        );

        assert_eq!(
            from_str(r#"{ "temperature": -3 }"#),
            Ok((Temperature { temperature: -3. }, 21))
        );

        use core::f32;

        assert_eq!(
            from_str(r#"{ "temperature": -1e500 }"#),
            Ok((
                Temperature {
                    temperature: f32::NEG_INFINITY
                },
                25
            ))
        );

        // NaNs will always compare unequal.
        let (r, n): (Temperature, usize) = from_str(r#"{ "temperature": null }"#).unwrap();
        assert!(r.temperature.is_nan());
        assert_eq!(n, 23);

        assert!(from_str::<Temperature>(r#"{ "temperature": 1e1e1 }"#).is_err());
        assert!(from_str::<Temperature>(r#"{ "temperature": -2-2 }"#).is_err());
        assert!(from_str::<Temperature>(r#"{ "temperature": 1 1 }"#).is_err());
        assert!(from_str::<Temperature>(r#"{ "temperature": 0.0. }"#).is_err());
        assert!(from_str::<Temperature>(r#"{ "temperature": ä }"#).is_err());
        assert!(from_str::<Temperature>(r#"{ "temperature": None }"#).is_err());
    }

    #[test]
    fn test_de_struct_option() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Property<'a> {
            #[serde(borrow)]
            description: Option<&'a str>,
        }

        let buf = &mut Vec::new();

        assert_eq!(
            from_bufstr(buf, r#"{ "description": "An ambient temperature sensor" }"#),
            Ok((
                Property {
                    description: Some("An ambient temperature sensor"),
                },
                50
            ))
        );

        assert_eq!(
            from_bufstr(buf, r#"{ "description": null }"#),
            Ok((Property { description: None }, 23))
        );

        assert_eq!(
            from_bufstr(buf, r#"{}"#),
            Ok((Property { description: None }, 2))
        );
    }

    #[test]
    fn test_de_struct_u8() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Temperature {
            temperature: u8,
        }

        assert_eq!(
            from_str(r#"{ "temperature": 20 }"#),
            Ok((Temperature { temperature: 20 }, 21))
        );

        assert_eq!(
            from_str(r#"{ "temperature": 0 }"#),
            Ok((Temperature { temperature: 0 }, 20))
        );

        // out of range
        assert!(from_str::<Temperature>(r#"{ "temperature": 256 }"#).is_err());
        assert!(from_str::<Temperature>(r#"{ "temperature": -1 }"#).is_err());
    }

    #[test]
    fn test_de_test_unit() {
        assert_eq!(from_str::<()>(r#"null"#), Ok(((), 4)));
    }

    #[test]
    fn test_de_newtype_struct() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct A(u32);

        assert_eq!(from_str::<A>(r#"54"#), Ok((A(54), 2)));
    }

    #[test]
    fn test_de_newtype_variant() {
        #[derive(Deserialize, Debug, PartialEq)]
        enum A {
            A(u32),
        }
        let a = A::A(54);
        let x = from_str::<A>(r#"{"A":54}"#);
        assert_eq!(x, Ok((a, 8)));
    }

    #[test]
    fn test_de_struct_variant() {
        #[derive(Deserialize, Debug, PartialEq)]
        enum A {
            A { x: u32, y: u16 },
        }
        let a = A::A { x: 54, y: 720 };
        let x = from_str::<A>(r#"{"A": {"x":54,"y":720 } }"#);
        assert_eq!(x, Ok((a, 25)));
    }

    #[test]
    fn test_de_struct_tuple() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Xy(i8, i8);

        assert_eq!(from_str(r#"[10, 20]"#), Ok((Xy(10, 20), 8)));
        assert_eq!(from_str(r#"[10, -20]"#), Ok((Xy(10, -20), 9)));

        // wrong number of args
        assert_eq!(
            from_str::<Xy>(r#"[10]"#),
            Err(Error::CustomError)
        );
        assert_eq!(
            from_str::<Xy>(r#"[10, 20, 30]"#),
            Err(Error::ExpectedArrayEnd)
        );
    }

    #[test]
    fn test_de_struct_with_array_field() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Test {
            status: bool,
            point: [u32; 3],
        }

        assert_eq!(
            from_str(r#"{ "status": true, "point": [1, 2, 3] }"#),
            Ok((
                Test {
                    status: true,
                    point: [1, 2, 3]
                },
                38
            ))
        );
    }

    #[test]
    fn test_de_struct_with_tuple_field() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Test {
            status: bool,
            point: (u32, u32, u32),
        }

        assert_eq!(
            from_str(r#"{ "status": true, "point": [1, 2, 3] }"#),
            Ok((
                Test {
                    status: true,
                    point: (1, 2, 3)
                },
                38
            ))
        );
    }

    #[test]
    fn test_de_ignoring_extra_fields() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Temperature {
            temperature: u8,
        }

        assert_eq!(
            from_str(r#"{ "temperature": 20, "high": 80, "low": -10, "updated": true }"#),
            Ok((Temperature { temperature: 20 }, 62))
        );

        assert_eq!(
            from_str(
                r#"{ "temperature": 20, "conditions": "windy", "forecast": "cloudy" }"#
            ),
            Ok((Temperature { temperature: 20 }, 66))
        );

        assert_eq!(
            from_str(r#"{ "temperature": 20, "hourly_conditions": ["windy", "rainy"] }"#),
            Ok((Temperature { temperature: 20 }, 62))
        );

        assert_eq!(
            from_str(
                r#"{ "temperature"  :  20, "source": { "station": "dock", "sensors": ["front", "back"] } }"#
            ),
            Ok((Temperature { temperature: 20 }, 87))
        );

        assert_eq!(
            from_str(
                r#"{ "source": { "station": "dock", "sensors": ["\\", "\""] }, "temperature":20}"#
            ),
            Ok((Temperature { temperature: 20 }, 77))
        );

        assert_eq!(
            from_str::<Temperature>(r#"{ "temperature": 20, "invalid": this-is-not-ignored }"#),
            Err(Error::ExpectedToken)
        );

        assert_eq!(
            from_str::<Temperature>(r#"{ "temperature": 20, "broken": }"#),
            Err(Error::UnexpectedChar)
        );

        assert_eq!(
            from_str::<Temperature>(r#"{ "temperature": 20, "broken": [ }"#),
            Err(Error::UnexpectedChar)
        );

        assert_eq!(
            from_str::<Temperature>(r#"{ "temperature": 20, "broken": ] }"#),
            Err(Error::UnexpectedChar)
        );
    }

    #[test]
    fn test_de_wot() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Thing<'a> {
            #[serde(borrow)]
            properties: Properties<'a>,
            #[serde(rename = "type")]
            ty: Type,
        }

        #[derive(Debug, Deserialize, PartialEq)]
        struct Properties<'a> {
            #[serde(borrow)]
            temperature: Property<'a>,
            #[serde(borrow)]
            humidity: Property<'a>,
            #[serde(borrow)]
            led: Property<'a>,
        }

        #[derive(Debug, Deserialize, PartialEq)]
        struct Property<'a> {
            #[serde(rename = "type")]
            ty: Type,
            unit: Option<&'a str>,
            #[serde(borrow)]
            description: Option<&'a str>,
            href: &'a str,
        }

        let buf = &mut Vec::new();

        assert_eq!(
            from_bufstr::<Thing<'_>>(buf,
                r#"
                    {
                    "type": "thing",
                    "properties": {
                        "temperature": {
                        "type": "number",
                        "unit": "celsius",
                        "description": "An ambient temperature sensor",
                        "href": "/properties/temperature"
                        },
                        "humidity": {
                        "type": "number",
                        "unit": "percent",
                        "href": "/properties/humidity"
                        },
                        "led": {
                        "type": "boolean",
                        "description": "A red LED",
                        "href": "/properties/led"
                        }
                    }
                    }
                    "#
            ),
            Ok((
                Thing {
                    properties: Properties {
                        temperature: Property {
                            ty: Type::Number,
                            unit: Some("celsius"),
                            description: Some("An ambient temperature sensor"),
                            href: "/properties/temperature",
                        },
                        humidity: Property {
                            ty: Type::Number,
                            unit: Some("percent"),
                            description: None,
                            href: "/properties/humidity",
                        },
                        led: Property {
                            ty: Type::Boolean,
                            unit: None,
                            description: Some("A red LED"),
                            href: "/properties/led",
                        },
                    },
                    ty: Type::Thing,
                },
                852
            ))
        )
    }
}