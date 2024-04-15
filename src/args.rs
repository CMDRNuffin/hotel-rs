use clap::Parser;
use std::path::PathBuf;

/// Sorts rooms based on cleaning time to minimize waiting time for guests
#[derive(Parser)]
pub struct Args {
    /// The number of cleaning crews employed by the hotel.
    #[arg(long, short, default_value_t = 1)]
    pub cleaning_crews: i32,

    /// Automatically hire more cleaning crews to meet demand
    #[arg(long, short = 'C')]
    pub hire_crews: bool,

    /// The input json file
    #[arg()]
    pub json_path: PathBuf,

    /// Makes guest arrivals predictable by starting the RNG on a fixed value. Can be any string.
    /// Note that providing an empty string will cause the program to behave as if no seed was provided at all.
    #[arg(long, short)]
    pub seed: Option<String>
}
