use clap::{Parser, Subcommand};
use std::path::PathBuf;
use thiserror::Error;
use wasm_wave::value::{resolve_wit_type, Value};
use wit_parser::{Resolve, Type, TypeId};

use wit_kv::kv::{KvError, KvStore};
use wit_kv::{CanonicalAbi, CanonicalAbiError, LinearMemory};

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

    #[error("KV store error: {0}")]
    Kv(#[from] KvError),
}

#[derive(Parser)]
#[command(name = "wit-kv")]
#[command(about = "Lower and lift WIT values using canonical ABI, with typed key-value storage")]
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

    /// Initialize a new key-value store
    Init {
        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Register a type for a keyspace
    SetType {
        /// Name of the keyspace
        keyspace: String,

        /// Path to the WIT file containing the type definition
        #[arg(long)]
        wit: PathBuf,

        /// Name of the type to use (if not specified, uses the first type found)
        #[arg(long)]
        type_name: Option<String>,

        /// Overwrite existing type definition
        #[arg(long)]
        force: bool,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Get the type definition for a keyspace
    GetType {
        /// Name of the keyspace
        keyspace: String,

        /// Output raw binary format instead of WIT text
        #[arg(long)]
        binary: bool,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Delete a keyspace type
    DeleteType {
        /// Name of the keyspace
        keyspace: String,

        /// Also delete all data in the keyspace
        #[arg(long)]
        delete_data: bool,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// List all registered types
    ListTypes {
        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Set a value in a keyspace
    Set {
        /// Name of the keyspace
        keyspace: String,

        /// Key for the value
        key: String,

        /// WAVE-encoded value
        #[arg(long, group = "input")]
        value: Option<String>,

        /// Read WAVE value from file
        #[arg(long, group = "input")]
        file: Option<PathBuf>,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Get a value from a keyspace
    Get {
        /// Name of the keyspace
        keyspace: String,

        /// Key for the value
        key: String,

        /// Output raw canonical ABI bytes (value only)
        #[arg(long)]
        binary: bool,

        /// Output full stored format (value + memory)
        #[arg(long)]
        raw: bool,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Delete a value from a keyspace
    Delete {
        /// Name of the keyspace
        keyspace: String,

        /// Key for the value
        key: String,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// List keys in a keyspace
    List {
        /// Name of the keyspace
        keyspace: String,

        /// Filter keys by prefix
        #[arg(long)]
        prefix: Option<String>,

        /// Maximum number of keys to return
        #[arg(long)]
        limit: Option<usize>,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },
}

fn main() -> Result<(), AppError> {
    use std::io::Write;

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
        Commands::Init { path } => {
            let store = KvStore::init(&path)?;
            drop(store);
            println!("Initialized KV store at {}", path.display());
            Ok(())
        }
        Commands::SetType {
            keyspace,
            wit,
            type_name,
            force,
            path,
        } => {
            let store = KvStore::open(&path)?;
            let metadata = store.set_type(&keyspace, &wit, type_name.as_deref(), force)?;
            println!(
                "Registered type '{}' for keyspace '{}' ({})",
                metadata.type_name, keyspace, metadata.qualified_name
            );
            Ok(())
        }
        Commands::GetType {
            keyspace,
            binary,
            path,
        } => {
            let store = KvStore::open(&path)?;
            match store.get_type(&keyspace)? {
                Some(metadata) => {
                    if binary {
                        let (buffer, memory) = metadata.encode()?;
                        std::io::stdout().write_all(&buffer)?;
                        if !memory.is_empty() {
                            std::io::stderr().write_all(b"Memory written to stderr\n")?;
                            std::io::stderr().write_all(&memory)?;
                        }
                    } else {
                        println!("{}", metadata.wit_definition);
                    }
                }
                None => {
                    eprintln!("Keyspace '{}' not found", keyspace);
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        Commands::DeleteType {
            keyspace,
            delete_data,
            path,
        } => {
            let store = KvStore::open(&path)?;
            store.delete_type(&keyspace, delete_data)?;
            if delete_data {
                println!("Deleted keyspace '{}' and all its data", keyspace);
            } else {
                println!("Deleted type for keyspace '{}'", keyspace);
            }
            Ok(())
        }
        Commands::ListTypes { path } => {
            let store = KvStore::open(&path)?;
            let types = store.list_types()?;
            if types.is_empty() {
                println!("No types registered");
            } else {
                for metadata in types {
                    println!(
                        "{}: {} (version {})",
                        metadata.name, metadata.qualified_name, metadata.type_version
                    );
                }
            }
            Ok(())
        }
        Commands::Set {
            keyspace,
            key,
            value,
            file,
            path,
        } => {
            let store = KvStore::open(&path)?;
            let wave_value = match (value, file) {
                (Some(v), None) => v,
                (None, Some(f)) => std::fs::read_to_string(f)?,
                (None, None) => {
                    eprintln!("Either --value or --file must be specified");
                    std::process::exit(1);
                }
                // Clap group ensures mutual exclusivity, but handle gracefully
                (Some(v), Some(_)) => v,
            };
            store.set(&keyspace, &key, &wave_value)?;
            println!("Set '{}' in keyspace '{}'", key, keyspace);
            Ok(())
        }
        Commands::Get {
            keyspace,
            key,
            binary,
            raw,
            path,
        } => {
            let store = KvStore::open(&path)?;
            if binary || raw {
                match store.get_raw(&keyspace, &key)? {
                    Some(stored) => {
                        if raw {
                            let (buffer, memory) = stored.encode()?;
                            std::io::stdout().write_all(&buffer)?;
                            if !memory.is_empty() {
                                std::io::stdout().write_all(&memory)?;
                            }
                        } else {
                            // binary - just the value bytes
                            std::io::stdout().write_all(&stored.value)?;
                        }
                    }
                    None => {
                        eprintln!("Key '{}' not found in keyspace '{}'", key, keyspace);
                        std::process::exit(1);
                    }
                }
            } else {
                match store.get(&keyspace, &key)? {
                    Some(wave_str) => {
                        println!("{}", wave_str);
                    }
                    None => {
                        eprintln!("Key '{}' not found in keyspace '{}'", key, keyspace);
                        std::process::exit(1);
                    }
                }
            }
            Ok(())
        }
        Commands::Delete { keyspace, key, path } => {
            let store = KvStore::open(&path)?;
            store.delete(&keyspace, &key)?;
            println!("Deleted '{}' from keyspace '{}'", key, keyspace);
            Ok(())
        }
        Commands::List {
            keyspace,
            prefix,
            limit,
            path,
        } => {
            let store = KvStore::open(&path)?;
            let keys = store.list(&keyspace, prefix.as_deref(), limit)?;
            if keys.is_empty() {
                println!("No keys found");
            } else {
                for key in keys {
                    println!("{}", key);
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
