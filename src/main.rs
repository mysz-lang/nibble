mod compiler;
mod linker;
mod out;
mod packages;

use clap::{Parser, Subcommand};
use std::fs::{File, create_dir};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::out::ResultType;

#[derive(Parser)]
#[command(name = "nibble", version, author, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the @ in the current directory
    Build {
        #[arg(short = 'O', long)]
        optimize: bool,

        #[arg(short = 'n', long)]
        noruntime: bool,

        #[arg(short = 'l', long = "link", value_name = "FILES", num_args = 1..)]
        link_files: Vec<PathBuf>,

        #[arg(short = 'I', long = "include", value_name = "DIR")]
        include: Vec<PathBuf>,

        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,

        #[arg(
            short = 'r',
            long = "result",
            value_name = "RESULT",
            default_value = "binary"
        )]
        result: ResultType,
    },

    /// Build and immediately run the @ in the current directory.
    Run {
        #[arg(short = 'I', long = "include", value_name = "DIR")]
        include: Vec<PathBuf>,
    },

    /// List all @s in the project and show which are not cached.
    List {},

    /// Type-check the @ in the current directory without producing output.
    Check {
        #[arg(short = 'I', long = "include", value_name = "DIR")]
        include: Vec<PathBuf>,
    },

    /// Add an @ dependency to manifest.nibble.
    Install {
        #[arg(value_name = "ALIAS")]
        alias: String,

        #[arg(long = "at", value_name = "AT_NAME")]
        at: Option<String>,

        #[arg(long = "version", value_name = "VERSION")]
        version: Option<String>,

        #[arg(long = "source", value_name = "SOURCE")]
        source: Option<String>,
    },

    /// Initialise a new Mysz @ project.
    Init {
        #[arg(value_name = "PROJECT_NAME")]
        projname: Option<String>,
    },

    /// Remove all generated Nibble output and cached @s.
    Clean,
}

fn main() {
    let cli = Cli::parse();
    let start_time = Instant::now();

    let mut outp = true;

    let result = match cli.command {
        Commands::Build {
            optimize,
            noruntime,
            link_files,
            include,
            output,
            result,
        } => compiler::Pipeline::new(
            output,
            optimize,
            noruntime,
            link_files,
            include,
            result,
        )
        .and_then(|pipeline| pipeline.compile()),

        Commands::Run { include } => compiler::Pipeline::run_ephemeral(include),

        Commands::Install {
            alias,
            at,
            version,
            source,
        } => {
            let dep = packages::Dependency {
                at: at.unwrap_or_else(|| alias.clone()),
                alias,
                version,
                source,
            };

            packages::add_dependency_to_manifest(&dep)
        }

        Commands::List {} => {
            outp = false;
            packages::list()
        }

        Commands::Init { projname } => initialise(projname),

        Commands::Check { include } => {
            outp = false;
            compiler::Pipeline::check(include)
        }

        Commands::Clean => {
            packages::clean()
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

    let name = if let Some(projname) = &projname {
        base_dir.push(projname);
        create_dir(&base_dir)?;
        projname.clone()
    } else {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "main".to_string())
    };

    let src_dir = base_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    let mut mainmysz = File::create(src_dir.join("main.mysz"))?;
    let mut manifest = File::create(base_dir.join("manifest.nibble"))?;
    let mut gitignore = File::create(base_dir.join(".gitignore"))?;

    let mainmysz_content = r#"use std::io;

fn pub main(): int {
    println("Hello, world!");
    return 0;
};"#;

    mainmysz.write_all(mainmysz_content.as_bytes())?;

    let manifest_content = format!(
        r#"at name="{name}" entry="src/main.mysz" version="0.1.0" description="" {{
    author ""
}}

compiler {{
    target "cranelift"
}}

dependencies {{
    std version="0.3.5"
}}
"#,
        name = name,
    );

    manifest.write_all(manifest_content.as_bytes())?;
    gitignore.write_all(packages::GITIGNORE_CONTENT.as_bytes())?;

    Ok(())
}