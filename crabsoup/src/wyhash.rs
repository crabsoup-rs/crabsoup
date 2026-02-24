// we don't need a secret, and generating a secret involves primality checks. oww.
// thus, new_with_default_secret

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use wyrand::WyHash;

#[derive(Copy, Clone, Debug, Default)]
pub struct WyHashBuilder;
impl BuildHasher for WyHashBuilder {
    type Hasher = WyHash;
    fn build_hasher(&self) -> Self::Hasher {
        WyHash::new_with_default_secret(0xfc1abcacd1fc58fe)
    }
}

pub type WyHashSet<V> = HashSet<V, WyHashBuilder>;
pub type WyHashMap<K, V> = HashMap<K, V, WyHashBuilder>;
