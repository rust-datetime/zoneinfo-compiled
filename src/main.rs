extern crate byteorder;
use byteorder::{ReadBytesExt, BigEndian};

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;


#[derive(Debug)]
struct Header {

    /// The version of this file's format - either '\0', or '2', or '3'.
    version: u8,

    /// The number of Universal Time entries in this file.
    /// (Equivalent to `tzh_ttisgmtcnt` in C)
    num_utc: u32,

    /// The number of standard entries in this file.
    /// (Equivalent to `tzh_ttisstdcnt` in C)
    num_standard: u32,

    /// The number of leap second entries in this file.
    /// (Equivalent to `tzh_leapcnt` in C)
    num_leap_seconds: u32,

    /// The number of transition entries in this file.
    /// (Equivalent to `tzh_timecnt` in C)
    num_transition_times: u32,

    /// The number of local time types in this file.
    /// (Equivalent to `tzh_typecnt` in C)
    num_time_types: u32,

    /// The number of characters of time zone abbreviation strings in this file.
    /// (Equivalent to `tzh_charcnt` in C)
    num_chars: u32,
}

#[derive(Debug)]
struct Transition {
    timestamp: u32,
    ttype: u8,
}

#[derive(Debug)]
enum TransitionType {
    Standard,
    Wall,
    UTC,
}

#[derive(Debug)]
struct PartialTimeInfo {

    /// Number of seconds to be added to Universal Time.
    /// (Equivalent to `tt_gmtoff` in C)
    offset: u32,

    /// Whether to set DST.
    /// (Equivalent to `tt_isdst` in C)
    is_dst: bool,

    /// Position in the array of time zone abbreviation characters, elsewhere
    /// in the file.
    /// (Equivalent to `tt_abbrind` in C)
    name_offset: u8,
}

#[derive(Debug)]
struct TimeInfo {
    partial: PartialTimeInfo,
    ttype: TransitionType,
}

#[derive(Debug)]
struct LeapSecondInfo {

    /// The time, as a number of seconds, at which a leap second occurs.
    timestamp: u32,

    /// Number of leap seconds to be added.
    leap_second_count: u32,
}

#[derive(Debug)]
struct TimezoneData {
    pub transitions:   Vec<Transition>,
    pub types:         Vec<TimeInfo>,
    pub leap_seconds:  Vec<LeapSecondInfo>,
}

/// Maximum number of transition times
static MAX_TIMES: usize = 1200;

/// Maximum number of TimeInfo settings
static MAX_TYPES: usize = 256;

/// Maximum number of bytes in timezone abbreviations
static MAX_CHARS: usize = 50;

/// Maximum number of leap second specifications
static MAX_LEAPS: usize = 50;

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

    fn read_transition_times(&mut self, count: usize) -> Result<Vec<Transition>, Box<Error>> {
        let mut times = Vec::with_capacity(count);
        for _ in 0 .. count {
            times.push(try!(self.cursor.read_u32::<BigEndian>()));
        }

        let mut types = Vec::with_capacity(count);
        for _ in 0 .. count {
            types.push(try!(self.cursor.read_u8()));
        }


        Ok(times.iter().zip(types.iter()).map(|(&ti, &ty)| {
            Transition {
                timestamp: ti,
                ttype:     ty,
            }
        }).collect())
     }

    fn read_transition_indices(&mut self, count: usize) -> Result<Vec<u8>, Box<Error>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(try!(self.cursor.read_u8()));
        }
        Ok(buf)
    }

    fn read_info_structures(&mut self, count: usize) -> Result<Vec<PartialTimeInfo>, Box<Error>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(PartialTimeInfo {
                offset:  try!(self.cursor.read_u32::<BigEndian>()),
                is_dst:  try!(self.cursor.read_u8()) != 0,
                name_offset: try!(self.cursor.read_u8()),
            });
        }
        Ok(buf)
    }

    fn read_leap_second_info(&mut self, count: usize) -> Result<Vec<LeapSecondInfo>, Box<Error>> {
        let mut buf = Vec::with_capacity(count);
        for _ in 0 .. count {
            buf.push(LeapSecondInfo {
                timestamp:          try!(self.cursor.read_u32::<BigEndian>()),
                leap_second_count:  try!(self.cursor.read_u32::<BigEndian>()),
            });
        }
        Ok(buf)
    }
}

fn main() {
    for arg in env::args().skip(1) {
        match File::open(&Path::new(&arg)) {
            Ok(mut file) => {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents);
                describe_file(contents);
            },
            Err(e) => println!("Couldn't open file {}: {}", arg, e),
        }
    }
}

fn describe_file(buf: Vec<u8>) {
    let mut parser = Parser::new(buf);
    let magic = parser.read_magic_number();
    println!("{:?}", magic);

    parser.skip_initial_zeroes();

    let header = parser.read_header().unwrap();
    println!("{:?}", header);

    let ts = parser.read_transition_times(header.num_transition_times as usize);
    println!("{:?}", ts);

    let tt = parser.read_info_structures(header.num_time_types as usize).unwrap();

    let ls = parser.read_leap_second_info(header.num_leap_seconds as usize);
    println!("ls: {:?}", ls);

    let i = parser.read_transition_indices(header.num_chars as usize);
    println!("i: {:?}", i);

    let standards = parser.read_transition_indices(header.num_standard as usize).unwrap();
    let utcs = parser.read_transition_indices(header.num_utc as usize).unwrap();

    let types: Vec<_> = standards.iter()
                                 .zip(utcs.iter())
                                 .map(|(&a, &b)| bools(a != 0, b != 0))
                                 .zip(tt.into_iter())
                                 .map(|(t, i)| TimeInfo { partial: i, ttype: t })
                                 .collect();

    println!("{:?}", types);
}

fn bools(a: bool, b: bool) -> TransitionType {
    match (a, b) {
        (_,     true)   => TransitionType::UTC,
        (true,  _)      => TransitionType::Standard,
        (false, false)  => TransitionType::Wall,
    }
}
