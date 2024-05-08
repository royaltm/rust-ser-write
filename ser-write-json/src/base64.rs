//! Base-64 codec.
use core::cell::Cell;
use crate::SerWrite;

static ALPHABET: &[u8;64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode an array of bytes as BASE-64 ASCII armour codes into a [`SerWrite`] implementing object.
///
/// _Note_: This function does not append BASE-64 '=' padding characters by itself
/// and instead returns the number of padding characters required: 0-2.
pub fn encode<W: SerWrite>(ser: &mut W, bytes: &[u8]) -> Result<u8, W::Error> {
    let mut chunks = bytes.chunks_exact(3);
    for slice in chunks.by_ref() {
        let [a,b,c] = slice.try_into().unwrap();
        let output = [
            a >> 2,
            ((a & 0x03) << 4) | ((b & 0xF0) >> 4),
            ((b & 0x0F) << 2) | ((c & 0xC0) >> 6),
            c & 0x3F
        ].map(|n| ALPHABET[(n & 0x3F) as usize]);
        ser.write(&output)?;
    }
    match chunks.remainder() {
        [a, b] => {
            let output = [
                a >> 2,
                ((a & 0x03) << 4) | ((b & 0xF0) >> 4),
                ((b & 0x0F) << 2)
            ].map(|n| ALPHABET[(n & 0x3F) as usize]);
            ser.write(&output)?;
            Ok(1)
        }
        [a] => {
            let output = [
                a >> 2,
                ((a & 0x03) << 4),
            ].map(|n| ALPHABET[(n & 0x3F) as usize]);
            ser.write(&output)?;
            Ok(2)
        }
        _ => Ok(0)
    }
}

#[inline]
fn get_code(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'/' => Some(63),
        b'+' => Some(62),
        _ => None
    }
}

// fn get_code(c: u8) -> Option<u8> {
//     match c & 0b11110000 {
//         b'@' if c >= b'A' => {
//             Some(c - b'A')
//         }
//         b'P' if c <= b'Z' => {
//             Some(c - b'A')
//         }
//         b'`' if c >= b'a' => {
//             Some(c - (b'a' - 26))
//         }
//         b'p' if c <= b'Z' => {
//             Some(c - b'A')
//         }
//         b'0' if c <= b'9' => {
//             Some(c + (52 - b'0'))
//         }
//         b' ' => match c {
//             b'/' => Some(63),
//             b'+' => Some(62),
//             _ => None
//         }
//         _ => None // non-ASCII or control
//     }
// }

// static DIGITS: &[u8;96] = [
//     80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 62, 80, 80, 80, 63, /* 32 - 47 */
//     52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 80, 80, 80, 64, 80, 80, /* 48 - 63 */
//     80,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, /* 64 - 79 */
//     15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 80, 80, 80, 80, 80, /* 80 - 95 */
//     80, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, /* 96 - 111 */
//     41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 80, 80, 80, 80, 80 /* 112 - 127 */
// ];

//   010100    110111    010101    101110
//                       010100 << 18
//                       110111 << 12
//                       010101 << 6
//                       101110
//   01010011 01110101 01101110
//
//                            1 (0) (31)
//                      1010100 (1) (25)
//                   1 01010000 (1) (25)(<<2)
//               10101 00110111 (2) (19)
//          1 01010011 01110000 (2) (19)(<<4)
//        101 01001101 11010101 (3) (13)
// 1 01010011 01110101 01000000 (3) (13)(<<6)
// 1 01010011 01110101 01101110 (4) (7)
#[inline(always)]
fn decode_cell(acc: u32, cell: &Cell<u8>) -> core::result::Result<u32, u32> {
    match get_code(cell.get()) {
        Some(code) => Ok((acc << 6) | u32::from(code)),
        None => Err(acc)
    }
}
/// Decode a BASE-64 encoded slice of byte characters in-place until a first
/// invalid character is found or until the end of the slice.
///
/// Return a tuple of: (decoded_len, encoded_len).
///
/// `decoded_len <= encoded_len <= slice.len()`
pub fn decode(slice: &mut[u8]) -> (usize, usize) {
    let cells = Cell::from_mut(slice).as_slice_of_cells();
    let mut chunks = cells.chunks_exact(4);
    let mut dest = cells.into_iter();
    let mut dcount: usize = 0;
    for slice in chunks.by_ref() {
        match slice.iter().try_fold(1, decode_cell) {
            Ok(packed) => {
                // SAFETY: dest and chunks iterate over the same cells slice,
                // while for every 4 byte chunk only 3 dest bytes are consumed,
                // there's no way dest.next() can be None at any point
                unsafe {
                    dest.next().unwrap_unchecked().set((packed >> 16) as u8);
                    dest.next().unwrap_unchecked().set((packed >> 8) as u8);
                    dest.next().unwrap_unchecked().set(packed as u8);
                }
                dcount += 3;
            }
            Err(packed) => return handle_tail(dcount, packed, dest)
        }
    }
    match chunks.remainder().iter().try_fold(1, decode_cell) {
        /* no tail */
        Ok(1) => (dcount, dcount * 4 / 3),
        /* some tail */
        Ok(packed)|Err(packed) => handle_tail(dcount, packed, dest)
    }
}

fn handle_tail<'a, I>(mut dcount: usize, mut packed: u32, mut dest: I) -> (usize, usize)
    where I: Iterator<Item=&'a Cell<u8>>
{
    // 31->(+0, +0), 25->(+0, +1), 19->(+1, +2), 13->(+2, +3)
    let leftovers = (31 - packed.leading_zeros()) / 6;
    packed <<= leftovers*2;
    let mut tail_dcount = leftovers.saturating_sub(1);
    let ecount = dcount * 4 / 3 + leftovers as usize;
    dcount += tail_dcount as usize;
    while tail_dcount != 0 {
        dest.next().unwrap().set((packed >> (tail_dcount * 8)) as u8);
        tail_dcount -= 1;
    }
    (dcount, ecount)
}

#[cfg(test)]
mod tests {
    use std::{vec::Vec};
    use super::*;

    #[test]
    fn test_base64_encode() {
        let vec = &mut Vec::new();
        encode(vec, &[]).unwrap();
        assert_eq!(&*vec, b"");
        encode(vec, &[0]).unwrap();
        assert_eq!(&*vec, b"AA");
        vec.clear();
        encode(vec, &[1]).unwrap();
        assert_eq!(&*vec, b"AQ");
        vec.clear();
        encode(vec, &[0,0]).unwrap();
        assert_eq!(&*vec, b"AAA");
        vec.clear();
        encode(vec, &[0,0,0]).unwrap();
        assert_eq!(&*vec, b"AAAA");
        vec.clear();
        encode(vec, &[0,0,0,0]).unwrap();
        assert_eq!(&*vec, b"AAAAAA");
        vec.clear();
        encode(vec, &[1,2]).unwrap();
        assert_eq!(&*vec, b"AQI");
        vec.clear();
        encode(vec, &[1,2,3]).unwrap();
        assert_eq!(&*vec, b"AQID");
        vec.clear();
        encode(vec, &[1,2,3,4]).unwrap();
        assert_eq!(&*vec, b"AQIDBA");
        vec.clear();
        encode(vec, &[0x80]).unwrap();
        assert_eq!(&*vec, b"gA");
        vec.clear();
        encode(vec, &[0x80,0x81]).unwrap();
        assert_eq!(&*vec, b"gIE");
        vec.clear();
        encode(vec, &[0x80,0x81,0x82]).unwrap();
        assert_eq!(&*vec, b"gIGC");
        vec.clear();
        encode(vec, &[0xFF]).unwrap();
        assert_eq!(&*vec, b"/w");
        vec.clear();
        encode(vec, &[0xFF,0xFF]).unwrap();
        assert_eq!(&*vec, b"//8");
        vec.clear();
        encode(vec, &[0xFF,0xFF,0xFF]).unwrap();
        assert_eq!(&*vec, b"////");
    }

    fn test_decode(vec: &mut Vec<u8>, encoded: &[u8], expected: (usize, usize), decoded: &[u8]) {
        for i in 0..=4 {
            vec.clear();
            vec.extend_from_slice(encoded);
            for _ in 0..i {
                vec.push(b'=');
            }
            assert_eq!(decode(vec.as_mut_slice()), expected);
            assert_eq!(&vec[..expected.0], decoded);
            if i == 0 {
                assert_eq!(vec.len(), expected.1);
            }
            else {
                assert_eq!(vec[expected.1], b'=');
            }
        }
    }

    #[test]
    fn test_base64_decode() {
        let vec = &mut Vec::new();
        test_decode(vec, b"", (0, 0), &[]);
        test_decode(vec, b"A", (0, 1), &[]);
        test_decode(vec, br"/", (0, 1), &[]);
        test_decode(vec, br"AA", (1,2), &[0]);
        test_decode(vec, br"AAA", (2,3), &[0,0]);
        test_decode(vec, br"AAAA", (3,4), &[0,0,0]);
        test_decode(vec, br"AAAAA", (3,5), &[0,0,0]);
        test_decode(vec, br"AAAAAA", (4,6), &[0,0,0,0]);
        test_decode(vec, br"AQ", (1,2), &[1]);
        test_decode(vec, br"AQI", (2,3), &[1,2]);
        test_decode(vec, br"AQID", (3,4), &[1,2,3]);
        test_decode(vec, br"AQIDB", (3,5), &[1,2,3]);
        test_decode(vec, br"AQIDBA", (4,6), &[1,2,3,4]);
        test_decode(vec, br"gA", (1,2), &[0x80]);
        test_decode(vec, br"gIE", (2,3), &[0x80,0x81]);
        test_decode(vec, br"gIGC", (3,4), &[0x80,0x81,0x82]);
        test_decode(vec, br"/w", (1,2), &[0xFF]);
        test_decode(vec, br"//8", (2,3), &[0xFF,0xFF]);
        test_decode(vec, br"////", (3,4), &[0xFF,0xFF,0xFF]);
        test_decode(vec, br"/////w", (4,6), &[0xFF,0xFF,0xFF,0xFF]);
        test_decode(vec, br"//////8", (5,7), &[0xFF,0xFF,0xFF,0xFF,0xFF]);
        test_decode(vec, br"////////", (6,8), &[0xFF,0xFF,0xFF,0xFF,0xFF,0xFF]);
   }
}