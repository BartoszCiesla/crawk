use hyphenated_pkg::Circle;
use hyphenated_pkg::render;
use hyphenated_pkg::shapes::*;

fn main() {
    let circle = Circle;
    let _ = Square;
    println!("{}", render::LABEL);
    render::draw(&circle);
}
