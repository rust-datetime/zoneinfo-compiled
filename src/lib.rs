#![warn(missing_copy_implementations)]
//#![warn(missing_docs)]
#![warn(nonstandard_style)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unused)]

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

        let f = File::open(path)?;
        let mut r = BufReader::new(f);
        let mut contents: Vec<u8> = Vec::new();

        r.read_to_end(&mut contents)?;
        let tz = Self::parse(contents)?;
        Ok(tz)
    }
}

impl CompiledData for TimeZone {
    fn parse(input: Vec<u8>) -> Result<TimeZone> {
        let data = parse(input)?;
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
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct LeapSecond {

    /// Unix timestamp at which a leap second occurs.
    pub timestamp: i32,

    /// Number of leap seconds to be added.
    pub leap_second_count: i32,
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

impl LocalTimeType {

    /// Convert this set of fields into datetime’s `FixedTimespan`
    /// representation.
    ///
    /// It doesn’t actually contain any `'static` data, but if the lifetime is
    /// not specified, Rust ties its lifetime to `self`, when they’re actually
    /// completely unrelated.
    fn to_fixed_timespan(&self) -> FixedTimespan<'static> {
        FixedTimespan {
            offset: self.offset,
            is_dst: self.is_dst,
            name: Cow::Owned(self.name.clone()),
        }
    }
}


/// Parses a series of bytes into a timezone data structure.
pub fn parse(input: Vec<u8>) -> Result<TZData> {
    let tz = parser::parse(input, parser::Limits::sensible())?;
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

        // (TODO: move to ‘copied’ when the library supports that Rust version)
        let std_flag = tz.standard_flags.get(i).cloned().unwrap_or_default() != 0;
        let gmt_flag = tz.gmt_flags.get(i).cloned().unwrap_or_default() != 0;

        let info = LocalTimeType {
            name:             String::from_utf8(name_bytes)?,
            offset:           ltt.offset as i64,
            is_dst:           ltt.is_dst != 0,
            transition_type:  flags_to_transition_type(std_flag, gmt_flag),
        };

        local_time_types.push(info);
    }

    // ...then, link each transition with the time type it refers to.
    for i in 0 .. tz.header.num_transitions as usize {
        let t = &tz.transitions[i];
        let ltt = local_time_types[t.local_time_type_index as usize].clone();
        let timespan = ltt.to_fixed_timespan();

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

    // The `OwnedTimeZone` struct *requires* there to be at least one
    // transition. If there aren’t any in the file, we need to reach back into
    // the structure to get the *base* offset time, as it won’t be in the
    // transitions list.

    if transitions.is_empty() {
        let time_zone = OwnedTimeZone {
            name: None,
            fixed_timespans: OwnedFixedTimespanSet {
                first: local_time_types[0].to_fixed_timespan(),
                rest: Vec::new(),
            },
        };

        Ok(TZData { time_zone, leap_seconds })
    }
    else {
        // We don’t care about the timestamp that the first transition happens
        // at: we assume it to have been in effect forever.
        let first = transitions.remove(0);
        let time_zone = OwnedTimeZone {
            name: None,
            fixed_timespans: OwnedFixedTimespanSet {
                first: first.1,
                rest: transitions,
            }
        };

        Ok(TZData { time_zone, leap_seconds })
    }
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
