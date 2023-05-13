pub(crate) mod exchange_rate;
pub(crate) mod fallback;
pub(crate) mod seed;

#[cfg(test)]
mod tests {
    // use rust_decimal_macros::dec;
    // use rusty_money::{iso, ExchangeRate};

    // #[test]
    // fn it_parses_rate_into_bidirectional_rates() {
    //     let (rate1, rate2) = parse_rate(iso::USD, "1.1037".to_string());
    //     let expected1 = ExchangeRate::new(iso::USD, iso::EUR, dec!(1) / dec!(1.1037)).unwrap();
    //     let expected2 = ExchangeRate::new(iso::EUR, iso::USD, dec!(1.1037)).unwrap();

    //     assert_eq!(rate1, expected1);
    //     assert_eq!(rate2, expected2);
    // }

    // #[test]
    // fn it_panics_if_rate_is_invalid_when_parsing() {
    //     let result = std::panic::catch_unwind(|| parse_rate(iso::USD, "1a.1037".to_string()));

    //     assert!(result.is_err());
    // }
}
