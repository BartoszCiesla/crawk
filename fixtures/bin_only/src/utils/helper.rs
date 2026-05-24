use crate::config::Config;

pub fn run(cfg: &Config) {
    println!("Running with config: {}", cfg.name);
}
