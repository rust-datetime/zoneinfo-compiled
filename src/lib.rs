#![crate_name = "zoneinfo_compiled"]
#![crate_type = "rlib"]
#![crate_type = "dylib"]

//! This is a library for parsing compiled zoneinfo files.
//!
//! ## Example
//!
//! ```no_run
//! use std::fs::File;
//! use std::io::Read;
//! use std::path::Path;
//! use zoneinfo_compiled::parse;
//!
//! let path = Path::new("/etc/localtime");
//! let mut contents = Vec::new();
//! File::open(path).unwrap().read_to_end(&mut contents).unwrap();
//! let tz = parse(contents).unwrap();
//!
//! for t in tz.transitions {
//!     println!("{:?}", t);
//! }
//! ```

extern crate byteorder;
use std::sync::Arc;

pub mod parser;
pub use parser::Result;


/// Parsed, interpreted contents of a zoneinfo file.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct TZData {

    /// Vector of transitions that are described in this data.
    pub transitions: Vec<Transition>,

    /// Vector of leap seconds that are described in this data.
    pub leap_seconds: Vec<LeapSecond>,
}


/// The 'type' of time that the change was announced in.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TransitionType {

    /// Standard Time ("non-summer" time)
    Standard,

    /// Wall clock time
    Wall,

    /// Co-ordinated Universal Time
    UTC,
}


/// A time change specification.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Transition {

    /// Unix timestamp when the clocks change.
    pub timestamp: i32,

    /// The new description of the local time.
    pub local_time_type: Arc<LocalTimeType>,
}


/// A leap second specification.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LeapSecond {

    /// Unix timestamp at which a leap second occurs.
    pub timestamp: i32,

    /// Number of leap seconds to be added.
    pub leap_second_count: u32,
}


/// A description of the local time in a particular timezone, during the
/// period in which the clocks do not change.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LocalTimeType {

    /// The time zone abbreviation - such as "GMT" or "UTC".
    pub name: String,

    /// Number of seconds to be added to Universal Time.
    pub offset: i32,

    /// Whether to set DST.
    pub is_dst: bool,

    /// The current 'type' of time.
    pub transition_type: TransitionType,
}


/// Parses a series of bytes into a timezone data structure.
pub fn parse(input: Vec<u8>) -> Result<TZData> {
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
            offset:           ltt.offset,
            is_dst:           ltt.is_dst != 0,
            transition_type:  flags_to_transition_type(tz.standard_flags[i] != 0,
                                                       tz.gmt_flags[i]      != 0),
        };

        local_time_types.push(Arc::new(info));
    }

    // ...then, link each transition with the time type it refers to.
    for i in 0 .. tz.header.num_transitions as usize {
        let t = &tz.transitions[i];

        let transition = Transition {
            timestamp:        t.timestamp,
            local_time_type:  local_time_types[t.local_time_type_index as usize].clone(),
        };

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

    Ok(TZData {
        transitions: transitions,
        leap_seconds: leap_seconds,
    })
}


/// Combine the two flags to get the type of this transition.
///
/// The transition type is stored as two separate flags in the data file. The
/// first set comes completely before the second, so these can only be
/// combined after the entire file has been read.
fn flags_to_transition_type(standard: bool, gmt: bool) -> TransitionType {
    match (standard, gmt) {
        (_,     true)   => TransitionType::UTC,
        (true,  _)      => TransitionType::Standard,
        (false, false)  => TransitionType::Wall,
    }
}
