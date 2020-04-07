//! Parsing and structures of time zone files
//!
//! This module reads data from a buffer of bytes, and parses it into a
//! `TZData` structure -- **doing a minimum of interpretation as to what these
//! values mean!** The data read are all kept as primitive numeric types. For
//! the code that turns these numbers into actual timezone data, see the root
//! module.
//!
//! For more information on what these values mean, see
//! [man 5 tzfile](ftp://ftp.iana.org/tz/code/tzfile.5.txt).

use byteorder::{ReadBytesExt, BigEndian};

use std::error::Error as ErrorTrait;
use std::fmt;
use std::io::{Cursor, Read};
use std::result;


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Header {

    /// The version of this file's format - either '\0', or '2', or '3'.
    pub version: u8,

    /// The number of GMT flags in this file.
    /// (Equivalent to `tzh_ttisgmtcnt` in C)
    pub num_gmt_flags: u32,

    /// The number of Standard Time flags in this file.
    /// (Equivalent to `tzh_ttisstdcnt` in C)
    pub num_standard_flags: u32,

    /// The number of leap second entries in this file.
    /// (Equivalent to `tzh_leapcnt` in C)
    pub num_leap_seconds: u32,

    /// The number of transition entries in this file.
    /// (Equivalent to `tzh_timecnt` in C)
    pub num_transitions: u32,

    /// The number of local time types in this file.
    /// (Equivalent to `tzh_typecnt` in C)
    pub num_local_time_types: u32,

    /// The number of characters of time zone abbreviation strings in this file.
    /// (Equivalent to `tzh_charcnt` in C)
    pub num_abbr_chars: u32,
}


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct TransitionData {

    /// The time at which the rules for computing local time change.
    pub timestamp: i32,

    /// Index into the local time types array for this transition.
    pub local_time_type_index: u8,
}


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct LocalTimeTypeData {

    /// Number of seconds to be added to Universal Time.
    /// (Equivalent to `tt_gmtoff` in C)
    pub offset: i32,

    /// Whether to set DST.
    /// (Equivalent to `tt_isdst` in C)
    pub is_dst: u8,

    /// Position in the array of time zone abbreviation characters, elsewhere
    /// in the file.
    /// (Equivalent to `tt_abbrind` in C)
    pub name_offset: u8,
}


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct LeapSecondData {

    /// The time, as a number of seconds, at which a leap second occurs.
    pub timestamp: i32,

    /// Number of leap seconds to be added.
    pub leap_second_count: i32,
}


/// Maximum numbers of structures that can be loaded from a time zone data
/// file. If more than these would be loaded, an error will be returned
/// instead.
///
/// Why have limits? Well, the header portion of the file (see `Header`)
/// specifies the numbers of structures that should be read as a `u32`
/// four-byte integer. This means that an invalid (or maliciously-crafted!)
/// file could try to read *gigabytes* of data while trying to read time zone
/// information. To prevent this, reasonable defaults are set, although they
/// can be turned off if necessary.
#[derive(Debug, Copy, Clone)]
pub struct Limits {

    /// Maximum number of transition structures
    pub max_transitions: Option<u32>,

    /// Maximum number of local time type structures
    pub max_local_time_types: Option<u32>,

    /// Maximum number of characters (bytes, technically) in timezone
    /// abbreviations
    pub max_abbreviation_chars: Option<u32>,

    /// Maximum number of leap second specifications
    pub max_leap_seconds: Option<u32>,
}

impl Limits {

    /// No size limits. This might use *lots* of memory when reading an
    /// invalid file, so be careful.
    pub fn none() -> Limits {
        Limits {
            max_transitions: None,
            max_local_time_types: None,
            max_abbreviation_chars: None,
            max_leap_seconds: None,
        }
    }

    /// A reasonable set of default values that pose no danger of using lots
    /// of memory.
    ///
    /// These values are taken from `tz_file.h`, at
    /// [ftp://ftp.iana.org/tz/code/tzfile.h].
    pub fn sensible() -> Limits {
        Limits {
            max_transitions: Some(2000),
            max_local_time_types: Some(256),
            max_abbreviation_chars: Some(50),
            max_leap_seconds: Some(50),
        }
    }

    /// Makes sure the values we just read from the header are within this set
    /// of limits. Returns `Ok(())` if everything is within the limits, and a
    /// boxed `Error` if at least one count is over.
    pub fn verify(self, header: &Header) -> Result<()> {
        let check = |structures, intended_count, limit| {
            if let Some(max) = limit {
                if intended_count > max {
                    return Err(Error::LimitReached {
                        structures,
                        intended_count,
                        limit: max,
                    });
                }
            }
            Ok(())
        };

        check(Structures::Transitions,       header.num_transitions,      self.max_transitions)?;
        check(Structures::LocalTimeTypes,    header.num_local_time_types, self.max_local_time_types)?;
        check(Structures::LeapSeconds,       header.num_leap_seconds,     self.max_leap_seconds)?;
        check(Structures::GMTFlags,          header.num_gmt_flags,        self.max_local_time_types)?;
        check(Structures::StandardFlags,     header.num_standard_flags,   self.max_local_time_types)?;
        check(Structures::TimezoneAbbrChars, header.num_abbr_chars,       self.max_abbreviation_chars)?;

        Ok(())
    }
}


struct Parser {
    cursor: Cursor<Vec<u8>>,
}

impl Parser {
    fn new(buf: Vec<u8>) -> Parser {
        Parser {
            cursor: Cursor::new(buf),
        }
    }

    fn read_magic_number(&mut self) -> Result<()> {
        let mut magic = [0u8; 4];
        self.cursor.read(&mut magic)?;
        if magic == *b"TZif" {
            Ok(())
        }
        else {
            Err(Box::new(Error::InvalidMagicNumber))
        }
    }

    fn skip_initial_zeroes(&mut self) -> Result<()> {
        let mut magic = [0u8; 15];
        self.cursor.read(&mut magic)?;
        Ok(())
    }

    fn read_header(&mut self) -> Result<Header> {
        Ok(Header {
            version:               self.cursor.read_u8()?,
            num_gmt_flags:         self.cursor.read_u32::<BigEndian>()?,
            num_standard_flags:    self.cursor.read_u32::<BigEndian>()?,
            num_leap_seconds:      self.cursor.read_u32::<BigEndian>()?,
            num_transitions:       self.cursor.read_u32::<BigEndian>()?,
            num_local_time_types:  self.cursor.read_u32::<BigEndian>()?,
            num_abbr_chars:        self.cursor.read_u32::<BigEndian>()?,
        })
    }

    fn read_transition_data(&mut self, count: usize) -> Result<Vec<TransitionData>> {
        let mut times = Vec::with_capacity(count);
        for _ in 0 .. count {
            times.push(self.cursor.read_i32::<BigEndian>()?);
        }

        let mut types = Vec::with_capacity(count);
        for _ in 0 .. count {
            types.push(self.cursor.read_u8()?);
        }

        Ok(times.iter().zip(types.iter()).map(|(&ti, &ty)| {
            TransitionData {
                timestamp: ti,
                local_time_type_index: ty,
            }
        }).collect())
     }

    fn read_octets(&mut self, count: usize) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(self.cursor.read_u8()?);
        }
        Ok(buf)
    }

    fn read_local_time_type_data(&mut self, count: usize) -> Result<Vec<LocalTimeTypeData>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(LocalTimeTypeData {
                offset:  self.cursor.read_i32::<BigEndian>()?,
                is_dst:  self.cursor.read_u8()?,
                name_offset: self.cursor.read_u8()?,
            });
        }
        Ok(buf)
    }

    fn read_leap_second_data(&mut self, count: usize) -> Result<Vec<LeapSecondData>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(LeapSecondData {
                timestamp:          self.cursor.read_i32::<BigEndian>()?,
                leap_second_count:  self.cursor.read_i32::<BigEndian>()?,
            });
        }
        Ok(buf)
    }
}


/// A `std::result::Result` with a `Box<std::error::Error>` as the error type.
/// This is used to return a bunch of errors early, including a limit being
/// reached, the buffer failed to be read from, or a string not being valid
/// UTF-8.
pub type Result<T> = result::Result<T, Box<dyn ErrorTrait>>;

#[derive(Debug, Copy, Clone)]
pub enum Error {

    /// The error when the first four bytes of the buffer weren't what they
    /// should be.
    InvalidMagicNumber,

    /// The error when too many structures would have been read from the
    /// buffer, in order to prevent this library from using too much memory.
    LimitReached {

        /// The type of structure that we attempted to read.
        structures: Structures,

        /// The number of these structures that we attempted to read.
        intended_count: u32,

        /// The maximum number of structures that we can get away with.
        limit: u32,
    },

    /// The error when a file doesn’t actually contain any transitions. (It
    /// should always contain at least one, so we know what the *base* offset
    /// from UTC is.)
    NoTransitions,
}

impl ErrorTrait for Error {
    fn description(&self) -> &str {
        match *self {
            Error::InvalidMagicNumber   => "invalid magic number",
            Error::LimitReached { .. }  => "limit reached",
            Error::NoTransitions        => "no transitions",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::InvalidMagicNumber => write!(f, "invalid magic number"),

            Error::LimitReached { ref structures, ref intended_count, ref limit } => {
                write!(f, "too many {} (tried to read {}, limit was {})", structures, intended_count, limit)
            },

            Error::NoTransitions => {
                write!(f, "read 0 transitions")
            },
        }
    }
}


/// A description of which value is being read. This gets used solely for
/// error reporting.
#[derive(Debug, Copy, Clone)]
pub enum Structures {
    Transitions,
    LocalTimeTypes,
    LeapSeconds,
    GMTFlags,
    StandardFlags,
    TimezoneAbbrChars,
}

impl fmt::Display for Structures {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Structures::Transitions        => "transitions".fmt(f),
            Structures::LocalTimeTypes     => "local time types".fmt(f),
            Structures::LeapSeconds        => "leap second".fmt(f),
            Structures::GMTFlags           => "GMT flags".fmt(f),
            Structures::StandardFlags      => "Standard Time flags".fmt(f),
            Structures::TimezoneAbbrChars  => "timezone abbreviation chars".fmt(f),
        }
    }
}


/// The internal structure of a zoneinfo file.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TZData {
    pub header: Header,
    pub transitions: Vec<TransitionData>,
    pub time_info: Vec<LocalTimeTypeData>,
    pub leap_seconds: Vec<LeapSecondData>,
    pub strings: Vec<u8>,
    pub standard_flags: Vec<u8>,
    pub gmt_flags: Vec<u8>,
}

/// Parse a series of bytes into a `TZData` structure, returning an error if
/// the buffer fails to be read from, or a limit is reached.
pub fn parse(buf: Vec<u8>, limits: Limits) -> Result<TZData> {
    let mut parser = Parser::new(buf);
    parser.read_magic_number()?;
    parser.skip_initial_zeroes()?;

    let header = parser.read_header()?;
    limits.verify(&header)?;

    let transitions    = parser.read_transition_data(header.num_transitions as usize)?;
    let time_info      = parser.read_local_time_type_data(header.num_local_time_types as usize)?;
    let strings        = parser.read_octets(header.num_abbr_chars as usize)?;
    let leap_seconds   = parser.read_leap_second_data(header.num_leap_seconds as usize)?;
    let standard_flags = parser.read_octets(header.num_standard_flags as usize)?;
    let gmt_flags      = parser.read_octets(header.num_gmt_flags as usize)?;

    Ok(TZData {
        header,
        transitions,
        time_info,
        leap_seconds,
        strings,
        standard_flags,
        gmt_flags,
    })
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn est() {
        let bytes = vec![
            0x54, 0x5A, 0x69, 0x66, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x04, 0xFF, 0xFF, 0xB9, 0xB0,
            0x00, 0x00, 0x45, 0x53, 0x54, 0x00, 0x00, 0x00,
        ];

        let data = parse(bytes, Limits::sensible()).unwrap();
        assert_eq!(data.header.num_transitions, 0);
        assert_eq!(data.header.num_leap_seconds, 0);
        assert_eq!(data.header.num_local_time_types, 1);
    }

    #[test]
    fn japan() {
        let bytes = vec![
            0x54, 0x5A, 0x69, 0x66, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03,
            0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x03,
            0x00, 0x00, 0x00, 0x0D, 0xC3, 0x55, 0x3B, 0x70,
            0xD7, 0x3E, 0x1E, 0x90, 0xD7, 0xEC, 0x16, 0x80,
            0xD8, 0xF9, 0x16, 0x90, 0xD9, 0xCB, 0xF8, 0x80,
            0xDB, 0x07, 0x1D, 0x10, 0xDB, 0xAB, 0xDA, 0x80,
            0xDC, 0xE6, 0xFF, 0x10, 0xDD, 0x8B, 0xBC, 0x80,
            0x02, 0x01, 0x02, 0x01, 0x02, 0x01, 0x02, 0x01,
            0x02, 0x00, 0x00, 0x7E, 0x90, 0x00, 0x00, 0x00,
            0x00, 0x8C, 0xA0, 0x01, 0x05, 0x00, 0x00, 0x7E,
            0x90, 0x00, 0x09, 0x4A, 0x43, 0x53, 0x54, 0x00,
            0x4A, 0x44, 0x54, 0x00, 0x4A, 0x53, 0x54, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let data = parse(bytes, Limits::sensible()).unwrap();
        assert_eq!(data.header.num_transitions, 9);
        assert_eq!(data.header.num_leap_seconds, 0);
        assert_eq!(data.header.num_local_time_types, 3);

        assert_eq!(data.transitions, vec![
            TransitionData { timestamp: -1_017_824_400, local_time_type_index: 2 },
            TransitionData { timestamp:   -683_794_800, local_time_type_index: 1 },
            TransitionData { timestamp:   -672_393_600, local_time_type_index: 2 },
            TransitionData { timestamp:   -654_764_400, local_time_type_index: 1 },
            TransitionData { timestamp:   -640_944_000, local_time_type_index: 2 },
            TransitionData { timestamp:   -620_290_800, local_time_type_index: 1 },
            TransitionData { timestamp:   -609_494_400, local_time_type_index: 2 },
            TransitionData { timestamp:   -588_841_200, local_time_type_index: 1 },
            TransitionData { timestamp:   -578_044_800, local_time_type_index: 2 },
        ]);

        assert_eq!(data.time_info, vec![
            LocalTimeTypeData { offset: 32400, is_dst: 0, name_offset: 0 },
            LocalTimeTypeData { offset: 36000, is_dst: 1, name_offset: 5 },
            LocalTimeTypeData { offset: 32400, is_dst: 0, name_offset: 9 },
        ]);
    }
}
