//! Node ids: six lowercase hex characters, generated at birth, never
//! derived from anything, never changed.

use std::collections::HashSet;

pub fn is_id(s: &str) -> bool {
    s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A fresh id not in `taken`; it is added to `taken` so a batch never
/// generates the same id twice.
pub fn generate(taken: &mut HashSet<String>) -> String {
    loop {
        let candidate = format!("{:06x}", fastrand::u32(..0x0100_0000));
        if taken.insert(candidate.clone()) {
            return candidate;
        }
    }
}
