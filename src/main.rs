use std::error::Error;

use skulpin::*;
use skulpin::app::*;

mod netcanv;
use netcanv::*;

fn main() {
    NetCanv::build();
}
