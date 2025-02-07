mod unpacked_gaussian;
use unpacked_gaussian::*;
mod spz;
use spz::*;
mod support;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Subcommand)]
enum Commands {
    /// Convert a .ply file to a .spz file
    Encode {
        #[arg(value_name = "INPUT")]
        /// The input .ply file
        input: PathBuf,

        #[arg(value_name = "OUTPUT")]
        /// The output .spz file
        output: PathBuf,

        #[arg(short, long)]
        /// Do not compress the output. This option will not produce a valid .spz file.
        uncompressed: bool,
    },

    /// Convert a .spz file to a .ply file
    Decode {
        #[arg(value_name = "INPUT")]
        /// The input .spz file
        input: PathBuf,

        #[arg(value_name = "OUTPUT")]
        /// The output .ply file
        output: PathBuf,

        #[arg(short, long, default_value = "false")]
        /// Do not decompress the input.
        uncompressed: bool,
    },
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Encode {
            input,
            output,
            uncompressed,
        } => {
            let gaussians = load_ply(&input).unwrap();
            if gaussians.len() == 1 {
                println!("{:?}", gaussians[0]);
            }
            write_spz(gaussians, &output, !uncompressed).unwrap();
        }
        Commands::Decode {
            input,
            output,
            uncompressed,
        } => {
            let gaussians = load_spz(&input, !uncompressed).unwrap();
            write_ply(&gaussians, &output).unwrap();
        }
    }
}
