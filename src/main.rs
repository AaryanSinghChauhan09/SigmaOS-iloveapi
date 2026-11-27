#![forbid(unsafe_code)]
use clap::{Parser, Subcommand};
#[derive(Parser)]
#[command(name = "iloveapi", about = "SigmaOS iLoveAPI client")]
struct Cli { #[command(subcommand)] cmd: Commands }
#[derive(Subcommand)]
enum Commands { List, Status }
fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::List   => println!("iloveapi: no entries configured."),
        Commands::Status => println!("iloveapi: ready."),
    }
}
