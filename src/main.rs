use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use thiserror::Error;
use wasm_wave::value::{resolve_wit_type, Value};
use wit_parser::{Resolve, Type, TypeId};

use wit_kv::kv::{BinaryExport, KvError, KvStore};
use wit_kv::wasm::{KeyFilter, MapOperation, ReduceOperation, TypedRunner, WasmError};
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

    #[error("Wasm execution error: {0}")]
    Wasm(#[from] WasmError),
}

/// Output format for map results.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// WAVE text format (human-readable).
    #[default]
    Wave,
    /// Binary format (binary-export).
    Binary,
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

        /// Output as canonical ABI binary (binary-export WIT type)
        #[arg(long)]
        binary: bool,

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

    /// Map values in a keyspace through a WebAssembly Component
    MapLow {
        /// Name of the keyspace
        keyspace: String,

        /// Path to the WebAssembly Component module (.wasm)
        #[arg(long)]
        module: PathBuf,

        /// Process only this specific key
        #[arg(long, group = "key_selection")]
        key: Option<String>,

        /// Filter keys by prefix
        #[arg(long, group = "key_selection")]
        prefix: Option<String>,

        /// Start key for range (inclusive)
        #[arg(long)]
        start: Option<String>,

        /// End key for range (exclusive)
        #[arg(long)]
        end: Option<String>,

        /// Maximum number of values to process
        #[arg(long)]
        limit: Option<usize>,

        /// WIT file defining the output type T1 (if different from keyspace type)
        #[arg(long)]
        output_wit: Option<PathBuf>,

        /// Name of the output type in output_wit
        #[arg(long)]
        output_type: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Wave)]
        format: OutputFormat,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Reduce values in a keyspace using a WebAssembly Component (fold-left)
    ReduceLow {
        /// Name of the keyspace
        keyspace: String,

        /// Path to the WebAssembly Component module (.wasm)
        #[arg(long)]
        module: PathBuf,

        /// Filter keys by prefix
        #[arg(long)]
        prefix: Option<String>,

        /// Start key for range (inclusive)
        #[arg(long)]
        start: Option<String>,

        /// End key for range (exclusive)
        #[arg(long)]
        end: Option<String>,

        /// Maximum number of values to process
        #[arg(long)]
        limit: Option<usize>,

        /// WIT file defining the state type (if not embedded in module)
        #[arg(long)]
        state_wit: Option<PathBuf>,

        /// Name of the state type in state_wit
        #[arg(long)]
        state_type: Option<String>,

        /// Output file for the result (stdout if not specified)
        #[arg(long)]
        output: Option<PathBuf>,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Map values using a typed WebAssembly Component (actual WIT types, not binary-export)
    Map {
        /// Name of the keyspace
        keyspace: String,

        /// Path to the WebAssembly Component module (.wasm)
        #[arg(long)]
        module: PathBuf,

        /// WIT file defining the component's types
        #[arg(long)]
        module_wit: PathBuf,

        /// Name of the input type in module_wit
        #[arg(long)]
        input_type: String,

        /// Name of the output type (defaults to input type)
        #[arg(long)]
        output_type: Option<String>,

        /// Process only this specific key
        #[arg(long, group = "key_selection")]
        key: Option<String>,

        /// Filter keys by prefix
        #[arg(long, group = "key_selection")]
        prefix: Option<String>,

        /// Start key for range (inclusive)
        #[arg(long)]
        start: Option<String>,

        /// End key for range (exclusive)
        #[arg(long)]
        end: Option<String>,

        /// Maximum number of values to process
        #[arg(long)]
        limit: Option<usize>,

        /// Store path
        #[arg(long, default_value = ".wit-kv", env = "WIT_KV_PATH")]
        path: PathBuf,
    },

    /// Reduce values using a typed WebAssembly Component (actual WIT types, not binary-export)
    Reduce {
        /// Name of the keyspace
        keyspace: String,

        /// Path to the WebAssembly Component module (.wasm)
        #[arg(long)]
        module: PathBuf,

        /// WIT file defining the component's types
        #[arg(long)]
        module_wit: PathBuf,

        /// Name of the input/value type in module_wit
        #[arg(long)]
        input_type: String,

        /// Name of the state type in module_wit
        #[arg(long)]
        state_type: String,

        /// Filter keys by prefix
        #[arg(long)]
        prefix: Option<String>,

        /// Start key for range (inclusive)
        #[arg(long)]
        start: Option<String>,

        /// End key for range (exclusive)
        #[arg(long)]
        end: Option<String>,

        /// Maximum number of values to process
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

            // Export using binary-export WIT type (single file with buffer + memory)
            let memory_bytes = memory.into_bytes();
            let export = BinaryExport {
                buffer: binary,
                memory: if memory_bytes.is_empty() {
                    None
                } else {
                    Some(memory_bytes)
                },
            };
            let (export_buffer, export_memory) = export.encode()?;

            // Write combined output
            let mut file = std::fs::File::create(&output)?;
            file.write_all(&export_buffer)?;
            file.write_all(&export_memory)?;

            println!(
                "Lowered value to {} ({} bytes)",
                output.display(),
                export_buffer.len() + export_memory.len()
            );
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

            // Read binary-export format (single file with buffer + memory)
            let data = std::fs::read(&input)?;
            let export = BinaryExport::decode_from_bytes(&data)?;

            // Lift the value from the exported buffer and memory
            let abi = CanonicalAbi::new(&resolve);
            let ty = Type::Id(type_id);
            let memory = export
                .memory
                .map(LinearMemory::from_bytes)
                .unwrap_or_default();
            let (value, _bytes_read) =
                abi.lift_with_memory(&export.buffer, &ty, &wave_type, &memory)?;

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
            path,
        } => {
            let store = KvStore::open(&path)?;
            if binary {
                match store.get_raw(&keyspace, &key)? {
                    Some(stored) => {
                        // Export using binary-export WIT type (buffer + memory)
                        let export = BinaryExport::from_stored(&stored);
                        let (buffer, memory) = export.encode()?;
                        std::io::stdout().write_all(&buffer)?;
                        std::io::stdout().write_all(&memory)?;
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
        Commands::MapLow {
            keyspace,
            module,
            key,
            prefix,
            start,
            end,
            limit,
            output_wit,
            output_type,
            format,
            path,
        } => {
            let store = KvStore::open(&path)?;

            // Determine key filter
            let key_filter = if let Some(k) = key {
                KeyFilter::Single(k)
            } else if let Some(p) = prefix {
                KeyFilter::Prefix(p)
            } else if start.is_some() || end.is_some() {
                KeyFilter::Range { start, end }
            } else {
                KeyFilter::All
            };

            // Execute map operation
            let mut op = MapOperation::new(&store, &module)?;
            let result = op.execute(&keyspace, key_filter, limit)?;

            // Output results
            match format {
                OutputFormat::Wave => {
                    // For WAVE output, we need to decode the binary values
                    // If output_wit is provided, use that type; otherwise use keyspace type
                    let (resolve, type_id) = if let Some(ref wit_path) = output_wit {
                        load_wit_type(wit_path, output_type.as_deref())?
                    } else {
                        // Use keyspace type
                        let metadata = store
                            .get_type(&keyspace)?
                            .ok_or_else(|| AppError::TypeNotFound(keyspace.clone()))?;
                        let mut resolve = Resolve::new();
                        resolve.push_str("stored.wit", &metadata.wit_definition)?;
                        let type_id = resolve
                            .types
                            .iter()
                            .find(|(_, ty)| ty.name.as_deref() == Some(&metadata.type_name))
                            .map(|(id, _)| id)
                            .ok_or_else(|| AppError::TypeNotFound(metadata.type_name.clone()))?;
                        (resolve, type_id)
                    };

                    let wave_type = resolve_wit_type(&resolve, type_id)
                        .map_err(|e| AppError::WaveParse(e.to_string()))?;
                    let abi = CanonicalAbi::new(&resolve);

                    for (k, export) in &result.values {
                        let memory = export
                            .memory
                            .as_ref()
                            .map(|m| LinearMemory::from_bytes(m.clone()))
                            .unwrap_or_default();
                        match abi.lift_with_memory(&export.buffer, &Type::Id(type_id), &wave_type, &memory) {
                            Ok((value, _)) => {
                                let wave_str = wasm_wave::to_string(&value)
                                    .map_err(|e| AppError::WaveWrite(e.to_string()))?;
                                println!("{}: {}", k, wave_str);
                            }
                            Err(e) => {
                                eprintln!("{}: <decode error: {}>", k, e);
                            }
                        }
                    }
                }
                OutputFormat::Binary => {
                    // Output raw binary-export format for each result
                    for (k, export) in &result.values {
                        let (buffer, memory) = export.encode()?;
                        // Write key length, key, then binary data
                        let key_bytes = k.as_bytes();
                        std::io::stdout().write_all(&(key_bytes.len() as u32).to_le_bytes())?;
                        std::io::stdout().write_all(key_bytes)?;
                        std::io::stdout().write_all(&(buffer.len() as u32).to_le_bytes())?;
                        std::io::stdout().write_all(&buffer)?;
                        std::io::stdout().write_all(&memory)?;
                    }
                }
            }

            // Print summary to stderr
            eprintln!("{}", result.summary());
            if result.has_errors() {
                for (k, err) in &result.errors {
                    eprintln!("  Error for '{}': {}", k, err);
                }
            }
            Ok(())
        }
        Commands::ReduceLow {
            keyspace,
            module,
            prefix,
            start,
            end,
            limit,
            state_wit,
            state_type,
            output,
            path,
        } => {
            let store = KvStore::open(&path)?;

            // Determine key filter
            let key_filter = if let Some(p) = prefix {
                KeyFilter::Prefix(p)
            } else if start.is_some() || end.is_some() {
                KeyFilter::Range { start, end }
            } else {
                KeyFilter::All
            };

            // Execute reduce operation
            let mut op = ReduceOperation::new(&store, &module)?;
            let result = op.execute(&keyspace, key_filter, limit)?;

            // Encode the final state as binary-export
            let (buffer, memory) = result.final_state.encode()?;

            // Write output
            match output {
                Some(output_path) => {
                    let mut file = std::fs::File::create(&output_path)?;
                    file.write_all(&buffer)?;
                    file.write_all(&memory)?;
                    eprintln!(
                        "Reduced {} values to {} ({} bytes)",
                        result.processed_count,
                        output_path.display(),
                        buffer.len() + memory.len()
                    );
                }
                None => {
                    // If state_wit is provided, decode and output as WAVE
                    if let Some(ref wit_path) = state_wit {
                        let (resolve, type_id) = load_wit_type(wit_path, state_type.as_deref())?;
                        let wave_type = resolve_wit_type(&resolve, type_id)
                            .map_err(|e| AppError::WaveParse(e.to_string()))?;
                        let abi = CanonicalAbi::new(&resolve);
                        let mem = result
                            .final_state
                            .memory
                            .as_ref()
                            .map(|m| LinearMemory::from_bytes(m.clone()))
                            .unwrap_or_default();
                        let (value, _) = abi.lift_with_memory(
                            &result.final_state.buffer,
                            &Type::Id(type_id),
                            &wave_type,
                            &mem,
                        )?;
                        let wave_str = wasm_wave::to_string(&value)
                            .map_err(|e| AppError::WaveWrite(e.to_string()))?;
                        println!("{}", wave_str);
                    } else {
                        // Output raw binary to stdout
                        std::io::stdout().write_all(&buffer)?;
                        std::io::stdout().write_all(&memory)?;
                    }
                    eprintln!("Reduced {} values ({} bytes)", result.processed_count, buffer.len() + memory.len());
                }
            }
            Ok(())
        }
        Commands::Map {
            keyspace,
            module,
            module_wit,
            input_type,
            output_type,
            key,
            prefix,
            start,
            end,
            limit,
            path,
        } => {
            let store = KvStore::open(&path)?;

            // Create typed runner
            let mut runner = TypedRunner::new(
                &module,
                &module_wit,
                &input_type,
                output_type.as_deref(),
            )?;

            // Get keyspace metadata for lifting values
            let metadata = store
                .get_type(&keyspace)?
                .ok_or_else(|| AppError::TypeNotFound(keyspace.clone()))?;

            // Get keys to process
            let keys: Vec<String> = if let Some(k) = key {
                vec![k]
            } else {
                store.list(&keyspace, prefix.as_deref(), limit)?
            };

            // Apply range filter if specified
            let keys: Vec<_> = keys
                .into_iter()
                .filter(|k| {
                    let after_start = start.as_ref().is_none_or(|s| k >= s);
                    let before_end = end.as_ref().is_none_or(|e| k < e);
                    after_start && before_end
                })
                .collect();

            let mut processed = 0;
            let mut filtered = 0;
            let mut transformed = 0;
            let mut errors: Vec<(String, String)> = Vec::new();

            // Get wave type for output
            let wave_type = runner.output_wave_type()?;

            // Load resolve and type_id once for output decoding
            let output_type_name = output_type.as_deref().unwrap_or(&input_type);
            let (output_resolve, output_type_id) = load_wit_type(&module_wit, Some(output_type_name))?;
            let output_abi = CanonicalAbi::new(&output_resolve);

            for k in keys {
                match store.get_raw(&keyspace, &k)? {
                    Some(stored) => {
                        // Call filter
                        match runner.call_filter(&stored) {
                            Ok(true) => {
                                // Call transform
                                match runner.call_transform(&stored, metadata.type_version) {
                                    Ok(result) => {
                                        // Output the transformed value
                                        let memory = result
                                            .memory
                                            .as_ref()
                                            .map(|m| LinearMemory::from_bytes(m.clone()))
                                            .unwrap_or_default();

                                        match output_abi.lift_with_memory(
                                            &result.value,
                                            &Type::Id(output_type_id),
                                            &wave_type,
                                            &memory,
                                        ) {
                                            Ok((value, _)) => {
                                                let wave_str = wasm_wave::to_string(&value)
                                                    .map_err(|e| AppError::WaveWrite(e.to_string()))?;
                                                println!("{}: {}", k, wave_str);
                                            }
                                            Err(e) => {
                                                eprintln!("{}: <decode error: {}>", k, e);
                                            }
                                        }
                                        transformed += 1;
                                    }
                                    Err(e) => {
                                        errors.push((k.clone(), format!("transform: {}", e)));
                                    }
                                }
                            }
                            Ok(false) => {
                                filtered += 1;
                            }
                            Err(e) => {
                                errors.push((k.clone(), format!("filter: {}", e)));
                            }
                        }
                        processed += 1;
                    }
                    None => {
                        errors.push((k.clone(), "not found".to_string()));
                    }
                }
            }

            eprintln!(
                "Processed {} keys: {} transformed, {} filtered out, {} errors",
                processed, transformed, filtered, errors.len()
            );
            for (k, err) in &errors {
                eprintln!("  Error for '{}': {}", k, err);
            }
            Ok(())
        }
        Commands::Reduce {
            keyspace,
            module,
            module_wit,
            input_type,
            state_type,
            prefix,
            start,
            end,
            limit,
            path,
        } => {
            let store = KvStore::open(&path)?;

            // Create typed runner with input_type for values and state_type for state
            let mut runner = TypedRunner::new(
                &module,
                &module_wit,
                &input_type,
                Some(&state_type),
            )?;

            // Get keyspace metadata for type version
            let metadata = store
                .get_type(&keyspace)?
                .ok_or_else(|| AppError::TypeNotFound(keyspace.clone()))?;

            // Get keys to process
            let keys: Vec<String> = store.list(&keyspace, prefix.as_deref(), None)?;

            // Apply range filter if specified
            let keys: Vec<_> = keys
                .into_iter()
                .filter(|k| {
                    let after_start = start.as_ref().is_none_or(|s| k >= s);
                    let before_end = end.as_ref().is_none_or(|e| k < e);
                    after_start && before_end
                })
                .collect();

            // Apply limit
            let keys: Vec<_> = match limit {
                Some(l) => keys.into_iter().take(l).collect(),
                None => keys,
            };

            // Initialize state
            let mut state = runner.call_init_state(metadata.type_version)?;
            let mut processed = 0;
            let mut errors: Vec<(String, String)> = Vec::new();

            for k in keys {
                match store.get_raw(&keyspace, &k)? {
                    Some(stored) => {
                        match runner.call_reduce(&state, &stored, metadata.type_version) {
                            Ok(new_state) => {
                                state = new_state;
                                processed += 1;
                            }
                            Err(e) => {
                                errors.push((k.clone(), format!("reduce: {}", e)));
                            }
                        }
                    }
                    None => {
                        errors.push((k.clone(), "not found".to_string()));
                    }
                }
            }

            // Output final state as WAVE
            let wave_type = runner.output_wave_type()?;
            let (output_resolve, output_type_id) = load_wit_type(&module_wit, Some(&state_type))?;
            let output_abi = CanonicalAbi::new(&output_resolve);
            let memory = state
                .memory
                .as_ref()
                .map(|m| LinearMemory::from_bytes(m.clone()))
                .unwrap_or_default();

            match output_abi.lift_with_memory(
                &state.value,
                &Type::Id(output_type_id),
                &wave_type,
                &memory,
            ) {
                Ok((value, _)) => {
                    let wave_str = wasm_wave::to_string(&value)
                        .map_err(|e| AppError::WaveWrite(e.to_string()))?;
                    println!("{}", wave_str);
                }
                Err(e) => {
                    eprintln!("<decode error: {}>", e);
                }
            }

            eprintln!(
                "Reduced {} values, {} errors",
                processed, errors.len()
            );
            for (k, err) in &errors {
                eprintln!("  Error for '{}': {}", k, err);
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
