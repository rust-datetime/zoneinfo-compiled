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
                file.read_to_end(&mut contents);
                match internals::parse(contents) {
                    Ok(tzdata) => println!("{:?}", tz::cook(tzdata)),
                    Err(e)     => println!("{}", e),
                }
            },
            Err(e) => println!("Couldn't open file {}: {}", arg, e),
        }
    }
}
