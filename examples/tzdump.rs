extern crate tz;
use tz::internals;

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;


fn main() {
    for arg in env::args().skip(1) {
        match File::open(&Path::new(&arg)) {
            Ok(mut file) => {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).unwrap();
                match internals::parse(contents, internals::Limits::sensible()) {
                    Ok(tzdata) => tzdump(tz::cook(tzdata).unwrap()),
                    Err(e)     => println!("{}", e),
                }
            },
            Err(e) => println!("Couldn't open file {}: {}", arg, e),
        }
    }
}

fn tzdump(mut transitions: Vec<tz::Transition>) {
    transitions.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    for t in transitions {
        let l = &*t.local_time_type;
        println!("{:10?}: name:{:5} offset:{:5} DST:{:5} type:{:?}",
                  t.timestamp, l.name, l.offset, l.is_dst, l.transition_type);
    }
}
