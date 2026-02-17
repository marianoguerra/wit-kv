use std::io::{self, Read};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use thiserror::Error;

use wit_core::{
    CanonicalAbiError, Value,
    binary_to_wave, load_wit_type_from_path, wave_from_str, wave_to_binary, wave_to_string,
};

/// CLI-specific errors.
#[derive(Error, Debug)]
pub enum AppError {
    /// Library error (wraps all wit_core errors)
    #[error(transparent)]
    Library(#[from] wit_core::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// WAVE parsing error
    #[error("WAVE parsing error: {0}")]
    WaveParse(String),

    /// WAVE writing error
    #[error("WAVE writing error: {0}")]
    WaveWrite(String),

    /// Missing value input
    #[error("Either --value, --file, or stdin must provide the WAVE text")]
    MissingValueInput,

    /// Schema not found
    #[error("No .type.wit found in {0} (use --wit to specify explicitly)")]
    SchemaNotFound(String),
}

impl From<CanonicalAbiError> for AppError {
    fn from(e: CanonicalAbiError) -> Self {
        Self::Library(e.into())
    }
}

#[derive(Parser)]
#[command(name = "wit-file")]
#[command(about = "Read and write raw canonical ABI binary files using WIT type definitions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Decode a binary file to WAVE text
    Read {
        /// Path to the WIT file defining the type (auto-discovers .type.wit if omitted)
        #[arg(long)]
        wit: Option<PathBuf>,

        /// Type name within the WIT file (defaults to first named type)
        #[arg(short = 't', long)]
        type_name: Option<String>,

        /// Binary or WAVE file to read
        file: PathBuf,

        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Encode WAVE text to a binary file
    Write {
        /// Path to the WIT file defining the type (auto-discovers .type.wit if omitted)
        #[arg(long)]
        wit: Option<PathBuf>,

        /// Type name within the WIT file (defaults to first named type)
        #[arg(short = 't', long)]
        type_name: Option<String>,

        /// Output file (binary by default, WAVE if .wave extension)
        #[arg(short, long)]
        output: PathBuf,

        /// WAVE text value (inline)
        #[arg(long, conflicts_with = "file")]
        value: Option<String>,

        /// File containing WAVE text
        #[arg(long, conflicts_with = "value")]
        file: Option<PathBuf>,
    },

    /// Validate WAVE text against a WIT type without writing
    Validate {
        /// Path to the WIT file defining the type (auto-discovers .type.wit if omitted)
        #[arg(long)]
        wit: Option<PathBuf>,

        /// Type name within the WIT file (defaults to first named type)
        #[arg(short = 't', long)]
        type_name: Option<String>,

        /// WAVE text value (inline)
        #[arg(long, conflicts_with = "file")]
        value: Option<String>,

        /// File containing WAVE text to validate
        #[arg(conflicts_with = "value")]
        file: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        format_error(&e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        Commands::Read {
            wit,
            type_name,
            file,
            output,
        } => {
            let wit_path = resolve_wit_path(wit.as_deref(), &file)?;
            cmd_read(&wit_path, type_name.as_deref(), &file, output.as_deref())
        }
        Commands::Write {
            wit,
            type_name,
            output,
            value,
            file,
        } => {
            // For auto-discovery, use the output file's directory or the input file's directory
            let reference_path = file.as_deref().unwrap_or(&output);
            let wit_path = resolve_wit_path(wit.as_deref(), reference_path)?;
            cmd_write(&wit_path, type_name.as_deref(), &output, value, file)
        }
        Commands::Validate {
            wit,
            type_name,
            value,
            file,
        } => {
            let reference_path = file.as_deref().unwrap_or(Path::new("."));
            let wit_path = resolve_wit_path(wit.as_deref(), reference_path)?;
            cmd_validate(&wit_path, type_name.as_deref(), value, file)
        }
    }
}

/// Resolve the WIT file path. If explicitly provided, use it.
/// Otherwise, look for `.type.wit` in the parent directory of the reference file.
fn resolve_wit_path(explicit: Option<&Path>, reference_file: &Path) -> Result<PathBuf, AppError> {
    if let Some(wit) = explicit {
        return Ok(wit.to_path_buf());
    }

    // Try to find .type.wit in the same directory as the reference file
    let dir = reference_file
        .parent()
        .unwrap_or(Path::new("."));
    let type_wit = dir.join(".type.wit");
    if type_wit.exists() {
        return Ok(type_wit);
    }

    Err(AppError::SchemaNotFound(dir.display().to_string()))
}

/// Check if a file path has a .wave extension.
fn is_wave_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "wave")
}

fn cmd_read(
    wit_path: &Path,
    type_name: Option<&str>,
    file: &Path,
    output: Option<&Path>,
) -> Result<(), AppError> {
    let resolved = load_wit_type_from_path(wit_path, type_name)?;

    // If input is a .wave file, parse as WAVE text, re-validate, re-format
    if is_wave_file(file) {
        let wave_text = std::fs::read_to_string(file)?;
        let wave_text = wave_text.trim();
        let parsed_value: Value =
            wave_from_str(&resolved.wave_type, wave_text).map_err(|e| AppError::WaveParse(e.to_string()))?;
        let wave_str =
            wave_to_string(&parsed_value).map_err(|e| AppError::WaveWrite(e.to_string()))?;
        write_output(output, &wave_str)?;
        return Ok(());
    }

    // Binary file
    let data = std::fs::read(file)?;
    let wave_str = binary_to_wave(&data, &resolved)?;
    write_output(output, &wave_str)?;

    Ok(())
}

fn cmd_write(
    wit_path: &Path,
    type_name: Option<&str>,
    output: &Path,
    value: Option<String>,
    file: Option<PathBuf>,
) -> Result<(), AppError> {
    let resolved = load_wit_type_from_path(wit_path, type_name)?;

    // Read WAVE text from --value, --file, or stdin
    let wave_text = if let Some(v) = value {
        v
    } else if let Some(f) = file {
        std::fs::read_to_string(&f)?
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        buf
    };

    let wave_text = wave_text.trim();
    if wave_text.is_empty() {
        return Err(AppError::MissingValueInput);
    }

    // If output is a .wave file, write normalized WAVE text instead of binary
    if is_wave_file(output) {
        let parsed_value: Value =
            wave_from_str(&resolved.wave_type, wave_text).map_err(|e| AppError::WaveParse(e.to_string()))?;
        let wave_str =
            wave_to_string(&parsed_value).map_err(|e| AppError::WaveWrite(e.to_string()))?;
        std::fs::write(output, &wave_str)?;
        return Ok(());
    }

    // Write binary
    let binary = wave_to_binary(wave_text, &resolved)?;
    std::fs::write(output, &binary)?;

    Ok(())
}

fn cmd_validate(
    wit_path: &Path,
    type_name: Option<&str>,
    value: Option<String>,
    file: Option<PathBuf>,
) -> Result<(), AppError> {
    let resolved = load_wit_type_from_path(wit_path, type_name)?;

    // Read WAVE text from --value, --file, or stdin
    let wave_text = if let Some(v) = value {
        v
    } else if let Some(f) = file {
        std::fs::read_to_string(&f)?
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        buf
    };

    let wave_text = wave_text.trim();
    if wave_text.is_empty() {
        return Err(AppError::MissingValueInput);
    }

    // Parse, validate, and verify it can be encoded
    wave_to_binary(wave_text, &resolved)?;

    println!("Valid");
    Ok(())
}

fn write_output(output: Option<&Path>, content: &str) -> Result<(), AppError> {
    match output {
        Some(path) => {
            std::fs::write(path, content)?;
        }
        None => {
            println!("{content}");
        }
    }
    Ok(())
}

fn format_error(e: &AppError) {
    eprintln!("Error: {e}");

    match e {
        AppError::Library(wit_core::Error::WaveParse(_)) | AppError::WaveParse(_) => {
            eprintln!("Hint: Check that the WAVE text matches the WIT type definition.");
        }
        AppError::Library(wit_core::Error::WitParse(_)) => {
            eprintln!("Hint: Check the WIT file for syntax errors.");
        }
        AppError::Library(wit_core::Error::DataTooSmall { .. }) => {
            eprintln!("Hint: The file may be truncated or encoded for a different type.");
        }
        AppError::SchemaNotFound(_) => {
            eprintln!("Hint: Place a .type.wit file in the same directory, or use --wit.");
        }
        _ => {}
    }
}
