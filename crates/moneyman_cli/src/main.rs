mod currency;

use std::path::PathBuf;

use chrono::NaiveDate;
use currency::Currency;
use moneyman::ConversionError;
use rust_decimal::Decimal;

use clap::{Command, Parser, Subcommand};
use rusty_money::{iso, Money};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Turn debugging information on
    #[arg(short, long)]
    debug: bool,

    #[command(subcommand)]
    commands: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Syncs historical data with the European Central Bank
    Sync {
        #[arg(short, long)]
        /// Don't do this unless you known the exchange store is messed up
        force: bool,
    },
    /// Convert one currency to another
    Convert {
        /// The amount of money you want to convert
        #[arg(value_name = "AMOUNT")]
        amount: Decimal,

        /// Source currency through ISO alpha code. e.g EUR
        #[arg(short, long, value_name = "CURRENCY")]
        from: Currency,
        /// Target currency through ISO alpha code. e.g USD
        #[arg(short, long, value_name = "CURRENCY")]
        to: Currency,
        /// Specify a specific date to convert. Will use the latest date in
        /// the exchange store if not specified. e.g 2023-05-05
        #[arg(long, value_name = "DATE")]
        on: Option<NaiveDate>,

        /// Where moneyman will save its local data store. Default: ~/.moneyman
        #[arg(long, value_name = "DIRECTORY_PATH")]
        data_dir: Option<PathBuf>,

        /// If this flag is presest, moneyman will interpolate missing rates
        /// based on the neighboring dates with rates.
        #[arg(long)]
        fallback: bool,
    },
}

const MONEYMAN: &str = "
 /$$      /$$  /$$$$$$  /$$   /$$ /$$$$$$$$ /$$     /$$ /$$      /$$  /$$$$$$  /$$   /$$
| $$$    /$$$ /$$__  $$| $$$ | $$| $$_____/|  $$   /$$/| $$$    /$$$ /$$__  $$| $$$ | $$
| $$$$  /$$$$| $$  \\ $$| $$$$| $$| $$       \\  $$ /$$/ | $$$$  /$$$$| $$  \\ $$| $$$$| $$
| $$ $$/$$ $$| $$  | $$| $$ $$ $$| $$$$$     \\  $$$$/  | $$ $$/$$ $$| $$$$$$$$| $$ $$ $$
| $$  $$$| $$| $$  | $$| $$  $$$$| $$__/      \\  $$/   | $$  $$$| $$| $$__  $$| $$  $$$$
| $$\\  $ | $$| $$  | $$| $$\\  $$$| $$          | $$    | $$\\  $ | $$| $$  | $$| $$\\  $$$
| $$ \\/  | $$|  $$$$$$/| $$ \\  $$| $$$$$$$$    | $$    | $$ \\/  | $$| $$  | $$| $$ \\  $$
|__/     |__/ \\______/ |__/  \\__/|________/    |__/    |__/     |__/|__/  |__/|__/  \\__/
";

fn print_result_no_fallback(
    from_amount: Money<iso::Currency>,
    converted_amount: Result<Money<iso::Currency>, ConversionError>,
    date: NaiveDate,
) {
    match converted_amount {
        Ok(money) => {
            println!(
                "{} {} -> {} {} on the date {}",
                from_amount.amount(),
                from_amount.currency(),
                money.amount(),
                money.currency(),
                date
            );
        }
        Err(ConversionError::MalformedExchangeStore) => {
            println!("The local data store may have been corrupted. You could try syncing it with `--force`.");
        }
        Err(ConversionError::NoExchangeRate(date)) => {
            println!(
                "No available rates on date {}. Some options:\n\n\t1. Sync with the latest ECB rates if you haven't already; or\n\t2. Use the --fallback flag to attempt to interpolate the rates",
                date
            );
        }
        Err(ConversionError::InvalidCurrency) => {
            println!("It's 1. ONEEEEEEEEEEEEEEEEEEEEE");
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let data_dir: PathBuf = dirs::home_dir()
        .map(|home_dir| home_dir.join(".moneyman"))
        .expect("need a home directory");

    let store = if !data_dir.join("eurofxref-hist.db3").exists() {
        moneyman::ExchangeStore::sync(data_dir).expect("failed ze sync")
    } else {
        moneyman::ExchangeStore::open(data_dir).expect("huh")
    };

    match cli.commands {
        // If the `--on` arg is specified
        Some(Commands::Convert {
            amount,
            from,
            to,
            on: Some(date),
            fallback: false,
            data_dir: None,
        }) => {
            let from_money = Money::from_decimal(amount, &from.0);
            let to_money = store.convert_on_date(from_money.clone(), &to.0, date);
            print_result_no_fallback(from_money, to_money, date);
        }
        // If the `--on` arg is not specified
        Some(Commands::Convert {
            amount,
            from,
            to,
            on: None,
            fallback: false,
            data_dir: None,
        }) => {
            let from_money = Money::from_decimal(amount, &from.0);

            match store.get_latest_date() {
                Some(date) => {
                    let to_money = store.convert_on_date(from_money.clone(), &to.0, date);
                    print_result_no_fallback(from_money, to_money, date);
                }
                None => {
                    println!("Unable to fetch the latest date from the local data store. Have you tried syncing it with ECB?");
                }
            }
        }
        Some(Commands::Convert {
            amount,
            from,
            to,
            on: Some(date),
            fallback: true,
            data_dir: None,
        }) => {
            let from_money = Money::from_decimal(amount, &from.0);
            let to_money = store.convert_on_date_with_fallback(from_money.clone(), &to.0, date);
            print_result_no_fallback(from_money, to_money, date);
        }

        Some(Commands::Convert {
            amount,
            from,
            to,
            on: None,
            fallback: true,
            data_dir: None,
        }) => {
            let from_money = Money::from_decimal(amount, &from.0);

            match store.get_latest_date() {
                Some(date) => {
                    let to_money =
                        store.convert_on_date_with_fallback(from_money.clone(), &to.0, date);
                    print_result_no_fallback(from_money, to_money, date);
                }
                None => {
                    println!("Unable to fetch the latest date from the local data store. Have you tried syncing it with ECB?");
                }
            }
        }

        _ => {
            let mut cmd = Command::new("moneyman");

            println!("{}", MONEYMAN);

            cmd.print_long_help().expect("Uh oh. I might've exploded.");
        }
    }
}
