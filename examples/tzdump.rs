extern crate zoneinfo_compiled;

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;


// This example is broken until we have a way to get at the transitions in
// time zone data directly. Right now it just does a Rust Debug dump of the
// file...

fn main() {
    for arg in env::args().skip(1) {
        match File::open(&Path::new(&arg)) {
            Ok(mut file) => {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).unwrap();
                match zoneinfo_compiled::parse(contents) {
                    Ok(tzdata) => println!("{:#?}", tzdata),
                    Err(e)     => println!("Error: {}", e),
                }
            },
            Err(e) => println!("Couldn't open file {}: {}", arg, e),
        }
    }
}

// fn tzdump(mut tz: zoneinfo_compiled::TZData) {
//     tz.transitions.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
//
//     for t in tz.transitions {
//         let l = &*t.local_time_type;
//         println!("{:11?}: name:{:5} offset:{:5} DST:{:5} type:{:?}",
//                   t.timestamp, l.name, l.offset, l.is_dst, l.transition_type);
//     }
// }
