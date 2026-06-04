#![allow(dead_code)]

pub mod a;
pub mod b;
pub mod deep;
pub mod deep_a;
pub mod deep_b;
pub mod leaf;
pub mod leaf_via_a;

use crate::a::A;
use crate::b::B;

pub fn entry(_a: A, _b: B) {}
