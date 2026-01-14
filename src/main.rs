use clap::{Parser, Subcommand};
use std::path::PathBuf;
use thiserror::Error;
use wasm_wave::value::{resolve_wit_type, Value};
use wit_parser::{Resolve, Type, TypeId};

use wit_value::{CanonicalAbi, CanonicalAbiError, LinearMemory};

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WIT parsing error: {0}")]
    WitParse(#[from] anyhow::Error),

    #[error("WAVE parsing error: {0}")]
    WaveParse(String),

    #[error("WAVE writing error: {0}")]
    WaveWrite(String),

    #[error("Type not found: {0}")]
    TypeNotFound(String),

    #[error("Canonical ABI error: {0}")]
    CanonicalAbi(#[from] CanonicalAbiError),

    #[error("No types found in WIT file")]
    NoTypes,
}

#[derive(Parser)]
#[command(name = "wit-value")]
#[command(about = "Lower and lift WIT values using canonical ABI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Lower a WAVE-encoded value to binary using canonical ABI
    Lower {
        /// Path to the WIT file containing the type definition
        #[arg(short, long)]
        wit: PathBuf,

        /// Name of the type to use (if not specified, uses the first type found)
        #[arg(short = 't', long)]
        type_name: Option<String>,

        /// WAVE-encoded value to lower
        #[arg(short, long)]
        value: String,

        /// Output file for the binary data
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Lift binary data to a WAVE-encoded value using canonical ABI
    Lift {
        /// Path to the WIT file containing the type definition
        #[arg(short, long)]
        wit: PathBuf,

        /// Name of the type to use (if not specified, uses the first type found)
        #[arg(short = 't', long)]
        type_name: Option<String>,

        /// Input file containing binary data
        #[arg(short, long)]
        input: PathBuf,

        /// Output file for WAVE representation (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<(), AppError> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Lower {
            wit,
            type_name,
            value,
            output,
        } => {
            let (resolve, type_id) = load_wit_type(&wit, type_name.as_deref())?;
            let wave_type = resolve_wit_type(&resolve, type_id)
                .map_err(|e| AppError::WaveParse(e.to_string()))?;

            let parsed_value: Value = wasm_wave::from_str(&wave_type, &value)
                .map_err(|e| AppError::WaveParse(e.to_string()))?;

            let abi = CanonicalAbi::new(&resolve);
            let ty = Type::Id(type_id);

            // Use linear memory to support variable-length types (strings, lists)
            let mut memory = LinearMemory::new();
            let binary = abi.lower_with_memory(&parsed_value, &ty, &wave_type, &mut memory)?;

            std::fs::write(&output, &binary)?;

            // Save linear memory if it contains data (for variable-length types)
            if !memory.is_empty() {
                let memory_path = format!("{}.memory", output.display());
                std::fs::write(&memory_path, memory.as_bytes())?;
                println!(
                    "Lowered value to {} ({} bytes) + {} ({} bytes)",
                    output.display(),
                    binary.len(),
                    memory_path,
                    memory.as_bytes().len()
                );
            } else {
                println!(
                    "Lowered value to {} ({} bytes)",
                    output.display(),
                    binary.len()
                );
            }
            Ok(())
        }
        Commands::Lift {
            wit,
            type_name,
            input,
            output,
        } => {
            let (resolve, type_id) = load_wit_type(&wit, type_name.as_deref())?;
            let wave_type = resolve_wit_type(&resolve, type_id)
                .map_err(|e| AppError::WaveParse(e.to_string()))?;

            let binary = std::fs::read(&input)?;

            // Check for associated .memory file for variable-length types
            let memory_path = format!("{}.memory", input.display());
            let memory = if std::path::Path::new(&memory_path).exists() {
                let memory_bytes = std::fs::read(&memory_path)?;
                Some(LinearMemory::from_bytes(memory_bytes))
            } else {
                None
            };

            let abi = CanonicalAbi::new(&resolve);
            let ty = Type::Id(type_id);
            let (value, _bytes_read) = match memory {
                Some(ref mem) => abi.lift_with_memory(&binary, &ty, &wave_type, mem)?,
                None => abi.lift(&binary, &ty, &wave_type)?,
            };

            let wave_str =
                wasm_wave::to_string(&value).map_err(|e| AppError::WaveWrite(e.to_string()))?;

            match output {
                Some(path) => {
                    std::fs::write(&path, &wave_str)?;
                    println!("Lifted value to {}", path.display());
                }
                None => {
                    println!("{}", wave_str);
                }
            }
            Ok(())
        }
    }
}

fn load_wit_type(wit_path: &PathBuf, type_name: Option<&str>) -> Result<(Resolve, TypeId), AppError> {
    let mut resolve = Resolve::new();
    resolve.push_path(wit_path)?;

    match type_name {
        Some(name) => {
            // Find the type by name
            for (id, ty) in resolve.types.iter() {
                if ty.name.as_deref() == Some(name) {
                    return Ok((resolve, id));
                }
            }
            Err(AppError::TypeNotFound(name.to_string()))
        }
        None => {
            // Use the first named type
            for (id, ty) in resolve.types.iter() {
                if ty.name.is_some() {
                    return Ok((resolve, id));
                }
            }
            Err(AppError::NoTypes)
        }
    }
}
