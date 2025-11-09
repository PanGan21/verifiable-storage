# Verifiable Storage System

A Rust implementation of a verifiable storage system where clients can upload files to a server and later download them with cryptographic proofs to verify integrity.

## Features

- **Ed25519 Signatures**: Cryptographic authentication for all requests
- **Merkle Tree Verification**: Cryptographic proofs for file integrity
- **Batch-Based Storage**: Files organized by batch_id for isolation
- **Flexible Backends**: Filesystem or PostgreSQL database storage
- **Multi-Client Support**: Each client has a unique identity derived from their public key

## Quick Start

### Build

```bash
cargo build --release
```

### Start Server

**Option A: Docker Compose (Recommended for Demo)**

```bash
docker compose up --build
```

**Option B: Local Server with Filesystem Storage**

```bash
lsof -ti :8080 | xargs kill -9 2>/dev/null || true
cargo run --release --bin server
```

**Option C: Local Server with Database Storage**

```bash
docker compose up -d postgres
export DATABASE_URL="postgresql://verifiable_storage:verifiable_storage_password@localhost:5432/verifiable_storage"
cargo run --release --bin server -- --storage db
```

## Quick Demo

### Step 1: Start Server with Docker Compose

```bash
# Start server and database
docker compose up --build

# Verify server is running
curl http://localhost:8080/health
```

### Step 2: Create Test Files

```bash
# Create test files for client 1
mkdir -p client1_files
echo "Client 1 - File 1 Content" > client1_files/file1.txt
echo "Client 1 - File 2 Content" > client1_files/file2.txt
echo "Client 1 - File 3 Content" > client1_files/file3.txt

# Create test files for client 2
mkdir -p client2_files
echo "Client 2 - File A Content" > client2_files/fileA.txt
echo "Client 2 - File B Content" > client2_files/fileB.txt
echo "Client 2 - File C Content" > client2_files/fileC.txt
```

### Step 3: Client 1 - Upload and Download

```bash
# Set client 1 data directory
export CLIENT_DATA_DIR="client1_data"

# Generate keypair for client 1
cargo run --release --bin client generate-keypair

# Upload files
cargo run --release --bin client upload \
    --dir client1_files \
    --server http://127.0.0.1:8080 \
    --batch-id client1-batch-001

# Download and verify file1.txt
cargo run --release --bin client download file1.txt \
    --batch-id client1-batch-001 \
    --server http://127.0.0.1:8080

# Download and verify file2.txt
cargo run --release --bin client download file2.txt \
    --batch-id client1-batch-001 \
    --server http://127.0.0.1:8080
```

### Step 4: Client 2 - Upload and Download

```bash
# Set client 2 data directory (different from client 1)
export CLIENT_DATA_DIR="client2_data"

# Generate keypair for client 2
cargo run --release --bin client generate-keypair

# Upload files
cargo run --release --bin client upload \
    --dir client2_files \
    --server http://127.0.0.1:8080 \
    --batch-id client2-batch-001

# Download and verify fileA.txt
cargo run --release --bin client download fileA.txt \
    --batch-id client2-batch-001 \
    --server http://127.0.0.1:8080

# Download and verify fileB.txt
cargo run --release --bin client download fileB.txt \
    --batch-id client2-batch-001 \
    --server http://127.0.0.1:8080
```

### Verify Client Isolation

```bash
# Check client IDs are different
cat client1_data/client_id.txt
cat client2_data/client_id.txt

# Check root hashes are different (different files)
cat client1_data/client1-batch-001/root_hash.txt
cat client2_data/client2-batch-001/root_hash.txt
```

**Expected Results:**

- Each client has a unique client ID
- Each client's files are stored separately on the server
- Clients cannot access each other's files (signature verification ensures isolation)
- Downloaded files match original files (Merkle proof verification succeeds)

## Advanced Usage

### Server Configuration

```bash
# Filesystem storage with custom data directory
cargo run --release --bin server -- --storage fs --data-dir /path/to/data

# Database storage with custom URL
cargo run --release --bin server -- --storage db --database-url "postgresql://..."

# Custom host and port
export SERVER_HOST=0.0.0.0
export SERVER_PORT=8080
cargo run --release --bin server
```

## Client Configuration

```bash
# Custom server URL
export CLIENT_SERVER_URL="http://localhost:8080"

# Custom data directory
export CLIENT_DATA_DIR="custom_client_data"
```

## Alternative: Local Server (Without Docker)

If you prefer to run the server locally without Docker:

```bash
# Start server with filesystem storage
lsof -ti :8080 | xargs kill -9 2>/dev/null || true
cargo run --release --bin server
```

Then use `http://127.0.0.1:8080` as the server URL in the demo above.

## Documentation

- **[docs/architecture.md](docs/architecture.md)** - Architecture, design decisions, implementation documentation, and improvements

## System Requirements

This implementation satisfies all system requirements:

✅ **Custom Merkle Tree**: Implemented from scratch (not using a library)  
✅ **Single Root Hash**: Client stores root hash in `client_data/{batch_id}/root_hash.txt`  
✅ **Arbitrary File Download**: Client can download any file by filename with Merkle proof  
✅ **Proof Verification**: Client verifies proof against stored root hash  
✅ **Rust Implementation**: Entire system written in Rust  
✅ **Networking**: HTTP REST API works across multiple machines  
✅ **Production-Ready**: Docker Compose, database support, error handling, logging  
✅ **Docker Compose Demo**: See Docker Compose section above

## File Deletion

After uploading files and verifying the upload was successful, clients can safely delete local copies. The client stores:

- Root hash in `client_data/{batch_id}/root_hash.txt`
- File list in `client_data/{batch_id}/filenames.json`

Files can be recovered later by downloading with Merkle proof verification.

## Project Structure

```
verifiable-storage/
├── crates/
│   ├── common/         # Shared types (requests, responses)
│   ├── crypto/         # Cryptographic utilities
│   ├── merkle-tree/    # Merkle tree implementation
│   └── storage/        # Storage abstraction (filesystem/database)
├── bin/
│   ├── client/         # Client binary
│   └── server/         # Server binary
└── docs/
    └── architecture.md # Architecture documentation
```

## Key Concepts

- **Client ID**: SHA256 hash of public key - unique client identity
- **Batch ID**: Identifier for a group of files uploaded together
- **Merkle Proof**: Cryptographic proof that a file belongs to a batch
- **Root Hash**: Merkle root computed from all files in a batch (stored locally by client)

## Requirements Checklist

✅ **Custom Merkle Tree**: Implemented from scratch in `crates/merkle-tree/`  
✅ **Single Root Hash**: Stored in `client_data/{batch_id}/root_hash.txt`  
✅ **Arbitrary File Download**: Download any file by filename with proof  
✅ **Proof Verification**: Client verifies proof against stored root hash  
✅ **Rust Language**: Entire system written in Rust  
✅ **Networking**: HTTP REST API works across multiple machines  
✅ **Production-Ready**: Docker, database, error handling, logging  
✅ **Docker Compose**: `docker-compose up --build` starts everything  
✅ **Documentation**: See [docs/architecture.md](docs/architecture.md)

## License

MIT OR Apache-2.0
