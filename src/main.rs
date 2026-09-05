mod compiler;
mod defaultfilename;
mod linker;
mod out;
mod packages;

use clap::{Parser, Subcommand};
use std::fs::{File, create_dir};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::defaultfilename::get;
use crate::out::ResultType;

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

        #[arg(short = 'O', long)]
        optimize: bool,

        #[arg(short = 'n', long)]
        noruntime: bool,

        #[arg(short = 'l', long = "link", value_name = "FILES", num_args = 1..)]
        link_files: Vec<PathBuf>,

        #[arg(short = 'I', long = "include", value_name = "DIR")]
        include: Vec<PathBuf>,

        #[arg(short = 'o', long = "output", default_value = "@")]
        output: PathBuf,

        #[arg(
            short = 'r',
            long = "result",
            value_name = "RESULT",
            default_value = "binary"
        )]
        result: ResultType,
    },

    Run {
        #[arg(value_name = "FILE")]
        input: PathBuf,

        #[arg(short = 'I', long = "include", value_name = "DIR")]
        include: Vec<PathBuf>,
    },

    Check {
        #[arg(value_name = "FILE")]
        input: PathBuf,

        #[arg(short = 'I', long = "include", value_name = "DIR")]
        include: Vec<PathBuf>,
    },

    Install {
        #[arg(value_name = "PACK_NAME")]
        package: String,
    },

    Init {
        #[arg(value_name = "project name")]
        projname: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let start_time = Instant::now();

    let mut outp = true;

    #[allow(clippy::cmp_owned)]
    let result = match cli.command {
        Commands::Build {
            input,
            output,
            optimize,
            noruntime,
            link_files,
            include,
            result,
        } => {
            let output: Result<PathBuf, anyhow::Error> = if output == PathBuf::from("@") {
                if input.len() != 1 {
                    Err(anyhow::anyhow!(
                        "Cannot infer output filename when compiling multiple input files; use -o"
                    ))
                } else {
                    input[0]
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Input filename is not valid UTF-8"))
                        .map(|input| get(input, result).into())
                }
            } else {
                Ok(output)
            };

            output
                .and_then(|output| {
                    compiler::Pipeline::new(
                        input, output, optimize, noruntime, link_files, include, result,
                    )
                })
                .and_then(|pipeline| pipeline.compile())
        }
        Commands::Run { input, include } => compiler::Pipeline::run_ephemeral(input, include),

        Commands::Install { package } => packages::install_package(
            &package,
            &packages::DependencySource::Named(package.clone()),
        ),

        Commands::Init { projname } => initialise(projname),

        Commands::Check { input, include } => {
            outp = false;
            compiler::Pipeline::check(input, include)
        }
    };

    if outp {
        if let Err(err) = result {
            eprintln!("\x1b[1;31mError:\x1b[0m {:?}", err);
            std::process::exit(1);
        }

        println!(
            "\x1b[1;32mFinished\x1b[0m task in {:.2?}",
            start_time.elapsed()
        );
    } else if let Err(err) = result {
        eprintln!("{:?}", err);
        std::process::exit(1);
    }
}

fn initialise(projname: Option<String>) -> Result<(), anyhow::Error> {
    let mut base_dir = PathBuf::from(".");

    if let Some(projname) = projname {
        base_dir.push(projname);
        create_dir(&base_dir)?;
    }

    let mut mainmysz = File::create(base_dir.join("main.mysz"))?;
    let mut nibbletoml = File::create(base_dir.join("nibble.toml"))?;

    let mainmysz_content = r#"use std::io;

fn pub main(): int {
    str_print("Hello, world!");
    return 0;
};"#;

    mainmysz.write_all(mainmysz_content.as_bytes())?;

    let nibbletoml_content = r#"[compiler]
target = "cranelift"
output_json = false

[dependencies]
std = "std"
"#;

    nibbletoml.write_all(nibbletoml_content.as_bytes())?;

    Ok(())
}
