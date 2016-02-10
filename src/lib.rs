#![crate_name = "zoneinfo_compiled"]
#![crate_type = "rlib"]
#![crate_type = "dylib"]

//! This is a library for parsing compiled zoneinfo files.

use std::borrow::Cow;
use std::convert::AsRef;
use std::path::Path;
use std::sync::Arc;

extern crate byteorder;
extern crate datetime;
use datetime::zone::{TimeZone, TimeType, TimeZoneSource, FixedTimespan};
use datetime::zone::runtime::{OwnedTimeZone, OwnedFixedTimespanSet};

pub mod parser;
pub use parser::Result;

pub trait CompiledData {
    fn parse(input: Vec<u8>) -> Result<TimeZone>;

    fn from_file<P: AsRef<Path>>(path: P) -> Result<TimeZone> {
        use std::io::{Read, BufReader};
        use std::fs::File;

        let f = try!(File::open(path));
        let mut r = BufReader::new(f);
        let mut contents: Vec<u8> = Vec::new();

        try!(r.read_to_end(&mut contents));
        let tz = try!(Self::parse(contents));
        Ok(tz)
    }
}

impl CompiledData for TimeZone {
    fn parse(input: Vec<u8>) -> Result<TimeZone> {
        let data = try!(parse(input));
        let arc = Arc::new(data.time_zone);
        let tz = TimeZone(TimeZoneSource::Runtime(arc));
        Ok(tz)
    }
}


/// Parsed, interpreted contents of a zoneinfo file.
#[derive(PartialEq, Debug)]
pub struct TZData {

    /// Vector of transitions that are described in this data.
    pub time_zone: OwnedTimeZone,

    /// Vector of leap seconds that are described in this data.
    pub leap_seconds: Vec<LeapSecond>,
}


/// A leap second specification.
#[derive(Debug, PartialEq)]
pub struct LeapSecond {

    /// Unix timestamp at which a leap second occurs.
    pub timestamp: i32,

    /// Number of leap seconds to be added.
    pub leap_second_count: u32,
}


/// A description of the local time in a particular timezone, during the
/// period in which the clocks do not change.
#[derive(Debug, PartialEq, Clone)]
pub struct LocalTimeType {

    /// The time zone abbreviation - such as "GMT" or "UTC".
    pub name: String,

    /// Number of seconds to be added to Universal Time.
    pub offset: i64,

    /// Whether to set DST.
    pub is_dst: bool,

    /// The current 'type' of time.
    pub transition_type: TimeType,
}


/// Parses a series of bytes into a timezone data structure.
pub fn parse(input: Vec<u8>) -> Result<TZData> {
    println!("{:?}", input);
    let tz = try!(parser::parse(input, parser::Limits::sensible()));
    cook(tz)
}


/// Interpret a set of internal time zone data.
pub fn cook(tz: parser::TZData) -> Result<TZData> {
    let mut transitions = Vec::with_capacity(tz.header.num_transitions as usize);
    let mut local_time_types = Vec::with_capacity(tz.header.num_local_time_types as usize);

    // First, build up a list of local time types...
    for i in 0 .. tz.header.num_local_time_types as usize {
        let ltt = &tz.time_info[i];

        // Isolate the relevant bytes by the index of the start of the
        // string and the next available null char
        let name_bytes = tz.strings.iter()
                                   .cloned()
                                   .skip(ltt.name_offset as usize)
                                   .take_while(|&c| c != 0)
                                   .collect();

        let info = LocalTimeType {
            name:             try!(String::from_utf8(name_bytes)),
            offset:           ltt.offset as i64,
            is_dst:           ltt.is_dst != 0,
            transition_type:  flags_to_transition_type(tz.standard_flags[i] != 0,
                                                       tz.gmt_flags[i]      != 0),
        };

        local_time_types.push(info);
    }

    // ...then, link each transition with the time type it refers to.
    for i in 0 .. tz.header.num_transitions as usize {
        let t = &tz.transitions[i];
        let ltt = local_time_types[t.local_time_type_index as usize].clone();

        let timespan = FixedTimespan {
            offset: ltt.offset,
            is_dst: ltt.is_dst,
            name: Cow::Owned(ltt.name),
        };

        let transition = (t.timestamp as i64, timespan);
        transitions.push(transition);
    }

    let mut leap_seconds = Vec::new();
    for ls in &tz.leap_seconds {
        let leap_second = LeapSecond {
            timestamp: ls.timestamp,
            leap_second_count: ls.leap_second_count,
        };

        leap_seconds.push(leap_second);
    }

    if transitions.is_empty() {
        return Err(Box::new(parser::Error::NoTransitions));
    }

    let first = transitions.remove(0);

    Ok(TZData {
        time_zone: OwnedTimeZone {
            name: None,
            fixed_timespans: OwnedFixedTimespanSet {
                first: first.1,
                rest: transitions,
            }
        },
        leap_seconds: leap_seconds,
    })
}


/// Combine the two flags to get the type of this transition.
///
/// The transition type is stored as two separate flags in the data file. The
/// first set comes completely before the second, so these can only be
/// combined after the entire file has been read.
fn flags_to_transition_type(standard: bool, gmt: bool) -> TimeType {
    match (standard, gmt) {
        (_,     true)   => TimeType::UTC,
        (true,  _)      => TimeType::Standard,
        (false, false)  => TimeType::Wall,
    }
}
