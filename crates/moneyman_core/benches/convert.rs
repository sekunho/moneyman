use std::path::PathBuf;

use chrono::NaiveDate;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use moneyman_core::convert_on_date;
use rust_decimal_macros::dec;
use rusty_money::{iso, Money};

pub fn non_indexed_convert_on_date(c: &mut Criterion) {
    let amount_in_usd = Money::from_decimal(dec!(6500), iso::USD);
    let date = NaiveDate::from_ymd_opt(1999, 1, 4).expect("ok date");
    let data_dir = PathBuf::new().join("..").join("..").join("test_data").join("non_indexed");

    c.bench_function("convert (non-indexed)", |b| {
        b.iter(|| {
            convert_on_date(
                black_box(&data_dir),
                amount_in_usd.clone(),
                black_box(iso::EUR),
                black_box(date),
            )
        })
    });
}

pub fn non_indexed_convert_on_date_non_euro(c: &mut Criterion) {
    let amount_in_usd = Money::from_decimal(dec!(6500), iso::USD);
    let date = NaiveDate::from_ymd_opt(1999, 1, 4).expect("ok date");
    let data_dir = PathBuf::new().join("..").join("..").join("test_data").join("non_indexed");

    c.bench_function("convert (non-indexed, non-euro)", |b| {
        b.iter(|| {
            convert_on_date(
                black_box(&data_dir),
                amount_in_usd.clone(),
                black_box(iso::JPY),
                black_box(date),
            )
        })
    });
}

pub fn indexed_convert_on_date(c: &mut Criterion) {
    let amount_in_usd = Money::from_decimal(dec!(6500), iso::USD);
    let date = NaiveDate::from_ymd_opt(1999, 1, 4).expect("ok date");
    let data_dir = PathBuf::new().join("..").join("..").join("test_data").join("indexed");

    c.bench_function("convert (indexed)", |b| {
        b.iter(|| {
            convert_on_date(
                black_box(&data_dir),
                amount_in_usd.clone(),
                black_box(iso::EUR),
                black_box(date),
            )
        })
    });
}

pub fn indexed_convert_on_date_non_euro(c: &mut Criterion) {
    let amount_in_usd = Money::from_decimal(dec!(6500), iso::USD);
    let date = NaiveDate::from_ymd_opt(1999, 1, 4).expect("ok date");
    let data_dir = PathBuf::new().join("..").join("..").join("test_data").join("indexed");

    c.bench_function("convert (indexed, non-euro)", |b| {
        b.iter(|| {
            convert_on_date(
                black_box(&data_dir),
                amount_in_usd.clone(),
                black_box(iso::JPY),
                black_box(date),
            )
        })
    });
}

criterion_group!(benches, non_indexed_convert_on_date, non_indexed_convert_on_date_non_euro, indexed_convert_on_date, indexed_convert_on_date_non_euro);
criterion_main!(benches);