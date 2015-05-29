extern crate byteorder;
use std::rc::Rc;

pub mod internals;

#[derive(Debug, PartialEq, Eq, Clone)]
enum TimeType {
    Standard,
    Wall,
    UTC,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Transition {
    timestamp: u32,
    info: Rc<TimeInfo>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LeapSecond {
    timestamp: u32,
    leap_second_count: u32,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TimeInfo {
    name: String,
    offset: u32,
    is_dst: bool,
    ttype: TimeType,
}

/// Maximum number of transition times
static MAX_TIMES: usize = 1200;

/// Maximum number of TimeInfo settings
static MAX_TYPES: usize = 256;

/// Maximum number of bytes in timezone abbreviations
static MAX_CHARS: usize = 50;

/// Maximum number of leap second specifications
static MAX_LEAPS: usize = 50;

pub fn cook(tz: internals::TZData) -> Option<Vec<Transition>> {
    let mut transitions = Vec::new();
    let mut infos = Vec::new();

    for i in 0 .. tz.header.num_time_types as usize {
        let ref data = tz.time_info[i];

        let name_bytes = tz.strings.iter()
                                   .cloned()
                                   .skip(data.name_offset as usize)
                                   .take_while(|&c| c != 0)
                                   .collect();

        let info = TimeInfo {
            name: String::from_utf8(name_bytes).unwrap(),
            offset: data.offset,
            is_dst: data.is_dst != 0,
            ttype: bools(tz.standard_flags[i] != 0, tz.utc_flags[i] != 0),
        };

        infos.push(Rc::new(info));
    }

    for i in 0 .. tz.header.num_transition_times as usize {
        let ref t = tz.transitions[i];
        let transition = Transition {
            timestamp: t.timestamp,
            info: infos[t.ttype as usize].clone(),
        };

        transitions.push(transition);
    }

    Some(transitions)
}

fn bools(a: bool, b: bool) -> TimeType {
    match (a, b) {
        (_,     true)   => TimeType::UTC,
        (true,  _)      => TimeType::Standard,
        (false, false)  => TimeType::Wall,
    }
}
