//! Bare structures of time zone files
//!
//! For more information on what these values mean, see
//! [man 5 tzfile](ftp://ftp.iana.org/tz/code/tzfile.5.txt).

use byteorder::{ReadBytesExt, BigEndian};

use std::error::Error;
use std::io::{Cursor, Read};


#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Header {

    /// The version of this file's format - either '\0', or '2', or '3'.
    pub version: u8,

    /// The number of Universal Time entries in this file.
    /// (Equivalent to `tzh_ttisgmtcnt` in C)
    pub num_utc: u32,

    /// The number of standard entries in this file.
    /// (Equivalent to `tzh_ttisstdcnt` in C)
    pub num_standard: u32,

    /// The number of leap second entries in this file.
    /// (Equivalent to `tzh_leapcnt` in C)
    pub num_leap_seconds: u32,

    /// The number of transition entries in this file.
    /// (Equivalent to `tzh_timecnt` in C)
    pub num_transition_times: u32,

    /// The number of local time types in this file.
    /// (Equivalent to `tzh_typecnt` in C)
    pub num_time_types: u32,

    /// The number of characters of time zone abbreviation strings in this file.
    /// (Equivalent to `tzh_charcnt` in C)
    pub num_chars: u32,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TransitionData {

    /// The time at which the rules for computing local time change.
    pub timestamp: u32,

    /// Index into the local time types array for this transition.
    pub local_time_type_index: u8,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LocalTimeTypeData {

    /// Number of seconds to be added to Universal Time.
    /// (Equivalent to `tt_gmtoff` in C)
    pub offset: u32,

    /// Whether to set DST.
    /// (Equivalent to `tt_isdst` in C)
    pub is_dst: u8,

    /// Position in the array of time zone abbreviation characters, elsewhere
    /// in the file.
    /// (Equivalent to `tt_abbrind` in C)
    pub name_offset: u8,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LeapSecondData {

    /// The time, as a number of seconds, at which a leap second occurs.
    pub timestamp: u32,

    /// Number of leap seconds to be added.
    pub leap_second_count: u32,
}


/// Maximum numbers of structures that can be loaded from a time zone data
/// file. If more than these would be loaded, an error will be returned
/// instead.
///
/// Why have limits? Well, the header portion of the file (see `Header`)
/// specifies the numbers of structures that should be read as a `u32`
/// four-byte integer. This means that
#[derive(Debug, Clone)]
pub struct Limits {

    /// Maximum number of transition structures
    pub max_transitions: Option<u32>,

    /// Maximum number of local time type structures
    pub max_local_time_types: Option<u32>,

    /// Maximum number of bytes in timezone abbreviations
    pub max_abbreviation_bytes: Option<u32>,

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
            max_abbreviation_bytes: None,
            max_leap_seconds: None,
        }
    }

    /// A reasonable set of default values that pose no danger of using lots
    /// of memory.
    ///
    /// These values are taken from `tz_file.h`, at
    /// ftp://ftp.iana.org/tz/code/tzfile.h
    pub fn sensible() -> Limits {
        Limits {
            max_transitions: Some(2000),
            max_local_time_types: Some(256),
            max_abbreviation_bytes: Some(50),
            max_leap_seconds: Some(50),
        }
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

    fn read_magic_number(&mut self) -> Result<bool, Box<Error>> {
        let mut magic = [0u8; 4];
        try!(self.cursor.read(&mut magic));
        Ok(magic == *b"TZif")
    }

    fn skip_initial_zeroes(&mut self) -> Result<(), Box<Error>> {
        let mut magic = [0u8; 15];
        try!(self.cursor.read(&mut magic));
        Ok(())
    }

    fn read_header(&mut self) -> Result<Header, Box<Error>> {
        Ok(Header {
            version: try!(self.cursor.read_u8()),
            num_utc: try!(self.cursor.read_u32::<BigEndian>()),
            num_standard: try!(self.cursor.read_u32::<BigEndian>()),
            num_leap_seconds: try!(self.cursor.read_u32::<BigEndian>()),
            num_transition_times: try!(self.cursor.read_u32::<BigEndian>()),
            num_time_types: try!(self.cursor.read_u32::<BigEndian>()),
            num_chars: try!(self.cursor.read_u32::<BigEndian>()),
        })
    }

    fn read_transitions(&mut self, count: usize) -> Result<Vec<TransitionData>, Box<Error>> {
        let mut times = Vec::with_capacity(count);
        for _ in 0 .. count {
            times.push(try!(self.cursor.read_u32::<BigEndian>()));
        }

        let mut types = Vec::with_capacity(count);
        for _ in 0 .. count {
            types.push(try!(self.cursor.read_u8()));
        }

        Ok(times.iter().zip(types.iter()).map(|(&ti, &ty)| {
            TransitionData {
                timestamp: ti,
                local_time_type_index: ty,
            }
        }).collect())
     }

    fn read_octets(&mut self, count: usize) -> Result<Vec<u8>, Box<Error>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(try!(self.cursor.read_u8()));
        }
        Ok(buf)
    }

    fn read_info_structures(&mut self, count: usize) -> Result<Vec<LocalTimeTypeData>, Box<Error>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(LocalTimeTypeData {
                offset:  try!(self.cursor.read_u32::<BigEndian>()),
                is_dst:  try!(self.cursor.read_u8()),
                name_offset: try!(self.cursor.read_u8()),
            });
        }
        Ok(buf)
    }

    fn read_leap_second_info(&mut self, count: usize) -> Result<Vec<LeapSecondData>, Box<Error>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(LeapSecondData {
                timestamp:          try!(self.cursor.read_u32::<BigEndian>()),
                leap_second_count:  try!(self.cursor.read_u32::<BigEndian>()),
            });
        }
        Ok(buf)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TZData {
    pub header: Header,
    pub transitions: Vec<TransitionData>,
    pub time_info: Vec<LocalTimeTypeData>,
    pub leap_seconds: Vec<LeapSecondData>,
    pub strings: Vec<u8>,
    pub standard_flags: Vec<u8>,
    pub utc_flags: Vec<u8>,
}

pub fn parse(buf: Vec<u8>) -> Result<TZData, Box<Error>> {
    let mut parser = Parser::new(buf);
    let magic = parser.read_magic_number();
    parser.skip_initial_zeroes();

    let header        = try!(parser.read_header());
    let transitions   = try!(parser.read_transitions(header.num_transition_times as usize));
    let time_types    = try!(parser.read_info_structures(header.num_time_types as usize));
    let leap_seconds  = try!(parser.read_leap_second_info(header.num_leap_seconds as usize));
    let strings       = try!(parser.read_octets(header.num_chars as usize));
    let standards     = try!(parser.read_octets(header.num_standard as usize));
    let utcs          = try!(parser.read_octets(header.num_utc as usize));

    Ok(TZData {
        header:          header,
        transitions:     transitions,
        time_info:       time_types,
        leap_seconds:    leap_seconds,
        strings:         strings,
        standard_flags:  standards,
        utc_flags:       utcs,
    })
}
