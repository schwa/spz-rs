mod hilbert_curve;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use hilbert_curve::hilbert_sort;
use ply_format::{load_ply, write_ply};
use spz::{unpacked_gaussian::UnpackedGaussian, *};
use spz_format::{load_spz, write_spz};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
enum Commands {
    /// Convert a .ply file to a .spz file
    Convert {
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
        omit_spherical_harmonics: bool,

        #[arg(long, default_value = "false")]
        /// Sort the gaussians using a hilbert curve. This may improve compression. Experimental.
        use_hilbert_sort: bool,
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

    Diff {
        #[arg(value_name = "OLD")]
        old: PathBuf,
        #[arg(value_name = "NEW")]
        new: PathBuf,
    },
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() {
    better_panic::install();

    let cli = Cli::parse();
    match cli.command {
        Commands::Convert {
            input,
            output,
            uncompressed,
            omit_spherical_harmonics,
            use_hilbert_sort,
        } => {
            convert(
                &input,
                &output,
                uncompressed,
                omit_spherical_harmonics,
                use_hilbert_sort,
            )
            .unwrap();
        }
        Commands::Info { input } => {
            info(&input).unwrap();
        }

        Commands::Dump {
            input,
            limit,
            format,
        } => {
            dump(&input, limit, format).unwrap();
        }

        Commands::Diff { old, new } => {
            diff(&old, &new, None).unwrap();
        }
    }
}

fn convert(
    input: &Path,
    output: &Path,
    uncompressed: bool,
    omit_spherical_harmonics: bool,
    use_hilbert_sort: bool,
) -> Result<()> {
    let mut gaussians = load(input)?;
    if use_hilbert_sort {
        gaussians = hilbert_sort(&gaussians, |g| g.position);
    }

    let options = SaveOptions {
        compressed: !uncompressed,
        omit_spherical_harmonics,
    };
    save(gaussians, output, &options)?;
    Ok(())
}

fn info(input: &Path) -> Result<()> {
    let mut info = Vec::<String>::new();
    let extension = input
        .extension()
        .and_then(|s| s.to_str())
        .ok_or(anyhow::anyhow!("No extension"))?;
    let gaussians: Option<Vec<UnpackedGaussian>> = match extension {
        "spz" => load_spz(input, true).ok(),
        "ply" => load_ply(input).ok(),
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
    Pretty,
    Json,
}

fn dump(input: &Path, limit: Option<usize>, format: DumpFormat) -> Result<()> {
    let mut gaussians = load(input)?;

    if let Some(limit) = limit {
        gaussians.truncate(limit);
    }

    match format {
        DumpFormat::Debug => {
            for g in gaussians.iter() {
                println!("{:?}", g);
            }
        }
        DumpFormat::Pretty => {
            for g in gaussians.iter() {
                println!("{:#?}", g);
            }
        }
        DumpFormat::Json => {
            let json = serde_json::to_string_pretty(&gaussians)?;
            print!("{}", json);
        }
    }
    Ok(())
}

fn load(input: &Path) -> Result<Vec<UnpackedGaussian>> {
    let extension = input
        .extension()
        .and_then(|s| s.to_str())
        .ok_or(anyhow::anyhow!("No extension"))?;
    match extension {
        "spz" => load_spz(input, true),
        "ply" => load_ply(input),
        _ => panic!("Unsupported file extension"),
    }
}

struct SaveOptions {
    compressed: bool,
    omit_spherical_harmonics: bool,
}

fn save(gaussians: Vec<UnpackedGaussian>, output: &Path, options: &SaveOptions) -> Result<()> {
    let extension = output
        .extension()
        .and_then(|s| s.to_str())
        .ok_or(anyhow::anyhow!("No extension"))?;
    match extension {
        "spz" => write_spz(
            gaussians,
            output,
            options.compressed,
            options.omit_spherical_harmonics,
        ),
        "ply" => write_ply(&gaussians, output),
        _ => panic!("Unsupported file extension"),
    }
}

fn diff(old: &Path, new: &Path, limit: Option<usize>) -> Result<()> {
    let old = load(old)?;
    let new = load(new)?;

    if old.len() != new.len() {
        println!(
            "Different number of gaussians: {} vs {}",
            old.len(),
            new.len()
        );
        return Ok(());
    }

    if old == new {
        println!("Files are identical");
        return Ok(());
    }

    for (old, new) in old.iter().zip(new.iter()) {
        if old != new {
            let old = format!("{:#?}", old);
            let new = format!("{:#?}", new);
            println!("{}", side_by_side(&old, &new));
        }
    }

    Ok(())
}

fn side_by_side(left: &str, right: &str) -> String {
    let left = left.lines();
    let left_max_len = left.clone().map(|l| l.len()).max().unwrap_or(0);
    let right = right.lines();
    
    left
        .zip(right)
        .map(|(l, r)| format!("{:<width$} | {}", l, r, width = left_max_len))
        .collect::<Vec<_>>()
        .join("\n")
}
