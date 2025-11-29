#![no_main]

use affs_read::{hash_name, names_equal};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Split data into two parts for name comparison
    let mid = data.len() / 2;
    let name1 = &data[..mid];
    let name2 = &data[mid..];

    // Fuzz hash_name
    let _ = hash_name(name1, false);
    let _ = hash_name(name1, true);
    let _ = hash_name(name2, false);
    let _ = hash_name(name2, true);

    // Fuzz names_equal
    let _ = names_equal(name1, name2, false);
    let _ = names_equal(name1, name2, true);
    let _ = names_equal(name1, name1, false);
    let _ = names_equal(name1, name1, true);
});
