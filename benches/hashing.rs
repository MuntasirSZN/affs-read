//! Benchmarks for name hashing and comparison.

use affs_read::{hash_name, names_equal};

fn main() {
    divan::main();
}

#[divan::bench]
fn bench_hash_name_short_ascii(bencher: divan::Bencher) {
    let name = b"test";
    bencher.bench_local(|| divan::black_box(hash_name(divan::black_box(name), false)));
}

#[divan::bench]
fn bench_hash_name_long_ascii(bencher: divan::Bencher) {
    let name = b"very_long_filename_test.txt";
    bencher.bench_local(|| divan::black_box(hash_name(divan::black_box(name), false)));
}

#[divan::bench]
fn bench_hash_name_short_intl(bencher: divan::Bencher) {
    let name = b"test";
    bencher.bench_local(|| divan::black_box(hash_name(divan::black_box(name), true)));
}

#[divan::bench]
fn bench_hash_name_long_intl(bencher: divan::Bencher) {
    let name = b"very_long_filename_test.txt";
    bencher.bench_local(|| divan::black_box(hash_name(divan::black_box(name), true)));
}

#[divan::bench]
fn bench_names_equal_short_match_ascii(bencher: divan::Bencher) {
    let a = b"test";
    let b = b"TEST";
    bencher.bench_local(|| {
        divan::black_box(names_equal(divan::black_box(a), divan::black_box(b), false))
    });
}

#[divan::bench]
fn bench_names_equal_long_match_ascii(bencher: divan::Bencher) {
    let a = b"very_long_filename_test.txt";
    let b = b"VERY_LONG_FILENAME_TEST.TXT";
    bencher.bench_local(|| {
        divan::black_box(names_equal(divan::black_box(a), divan::black_box(b), false))
    });
}

#[divan::bench]
fn bench_names_equal_short_nomatch_ascii(bencher: divan::Bencher) {
    let a = b"test";
    let b = b"fail";
    bencher.bench_local(|| {
        divan::black_box(names_equal(divan::black_box(a), divan::black_box(b), false))
    });
}

#[divan::bench]
fn bench_names_equal_length_mismatch(bencher: divan::Bencher) {
    let a = b"test";
    let b = b"testing";
    bencher.bench_local(|| {
        divan::black_box(names_equal(divan::black_box(a), divan::black_box(b), false))
    });
}

#[divan::bench]
fn bench_names_equal_intl_match(bencher: divan::Bencher) {
    let a = b"test";
    let b = b"TEST";
    bencher.bench_local(|| {
        divan::black_box(names_equal(divan::black_box(a), divan::black_box(b), true))
    });
}
