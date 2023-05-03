use polars::{export::chrono::NaiveDate, lazy::dsl::{col, lit}, prelude::*};

pub mod blocking;

pub fn convert() {}

pub fn get_rate(
    _from: rusty_money::iso::Currency,
    _to: rusty_money::iso::Currency,
    on_date: NaiveDate,
) {
    let date_str = on_date.to_string();

    let options = StrpTimeOptions {
        // https://docs.python.org/3/library/datetime.html#strftime-and-strptime-format-codes
        fmt: Some("%Y-%m-%d".into()),
        date_dtype: DataType::Date,
        tz_aware: false,
        utc: false,
        exact: true,
        strict: true,
        cache: true,
    };

    println!("{}", date_str.as_str());
    let history_path = std::path::Path::new("eurofxref-hist.csv");
    let df = LazyCsvReader::new(history_path)
        .has_header(true)
        .with_null_values(Some(NullValues::AllColumnsSingle(String::from("N/A"))))
        .finish()
        .expect("unsuccessful read")
        .with_column(col("Date").str().strptime(options.clone()))
        .filter(col("Date").eq(lit(on_date)))
        .collect()
        .expect("hey");

    println!("{}", df);
}
