use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod error;
mod fs;
mod inode;
mod schema;
mod store;
mod validate;

/// FUSE filesystem for WIT-typed files.
///
/// Mount a backing directory as a FUSE filesystem where files are validated
/// against WIT type schemas. Each value is accessible as human-readable WAVE
/// text (.wave) and raw canonical ABI binary (.witb).
#[derive(Parser)]
#[command(name = "wit-fs")]
#[command(about = "FUSE filesystem for WIT-typed files")]
struct Cli {
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

        /// Run in foreground (don't daemonize)
        #[arg(long, short)]
        foreground: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Mount {
            backing_dir,
            mountpoint,
            read_only,
            foreground: _,
        } => {
            std::fs::create_dir_all(&backing_dir)?;

            if !mountpoint.exists() {
                std::fs::create_dir_all(&mountpoint)?;
            }

            let backing_store = store::Store::new(backing_dir.clone())?;
            let uid = unsafe { libc::getuid() };
            let gid = unsafe { libc::getgid() };
            let wit_fs = fs::WitFs::new(backing_store, read_only, uid, gid);

            let mut config = fuser::Config::default();
            config.mount_options.push(fuser::MountOption::FSName("wit-fs".to_string()));
            config.mount_options.push(fuser::MountOption::DefaultPermissions);
            if read_only {
                config.mount_options.push(fuser::MountOption::RO);
            }

            let display_path = backing_dir
                .canonicalize()
                .unwrap_or(backing_dir)
                .display()
                .to_string();

            eprintln!("Mounting wit-fs at {}", mountpoint.display());
            eprintln!("Backing store: {display_path}");
            eprintln!("Press Ctrl+C to unmount");

            fuser::mount2(wit_fs, &mountpoint, &config)?;
            Ok(())
        }
    }
}
