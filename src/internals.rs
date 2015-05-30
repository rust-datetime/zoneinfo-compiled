//! Bare structures of time zone files
//!
//! For more information on what these values mean, see
//! [man 5 tzfile](ftp://ftp.iana.org/tz/code/tzfile.5.txt).

use byteorder::{ReadBytesExt, BigEndian};

use std::error;
use std::fmt;
use std::io::{Cursor, Read};
use std::result;


#[derive(Debug, PartialEq, Eq, Clone)]
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
    pub offset: i32,

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
/// four-byte integer. This means that an invalid (or maliciously-crafted!)
/// file could try to read *gigabytes* of data while trying to read time zone
/// information. To prevent this, reasonable defaults are set, although they
/// can be turned off if necessary.
#[derive(Debug, Clone)]
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
    /// ftp://ftp.iana.org/tz/code/tzfile.h
    pub fn sensible() -> Limits {
        Limits {
            max_transitions: Some(2000),
            max_local_time_types: Some(256),
            max_abbreviation_chars: Some(50),
            max_leap_seconds: Some(50),
        }
    }

    pub fn verify(self, header: &Header) -> Result<()> {
        let check = |structures, intended_count, limit| {
            if let Some(max) = limit {
                if intended_count > max {
                    return Err(Error::LimitReached {
                        structures: structures,
                        intended_count: intended_count,
                        limit: max,
                    });
                }
            }
            Ok(())
        };

        try!(check(Structures::Transitions,       header.num_transitions,      self.max_transitions));
        try!(check(Structures::LocalTimeTypes,    header.num_local_time_types, self.max_local_time_types));
        try!(check(Structures::LeapSeconds,       header.num_leap_seconds,     self.max_leap_seconds));
        try!(check(Structures::GMTFlags,          header.num_gmt_flags,        self.max_local_time_types));
        try!(check(Structures::StandardFlags,     header.num_standard_flags,   self.max_local_time_types));
        try!(check(Structures::TimezoneAbbrChars, header.num_abbr_chars,       self.max_abbreviation_chars));

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
        try!(self.cursor.read(&mut magic));
        if magic == *b"TZif" {
            Ok(())
        }
        else {
            Err(Box::new(Error::InvalidMagicNumber { bytes_read: magic }))
        }
    }

    fn skip_initial_zeroes(&mut self) -> Result<()> {
        let mut magic = [0u8; 15];
        try!(self.cursor.read(&mut magic));
        Ok(())
    }

    fn read_header(&mut self) -> Result<Header> {
        Ok(Header {
            version:               try!(self.cursor.read_u8()),
            num_gmt_flags:         try!(self.cursor.read_u32::<BigEndian>()),
            num_standard_flags:    try!(self.cursor.read_u32::<BigEndian>()),
            num_leap_seconds:      try!(self.cursor.read_u32::<BigEndian>()),
            num_transitions:       try!(self.cursor.read_u32::<BigEndian>()),
            num_local_time_types:  try!(self.cursor.read_u32::<BigEndian>()),
            num_abbr_chars:        try!(self.cursor.read_u32::<BigEndian>()),
        })
    }

    fn read_transition_data(&mut self, count: usize) -> Result<Vec<TransitionData>> {
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

    fn read_octets(&mut self, count: usize) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(try!(self.cursor.read_u8()));
        }
        Ok(buf)
    }

    fn read_local_time_type_data(&mut self, count: usize) -> Result<Vec<LocalTimeTypeData>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(LocalTimeTypeData {
                offset:  try!(self.cursor.read_i32::<BigEndian>()),
                is_dst:  try!(self.cursor.read_u8()),
                name_offset: try!(self.cursor.read_u8()),
            });
        }
        Ok(buf)
    }

    fn read_leap_second_data(&mut self, count: usize) -> Result<Vec<LeapSecondData>> {
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


pub type Result<T> = result::Result<T, Box<error::Error>>;

#[derive(Debug)]
pub enum Error {
    InvalidMagicNumber {
        bytes_read: [u8; 4],
    },

    LimitReached {
        structures: Structures,
        intended_count: u32,
        limit: u32,
    }
}

#[derive(Debug)]
pub enum Structures {
    Transitions,
    LocalTimeTypes,
    LeapSeconds,
    GMTFlags,
    StandardFlags,
    TimezoneAbbrChars,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::InvalidMagicNumber { ref bytes_read } => {
                write!(f, "invalid magic number - got {:?}", bytes_read)
            },

            Error::LimitReached { ref structures, ref intended_count, ref limit } => {
                write!(f, "too many {} (tried to read {}, limit was {}", structures, intended_count, limit)
            },
        }
    }
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


impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::InvalidMagicNumber { .. }  => "invalid magic number",
            Error::LimitReached { .. }        => "limit reached",
        }
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
    pub gmt_flags: Vec<u8>,
}

pub fn parse(buf: Vec<u8>, limits: Limits) -> Result<TZData> {
    let mut parser = Parser::new(buf);
    try!(parser.read_magic_number());
    try!(parser.skip_initial_zeroes());

    let header = try!(parser.read_header());
    try!(limits.verify(&header));

    let transitions   = try!(parser.read_transition_data(header.num_transitions as usize));
    let time_types    = try!(parser.read_local_time_type_data(header.num_local_time_types as usize));
    let leap_seconds  = try!(parser.read_leap_second_data(header.num_leap_seconds as usize));
    let strings       = try!(parser.read_octets(header.num_abbr_chars as usize));
    let standards     = try!(parser.read_octets(header.num_standard_flags as usize));
    let gmts          = try!(parser.read_octets(header.num_gmt_flags as usize));

    Ok(TZData {
        header:          header,
        transitions:     transitions,
        time_info:       time_types,
        leap_seconds:    leap_seconds,
        strings:         strings,
        standard_flags:  standards,
        gmt_flags:       gmts,
    })
}
