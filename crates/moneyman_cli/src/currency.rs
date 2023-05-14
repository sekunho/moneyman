use clap::{
    builder::{TypedValueParser, ValueParserFactory},
    error::{ContextKind, ContextValue, ErrorKind},
    Arg, Command,
};
use rusty_money::iso;

#[derive(Clone, Debug)]
pub struct Currency(pub iso::Currency);

impl ValueParserFactory for Currency {
    type Parser = CurrencyParser;

    fn value_parser() -> Self::Parser {
        CurrencyParser
    }
}

#[derive(Clone, Debug)]
pub struct CurrencyParser;

impl TypedValueParser for CurrencyParser {
    type Value = Currency;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let curr_str = value.to_str().and_then(|str| Some(str.to_uppercase()));

        match curr_str {
            Some(str) => match iso::find(str.as_str()) {
                Some(currency) => Ok(Currency(*currency)),
                None => {
                    let mut err = clap::Error::new(ErrorKind::InvalidValue).with_cmd(cmd);

                    arg.and_then(|arg| {
                        arg.get_long().and_then(|name| {
                            err.insert(
                                ContextKind::InvalidArg,
                                ContextValue::String(format!("--{}", name.to_string())),
                            )
                        })
                    });

                    err.insert(
                        ContextKind::InvalidValue,
                        ContextValue::String(str.to_string()),
                    );

                    Err(err)
                }
            },
            None => {
                let err = clap::Error::new(ErrorKind::InvalidValue).with_cmd(cmd);

                Err(err)
            }
        }
    }
}
