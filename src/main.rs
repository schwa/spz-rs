mod hilbert_curve;
mod spz;
mod support;
mod unpacked_gaussian;

use clap::{Parser, Subcommand};
use hilbert_curve::hilbert_sort;
use spz::{load_spz, write_spz};
use std::path::PathBuf;
use unpacked_gaussian::{load_ply, write_ply};

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

        #[arg(short, long, default_value = "false")]
        /// Do not include spherical harmonics in the output.
        skip_spherical_harmonics: bool,

        #[arg(short, long, default_value = "false")]
        /// Sort the gaussians using a hilbert curve. This may improve compression. Experimental.
        use_hilbert_sort: bool,
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
            skip_spherical_harmonics,
            use_hilbert_sort,
        } => {
            let mut gaussians = load_ply(&input).unwrap();
            if gaussians.len() == 1 {
                println!("{:?}", gaussians[0]);
            }

            if use_hilbert_sort {
                gaussians = hilbert_sort(&gaussians, |g| g.position);
            }

            write_spz(gaussians, &output, !uncompressed, skip_spherical_harmonics).unwrap();
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
