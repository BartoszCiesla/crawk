use crate::reexports::Foo;

// --- Functions ---
pub fn pub_function() -> &'static str {
    "pub_function"
}

pub(crate) fn pub_crate_function() -> &'static str {
    "pub_crate_function"
}

pub(super) fn pub_super_function() -> &'static str {
    "pub_super_function"
}

fn private_function() -> &'static str {
    "private_function"
}

// --- Structs with mixed field visibility ---
#[derive(Default)]
pub struct PubStruct {
    pub pub_field: u32,
    pub(crate) pub_crate_field: u32,
    pub(super) pub_super_field: u32,
    private_field: u32,
}

pub(crate) struct PubCrateStruct {
    pub value: u32,
}

struct PrivateStruct {
    pub value: u32,
}

// --- Enums ---
pub enum PubEnum {
    VariantA,
    VariantB(u32),
    VariantC { x: i32, y: i32 },
}

pub(crate) enum PubCrateEnum {
    Alpha,
    Beta,
}

// --- Traits ---
pub trait PubTrait {
    fn required(&self) -> &str;

    fn provided(&self) -> String {
        format!("default: {}", self.required())
    }
}

pub(crate) trait PubCrateTrait {
    fn crate_only(&self) -> u32;
}

// --- Impl blocks with mixed method visibility ---
impl PubStruct {
    pub fn pub_method(&self) -> u32 {
        self.pub_field
    }

    pub(crate) fn pub_crate_method(&self) -> u32 {
        self.pub_crate_field
    }

    fn private_method(&self) -> u32 {
        self.private_field
    }
}

impl PubTrait for PubStruct {
    fn required(&self) -> &str {
        "PubStruct"
    }
}

impl PubCrateTrait for PubStruct {
    fn crate_only(&self) -> u32 {
        self.private_method()
    }
}

// --- Constants ---
pub const PUB_CONST: u32 = 100;
pub(crate) const PUB_CRATE_CONST: &str = "crate-visible";
const PRIVATE_CONST: u32 = 999;

// --- Statics ---
pub static PUB_STATIC: u32 = 200;
pub(crate) static PUB_CRATE_STATIC: &str = "crate-static";
static PRIVATE_STATIC: u32 = 0;

// --- Type aliases ---
pub type PubAlias = Vec<u32>;
pub(crate) type PubCrateAlias = Option<String>;
type PrivateAlias = Result<u32, String>;

// Exercise all items across every visibility level
pub fn exercise_all() -> String {
    // Functions: pub(crate), pub(super), private
    let f1 = pub_crate_function();
    let f2 = pub_super_function();
    let f3 = private_function();

    // Struct fields: pub(crate), pub(super), private (via methods)
    let s = PubStruct::default();
    let _ = s.pub_crate_field;
    let _ = s.pub_super_field;
    let _ = s.pub_crate_method();
    let _ = s.private_method();

    // pub(crate) struct
    let pcs = PubCrateStruct { value: 1 };

    // Private struct
    let ps = PrivateStruct { value: PRIVATE_CONST };

    // pub(crate) enum — construct both variants
    let _ = PubCrateEnum::Beta;
    let e = match PubCrateEnum::Alpha {
        PubCrateEnum::Alpha => "alpha",
        PubCrateEnum::Beta => "beta",
    };

    // pub(crate) trait
    let t = s.crate_only();

    // Constants and statics
    let _ = PUB_CRATE_CONST;
    let _ = PUB_CRATE_STATIC;
    let _ = PRIVATE_STATIC;

    // Type aliases
    let _: PubCrateAlias = None;
    let _: PrivateAlias = Ok(0);

    // Cross-module reference via use import
    let reexported = Foo::new();

    format!(
        "{f1}:{f2}:{f3}:{}:{}:{e}:{t}:{}",
        pcs.value, ps.value, reexported.value
    )
}
