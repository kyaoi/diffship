mod cli;
mod exit;
mod git;
mod ops;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();

    let code = match ops::dispatch(cli) {
        Ok(()) => exit::EXIT_OK,
        Err(e) => {
            // Keep stderr human-friendly but preserve context.
            eprintln!("{}", e);
            e.code
        }
    };

    std::process::exit(code);
}
