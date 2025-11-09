//! Verifiable storage client

mod config;
mod download;
mod keypair;
mod upload;

use clap::{Parser, Subcommand};
use config::CLIENT_DATA_DIR;
use keypair::{generate_keypair_command, get_or_create_keypair};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "client")]
#[command(about = "Verifiable storage client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new Ed25519 keypair
    GenerateKeypair {
        /// Force generation even if keypair already exists
        #[arg(short, long)]
        force: bool,
    },
    /// Upload files to server
    Upload {
        /// Directory containing files
        #[arg(short, long)]
        dir: PathBuf,
        /// Server URL
        #[arg(short, long, default_value = "http://127.0.0.1:8080")]
        server: String,
        /// Batch ID for this upload (all files in this upload belong to the same batch)
        #[arg(short, long)]
        batch_id: String,
    },
        /// Download and verify a file from server
        Download {
            /// Filename to download
            filename: String,
            /// Batch ID this file belongs to
            #[arg(short, long)]
            batch_id: String,
            /// Server URL
            #[arg(short, long, default_value = "http://127.0.0.1:8080")]
            server: String,
            /// Root hash to verify against (if not provided, loads from client_data/{batch_id}/root_hash.txt)
            #[arg(short, long)]
            root_hash: Option<String>,
            /// Output directory for downloaded file (default: client_data/{batch_id}/downloaded/)
            #[arg(short, long)]
            output_dir: Option<PathBuf>,
        },
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    // Handle generate-keypair command separately (doesn't need existing keypair)
    if let Commands::GenerateKeypair { force } = &cli.command {
        return generate_keypair_command(*force);
    }

    // For all other commands, load or create keypair
    let (signing_key, client_id) = get_or_create_keypair()?;

    // Save client ID to file for easy access
    let client_id_file = PathBuf::from(CLIENT_DATA_DIR).join("client_id.txt");
    if let Some(parent) = client_id_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&client_id_file, &client_id)?;

    match cli.command {
        Commands::GenerateKeypair { .. } => {
            unreachable!("GenerateKeypair should have been handled earlier")
        }
        Commands::Upload {
            dir,
            server,
            batch_id,
        } => {
            upload::upload_files(&dir, &server, &batch_id, &signing_key)?;
        }
        Commands::Download {
            filename,
            batch_id,
            server,
            root_hash,
            output_dir,
        } => {
            // Load root hash from file if not provided
            let root_hash = root_hash.unwrap_or_else(|| {
                download::load_root_hash(&batch_id).expect("Failed to load root hash")
            });
            download::download_file(
                &server,
                &filename,
                &batch_id,
                &signing_key,
                &root_hash,
                output_dir.as_ref(),
            )?;
        }
    }

    Ok(())
}
