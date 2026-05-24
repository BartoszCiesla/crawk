pub struct Config {
    pub name: String,
}

impl Config {
    pub fn new() -> Self {
        Self {
            name: String::from("default"),
        }
    }
}
