mod config;
mod utils;

use crate::config::Config;
use crate::utils::helper::run;

fn main() {
    let cfg = Config::new();
    run(&cfg);
}
