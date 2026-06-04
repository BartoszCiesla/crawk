use crate::deep_a::DeepA;
use crate::leaf::Leaf;
use crate::leaf_via_a::LeafViaA;

pub struct A(pub Leaf, pub LeafViaA, pub DeepA);
