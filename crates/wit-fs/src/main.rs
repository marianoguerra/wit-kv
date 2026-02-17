use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod fs;

/// FUSE filesystem for WIT-typed files.
///
/// Mount a backing directory as a FUSE filesystem where files are validated
/// against WIT type schemas. Each value is accessible as human-readable WAVE
/// text (.wave) and raw canonical ABI binary (.witb).
#[derive(Parser)]
#[command(name = "wit-fs")]
#[command(about = "FUSE filesystem for WIT-typed files")]
struct Cli {
    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Mount a WIT-typed filesystem
    Mount {
        /// Path to the backing directory (stores binary data)
        backing_dir: PathBuf,

        /// Path to the mount point
        mountpoint: PathBuf,

        /// Mount as read-only
        #[arg(long)]
        read_only: bool,
    },
}

/// Format an error for user-friendly display.
fn format_error(err: &fs::Error) -> String {
    use std::io::IsTerminal;

    let use_colors = std::io::stderr().is_terminal();

    let (red, yellow, reset) = if use_colors {
        ("\x1b[0;31m", "\x1b[0;33m", "\x1b[0m")
    } else {
        ("", "", "")
    };

    let mut output = format!("{red}Error:{reset} {err}\n");

    if let Some(hint) = get_error_hint(err) {
        output.push_str(&format!("{yellow}Hint:{reset} {hint}\n"));
    }

    output
}

/// Get a helpful hint for common errors.
fn get_error_hint(err: &fs::Error) -> Option<&'static str> {
    match err {
        fs::Error::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Some("Check that you have permission to access the backing directory and mountpoint")
        }
        fs::Error::Io(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Some("Check that the parent directory exists")
        }
        fs::Error::Schema(_) => {
            Some("Ensure .type.wit files contain valid WIT type definitions")
        }
        _ => None,
    }
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging (RUST_LOG env var overrides --log-level)
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    if let Err(err) = run(cli) {
        eprint!("{}", format_error(&err));
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), fs::Error> {
    match cli.command {
        Commands::Mount {
            backing_dir,
            mountpoint,
            read_only,
        } => {
            std::fs::create_dir_all(&backing_dir)?;

            if !mountpoint.exists() {
                std::fs::create_dir_all(&mountpoint)?;
            }

            let backing_store = fs::Store::new(backing_dir.clone())?;
            let uid = unsafe { libc::getuid() };
            let gid = unsafe { libc::getgid() };
            let wit_fs = fs::WitFs::new(backing_store, read_only, uid, gid);

            let mut config = fuser::Config::default();
            config.mount_options.push(fuser::MountOption::FSName("wit-fs".to_string()));
            config.mount_options.push(fuser::MountOption::DefaultPermissions);
            #[cfg(target_os = "macos")]
            config.mount_options.push(fuser::MountOption::CUSTOM("noappledouble".to_string()));
            if read_only {
                config.mount_options.push(fuser::MountOption::RO);
            }

            let display_path = backing_dir
                .canonicalize()
                .unwrap_or(backing_dir)
                .display()
                .to_string();

            tracing::info!(mountpoint = %mountpoint.display(), "Mounting wit-fs");
            tracing::info!(backing_store = %display_path, "Backing store path");
            if read_only {
                tracing::info!("Mounted as read-only");
            }

            fuser::mount2(wit_fs, &mountpoint, &config)?;

            tracing::info!("Filesystem unmounted, shutdown complete");
            Ok(())
        }
    }
}
