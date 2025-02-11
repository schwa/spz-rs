mod hilbert_curve;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use hilbert_curve::hilbert_sort;
use ply_format::{load_ply, write_ply};
use spz::{unpacked_gaussian::UnpackedGaussian, *};
use spz_format::{load_spz, write_spz};
use std::path::PathBuf;

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

        #[arg(short, long, default_value = "true")]
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

    Info {
        #[arg(value_name = "INPUT")]
        /// The input .spz file
        input: PathBuf,
    },

    Dump {
        #[arg(value_name = "INPUT")]
        /// The input .spz file
        input: PathBuf,

        #[arg(short, long)]
        limit: Option<usize>,

        #[arg(short, long, default_value = "debug")]
        format: DumpFormat,
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
            encode(
                input,
                output,
                uncompressed,
                skip_spherical_harmonics,
                use_hilbert_sort,
            )
            .unwrap();
        }
        Commands::Decode {
            input,
            output,
            uncompressed,
        } => {
            decode(input, output, uncompressed).unwrap();
        }
        Commands::Info { input } => {
            info(input).unwrap();
        }

        Commands::Dump {
            input,
            limit,
            format,
        } => {
            dump(input, limit, format).unwrap();
        }
    }
}

fn encode(
    input: PathBuf,
    output: PathBuf,
    uncompressed: bool,
    skip_spherical_harmonics: bool,
    use_hilbert_sort: bool,
) -> Result<()> {
    let mut gaussians = load_ply(&input)?;
    if gaussians.len() == 1 {
        println!("{:?}", gaussians[0]);
    }
    if use_hilbert_sort {
        gaussians = hilbert_sort(&gaussians, |g| g.position);
    }
    write_spz(gaussians, &output, !uncompressed, skip_spherical_harmonics)?;
    Ok(())
}

fn decode(input: PathBuf, output: PathBuf, uncompressed: bool) -> Result<()> {
    let gaussians = load_spz(&input, !uncompressed)?;
    write_ply(&gaussians, &output)?;
    Ok(())
}

fn info(input: PathBuf) -> Result<()> {
    let mut info = Vec::<String>::new();
    let extension = input
        .extension()
        .and_then(|s| s.to_str())
        .ok_or(anyhow::anyhow!("No extension"))?;
    let gaussians: Option<Vec<UnpackedGaussian>> = match extension {
        "spz" => load_spz(&input, true).ok(),
        "ply" => load_ply(&input).ok(),
        _ => panic!("Unsupported file extension"),
    };
    let gaussians = gaussians.ok_or(anyhow::anyhow!("Failed to load file"))?;
    info.push(format!("Number of gaussians: {}", gaussians.len()));
    println!("{}", info.join("\n"));
    Ok(())
}

#[derive(Clone, ValueEnum)]
enum DumpFormat {
    Debug,
    Json,
}

fn dump(input: PathBuf, limit: Option<usize>, format: DumpFormat) -> Result<()> {
    let extension = input
        .extension()
        .and_then(|s| s.to_str())
        .ok_or(anyhow::anyhow!("No extension"))?;
    let gaussians: Option<Vec<UnpackedGaussian>> = match extension {
        "spz" => load_spz(&input, true).ok(),
        "ply" => load_ply(&input).ok(),
        _ => panic!("Unsupported file extension"),
    };
    let mut gaussians = gaussians.ok_or(anyhow::anyhow!("Failed to load file"))?;

    if let Some(limit) = limit {
        gaussians.truncate(limit);
    }

    match format {
        DumpFormat::Debug => {
            for g in gaussians.iter() {
                println!("{:?}", g);
            }
            Ok(())
        }
        DumpFormat::Json => {
            let json = serde_json::to_string_pretty(&gaussians)?;
            println!("{}", json);
            Ok(())
        }
    }
}
