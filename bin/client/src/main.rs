mod config;
mod download;
mod keypair;
mod logger;
mod upload;

use clap::{Parser, Subcommand};
use config::ClientConfig;
use keypair::{generate_keypair_command, get_or_create_keypair};
use logger::init as init_logger;
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
        /// Server URL (defaults to CLIENT_SERVER_URL env var or http://127.0.0.1:8080)
        #[arg(short, long)]
        server: Option<String>,
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
        /// Server URL (defaults to CLIENT_SERVER_URL env var or http://127.0.0.1:8080)
        #[arg(short, long)]
        server: Option<String>,
        /// Root hash to verify against (if not provided, loads from client_data/{batch_id}/root_hash.txt)
        #[arg(short, long)]
        root_hash: Option<String>,
        /// Output directory for downloaded file (default: client_data/{batch_id}/downloaded/)
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    init_logger();

    let cli = Cli::parse();
    let config = ClientConfig::load();

    if let Commands::GenerateKeypair { force } = &cli.command {
        return generate_keypair_command(&config.data_dir, *force);
    }

    let (signing_key, client_id) = get_or_create_keypair(&config.data_dir)?;

    let client_id_file = config.data_dir.join("client_id.txt");
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
            let server_url = config.get_server_url(server.as_deref());
            upload::upload_files(&dir, &server_url, &batch_id, &signing_key, &config.data_dir)?;
        }
        Commands::Download {
            filename,
            batch_id,
            server,
            root_hash,
            output_dir,
        } => {
            let server_url = config.get_server_url(server.as_deref());
            let root_hash = root_hash.unwrap_or_else(|| {
                download::load_root_hash(&batch_id, &config.data_dir).expect("Failed to load root hash")
            });
            download::download_file(
                &server_url,
                &filename,
                &batch_id,
                &signing_key,
                &root_hash,
                output_dir.as_ref(),
                &config.data_dir,
            )?;
        }
    }

    Ok(())
}
