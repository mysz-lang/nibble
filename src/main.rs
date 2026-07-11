mod compiler;
mod linker;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "nibble", version, author, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        #[arg(value_name = "FILE")]
        input: Vec<PathBuf>,

        #[arg(short, long, default_value = "main")]
        output: PathBuf,

        #[arg(short = 'O', long)]
        optimize: bool,

        #[arg(short = 'n', long)]
        noruntime: bool,

        #[arg(short = 'l', long = "link", value_name = "FILES", num_args = 1..)]
        link_files: Vec<PathBuf>,

        #[arg(short = 'I', long = "include", value_name = "DIR")]
        include: Vec<PathBuf>,
    },
    Run {
        #[arg(value_name = "FILE")]
        input: PathBuf,

        #[arg(short = 'I', long = "include", value_name = "DIR")]
        include: Vec<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    let start_time = Instant::now();

    let result = match cli.command {
        Commands::Build {
            input,
            output,
            optimize,
            noruntime,
            link_files,
            include,
        } => compiler::Pipeline::new(input, output, optimize, noruntime, link_files, include)
            .compile(),
        Commands::Run { input, include } => compiler::Pipeline::run_ephemeral(input, include),
    };

    if let Err(err) = result {
        eprintln!("\x1b[1;31mError:\x1b[0m {:?}", err);
        std::process::exit(1);
    }

    println!(
        "\x1b[1;32mFinished\x1b[0m task in {:.2?}",
        start_time.elapsed()
    );
}
