mod support;

#[test]
fn shapes_are_constructible() {
    let _ = support::square();
}

// Reaches the library through qualified paths only, never a `use` item: a
// type annotation, a value, a call and a macro argument.
#[test]
fn circle_is_drawn_under_its_label() {
    let circle: hyphenated_pkg::shapes::Circle = hyphenated_pkg::Circle;
    hyphenated_pkg::render::draw(&circle);
    assert_eq!(hyphenated_pkg::render::LABEL, "circle");
}
