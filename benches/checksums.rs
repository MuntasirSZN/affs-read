//! Benchmarks for checksum calculations.

use affs_read::{bitmap_sum, boot_sum, normal_sum_slice};

fn main() {
    divan::main();
}

#[divan::bench]
fn bench_normal_sum_512(bencher: divan::Bencher) {
    let buf = [0u8; 512];
    bencher.bench_local(|| divan::black_box(normal_sum_slice(divan::black_box(&buf), 20)));
}

#[divan::bench]
fn bench_boot_sum(bencher: divan::Bencher) {
    let buf = [0u8; 1024];
    bencher.bench_local(|| divan::black_box(boot_sum(divan::black_box(&buf))));
}

#[divan::bench]
fn bench_bitmap_sum(bencher: divan::Bencher) {
    let buf = [0u8; 512];
    bencher.bench_local(|| divan::black_box(bitmap_sum(divan::black_box(&buf))));
}

#[divan::bench]
fn bench_normal_sum_varied_data(bencher: divan::Bencher) {
    let mut buf = [0u8; 512];
    // Fill with varied data to prevent optimization
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 256) as u8;
    }
    bencher.bench_local(|| divan::black_box(normal_sum_slice(divan::black_box(&buf), 20)));
}

#[divan::bench]
fn bench_boot_sum_varied_data(bencher: divan::Bencher) {
    let mut buf = [0u8; 1024];
    // Fill with varied data
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((i * 7 + 13) % 256) as u8;
    }
    bencher.bench_local(|| divan::black_box(boot_sum(divan::black_box(&buf))));
}
