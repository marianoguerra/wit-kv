use clap::{Parser, Subcommand};
use std::path::PathBuf;
use thiserror::Error;
use wasm_wave::value::{resolve_wit_type, Value};
use wit_parser::{Resolve, Type, TypeId};

use wit_kv::kv::{BinaryExport, KvError, KvStore};
use wit_kv::wasm::{TypedRunner, WasmError};
use wit_kv::{
    find_first_named_type, find_type_by_name, val_to_wave, CanonicalAbi, CanonicalAbiError,
    LinearMemory, ValConvertError,
};

/// CLI-specific errors.
#[derive(Error, Debug)]
pub enum AppError {
    /// Library error (wraps all wit_kv errors)
    #[error(transparent)]
    Library(#[from] wit_kv::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// WAVE parsing error
    #[error("WAVE parsing error: {0}")]
    WaveParse(String),

    /// WAVE writing error
    #[error("WAVE writing error: {0}")]
    WaveWrite(String),

    /// Type not found
    #[error("Type not found: {0}")]
    TypeNotFound(String),

    /// No types found in WIT file
    #[error("No types found in WIT file")]
    NoTypes,

    /// Missing value input
    #[error("Either --value or --file must be specified")]
    MissingValueInput,

    /// Key not found
    #[error("Key '{key}' not found in keyspace '{keyspace}'")]
    KeyNotFound { keyspace: String, key: String },

    /// Keyspace not found
    #[error("Keyspace '{0}' not found")]
    KeyspaceNotFound(String),
}

impl From<KvError> for AppError {
    fn from(e: KvError) -> Self {
        Self::Library(e.into())
    }
}

impl From<WasmError> for AppError {
    fn from(e: WasmError) -> Self {
        Self::Library(e.into())
    }
}

impl From<CanonicalAbiError> for AppError {
    fn from(e: CanonicalAbiError) -> Self {
        Self::Library(e.into())
    }
}

impl From<ValConvertError> for AppError {
    fn from(e: ValConvertError) -> Self {
        Self::WaveParse(e.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self::Library(wit_kv::Error::WitParse(e))
    }
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
        #[arg(short = 't', long)]
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

/// Format an error for user-friendly display
fn format_error(err: &AppError) -> String {
    use std::io::IsTerminal;

    let use_colors = std::io::stderr().is_terminal();

    let (red, yellow, reset) = if use_colors {
        ("\x1b[0;31m", "\x1b[0;33m", "\x1b[0m")
    } else {
        ("", "", "")
    };

    let mut output = format!("{}Error:{} {}\n", red, reset, err);

    // Add hints for common errors
    if let Some(hint) = get_error_hint(err) {
        output.push_str(&format!("{}Hint:{} {}\n", yellow, reset, hint));
    }

    output
}

/// Get a helpful hint for common errors
fn get_error_hint(err: &AppError) -> Option<&'static str> {
    match err {
        AppError::Library(wit_kv::Error::Kv(KvError::NotInitialized(_))) => {
            Some("Run 'wit-kv init --path <PATH>' to initialize a new store")
        }
        AppError::Library(wit_kv::Error::Kv(KvError::KeyspaceNotFound(_)))
        | AppError::KeyspaceNotFound(_) => {
            Some("Run 'wit-kv set-type <KEYSPACE> --wit <FILE>' to register a type first")
        }
        AppError::KeyNotFound { .. } => {
            Some("Use 'wit-kv list <KEYSPACE>' to see available keys")
        }
        AppError::TypeNotFound(_) => {
            Some("Use --type-name to specify the exact type, or check the WIT file for available types")
        }
        AppError::WaveParse(_) => {
            Some("WAVE format: records {field: value}, enums name, variants case(value)")
        }
        AppError::NoTypes => {
            Some("Ensure the WIT file contains at least one type definition")
        }
        AppError::MissingValueInput => {
            Some("Provide a value with --value '{...}' or from a file with --file path.wave")
        }
        _ => None,
    }
}

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        eprint!("{}", format_error(&err));
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), AppError> {
    use std::io::Write;

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
            // Uses Val-based path: binary -> Val -> wasm_wave::Value -> text
            let abi = CanonicalAbi::new(&resolve);
            let ty = Type::Id(type_id);
            let memory = LinearMemory::from_option(export.memory);
            let (val, _bytes_read) = abi.lift_to_val(&export.buffer, &ty, None, &memory)?;
            let value = val_to_wave(&val, &wave_type)?;

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
                    return Err(AppError::KeyspaceNotFound(keyspace));
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
                    return Err(AppError::MissingValueInput);
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
                        let export = BinaryExport::from_stored_owned(stored);
                        let (buffer, memory) = export.encode()?;
                        std::io::stdout().write_all(&buffer)?;
                        std::io::stdout().write_all(&memory)?;
                    }
                    None => {
                        return Err(AppError::KeyNotFound {
                            keyspace,
                            key,
                        });
                    }
                }
            } else {
                match store.get(&keyspace, &key)? {
                    Some(wave_str) => {
                        println!("{}", wave_str);
                    }
                    None => {
                        return Err(AppError::KeyNotFound {
                            keyspace,
                            key,
                        });
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
            let keys = store.list(&keyspace, prefix.as_deref(), None, None, limit)?;
            if keys.is_empty() {
                println!("No keys found");
            } else {
                for key in keys {
                    println!("{}", key);
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
            let mut runner = TypedRunner::new(&module, &module_wit, &input_type, output_type.as_deref())?;
            let metadata = store
                .get_type(&keyspace)?
                .ok_or_else(|| AppError::TypeNotFound(keyspace.clone()))?;

            let keys = collect_keys(&store, &keyspace, key, prefix.as_deref(), start.as_deref(), end.as_deref(), limit)?;
            let mut stats = ProcessingStats::new();

            for k in keys {
                match store.get_raw(&keyspace, &k)? {
                    Some(stored) => {
                        match runner.call_filter(&stored) {
                            Ok(true) => {
                                match runner.call_transform(&stored, metadata.type_version) {
                                    Ok(result) => {
                                        match runner.stored_to_wave_string(&result) {
                                            Ok(wave_str) => println!("{}: {}", k, wave_str),
                                            Err(e) => eprintln!("{}: <decode error: {}>", k, e),
                                        }
                                        stats.transformed += 1;
                                    }
                                    Err(e) => stats.add_error(&k, format!("transform: {}", e)),
                                }
                            }
                            Ok(false) => stats.filtered += 1,
                            Err(e) => stats.add_error(&k, format!("filter: {}", e)),
                        }
                        stats.processed += 1;
                    }
                    None => stats.add_error(&k, "not found".to_string()),
                }
            }

            stats.print_map_summary();
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
            let mut runner = TypedRunner::new(&module, &module_wit, &input_type, Some(&state_type))?;
            let metadata = store
                .get_type(&keyspace)?
                .ok_or_else(|| AppError::TypeNotFound(keyspace.clone()))?;

            let keys = collect_keys(&store, &keyspace, None, prefix.as_deref(), start.as_deref(), end.as_deref(), limit)?;
            let mut state = runner.call_init_state(metadata.type_version)?;
            let mut stats = ProcessingStats::new();

            for k in keys {
                match store.get_raw(&keyspace, &k)? {
                    Some(stored) => {
                        match runner.call_reduce(&state, &stored, metadata.type_version) {
                            Ok(new_state) => {
                                state = new_state;
                                stats.processed += 1;
                            }
                            Err(e) => stats.add_error(&k, format!("reduce: {}", e)),
                        }
                    }
                    None => stats.add_error(&k, "not found".to_string()),
                }
            }

            match runner.stored_to_wave_string(&state) {
                Ok(wave_str) => println!("{}", wave_str),
                Err(e) => eprintln!("<decode error: {}>", e),
            }

            stats.print_reduce_summary();
            Ok(())
        }
    }
}

fn load_wit_type(wit_path: &PathBuf, type_name: Option<&str>) -> Result<(Resolve, TypeId), AppError> {
    let mut resolve = Resolve::new();
    resolve.push_path(wit_path)?;

    let type_id = match type_name {
        Some(name) => {
            find_type_by_name(&resolve, name).ok_or_else(|| AppError::TypeNotFound(name.to_string()))
        }
        None => find_first_named_type(&resolve).ok_or(AppError::NoTypes),
    }?;

    Ok((resolve, type_id))
}

/// Collects keys for processing based on filter options.
fn collect_keys(
    store: &KvStore,
    keyspace: &str,
    key: Option<String>,
    prefix: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<String>, AppError> {
    if let Some(k) = key {
        Ok(vec![k])
    } else {
        Ok(store.list(keyspace, prefix, start, end, limit)?)
    }
}

/// Statistics for map/reduce operations.
struct ProcessingStats {
    processed: usize,
    transformed: usize,
    filtered: usize,
    errors: Vec<(String, String)>,
}

impl ProcessingStats {
    fn new() -> Self {
        Self {
            processed: 0,
            transformed: 0,
            filtered: 0,
            errors: Vec::new(),
        }
    }

    fn add_error(&mut self, key: &str, error: String) {
        self.errors.push((key.to_string(), error));
    }

    fn print_map_summary(&self) {
        eprintln!(
            "Processed {} keys: {} transformed, {} filtered out, {} errors",
            self.processed, self.transformed, self.filtered, self.errors.len()
        );
        for (k, err) in &self.errors {
            eprintln!("  Error for '{}': {}", k, err);
        }
    }

    fn print_reduce_summary(&self) {
        eprintln!(
            "Reduced {} values, {} errors",
            self.processed, self.errors.len()
        );
        for (k, err) in &self.errors {
            eprintln!("  Error for '{}': {}", k, err);
        }
    }
}
