# Architecture & Implementation Documentation

## Overview

This document describes the architecture, design decisions, and implementation of a verifiable storage system that allows clients to upload files to a server, delete local copies, and later download files with cryptographic proofs to verify integrity. The system uses Merkle trees for integrity verification and Ed25519 signatures for authentication.

## Approach

### Problem Statement

The system requirements are:

1. Client uploads a set of files {F0, F1, ..., Fn} to a server
2. Client deletes local copies after upload
3. Client can later download arbitrary file Fi with Merkle proof Pi
4. Client verifies proof against stored root hash to ensure file correctness

### Solution Strategy

**Merkle Tree Approach**: Use a Merkle tree where each leaf represents a file hash. The client computes the root hash after uploading all files and stores it locally. When downloading a file, the server provides the file and a Merkle proof (sibling hashes along the path to root). The client reconstructs the root hash from the proof and compares it with the stored root.

**Key Design Principles**:

- **Cryptographic Security**: Use Ed25519 for authentication, SHA-256 for hashing
- **Minimal Client Storage**: Client only stores root hash (32 bytes)
- **Server Efficiency**: Server builds tree on-demand (no tree storage overhead)
- **Multiple storage support**: Support multiple storage backends, error handling, logging
- **Scalability**: Database backend enables horizontal scaling

### Implementation Highlights

1. **Custom Merkle Tree**: Implemented from scratch (not using a library) with domain separation
2. **Storage Abstraction**: Trait-based design supports filesystem and PostgreSQL
3. **Authentication**: Ed25519 signatures on all requests with auto-registration
4. **Batch Organization**: Files organized by batch_id for logical grouping
5. **Network Protocol**: HTTP REST API for client-server communication
6. **Docker Support**: Docker Compose setup for easy deployment

## Key Features

### 1. Merkle Tree Implementation

The custom Merkle tree implementation is clean and efficient:

- Domain separation (0x00 for leaves, 0x01 for internal nodes) prevents attacks
- Efficient proof generation (O(log n) space complexity)
- Handles odd numbers of files correctly (duplicates last node)
- Well-tested with unit tests

### 2. Storage Abstraction

The `Storage` trait allows easy switching between filesystem and database backends:

- Filesystem good for development and single-instance deployments
- Database enables horizontal scaling
- Clean interface makes testing easier

### 3. Authentication Design

Ed25519 signature-based authentication:

- Fast signature generation and verification
- Small key and signature sizes
- Client ID derived from public key (prevents spoofing)
- Auto-registration simplifies client setup

### 4. Setup

The setup is as following:

- Docker Compose setup
- Database connection retry logic
- Configuration via environment variables
- Health check endpoint
- Structured logging

## Limitations

### 1. Performance

**Limitation**: Server must rebuild Merkle tree on every download request.

**Impact**: For large batches (thousands of files), tree building becomes expensive.

**Solution**: Could cache trees per batch (invalidate on upload). Not implemented due to time constraints.

### 2. Replay Attacks

**Limitation**: No protection against replay attacks.

**Current State**: Timestamps included in signatures but not validated.

**Impact**: An attacker could replay old requests (though they wouldn't gain unauthorized access due to signature verification).

**Solution**: Add timestamp validation or use nonces. Not implemented due to time constraints.

### 3. Batch Size Limits

**Limitation**: No limit on batch size.

**Impact**: Very large batches could cause memory issues or timeouts.

**Solution**: Add configurable batch size limits. Not implemented.

### 4. Concurrent Uploads

**Limitation**: Files uploaded sequentially.

**Impact**: Slow upload for many files.

**Solution**: Implement parallel uploads. Not implemented due to complexity.

## Core Components

### 1. Client

- **Keypair Management**: Generates and stores Ed25519 keypairs
- **Upload**: Reads files, builds Merkle tree, uploads files with signatures
- **Download**: Requests file with proof, verifies against stored root hash
- **Client ID**: Derived from public key (`SHA256(public_key)`)

### 2. Server

- **Authentication**: Verifies Ed25519 signatures on all requests
- **Storage**: Stores files and metadata (filesystem or database)
- **Proof Generation**: Builds Merkle tree on-demand and generates proofs
- **Auto-Registration**: Registers clients on first upload

### 3. Storage Backend

- **Filesystem**: Stores files in directory structure `server_data/{client_id}/{batch_id}/`
- **Database**: PostgreSQL with tables for clients, batches, files, and metadata
- **Abstraction**: `Storage` trait allows switching backends

## System Flow

### Upload Flow

```
1. Client reads files from directory
2. Client sorts filenames (deterministic order)
3. Client builds Merkle tree from all files
4. Client computes root hash
5. For each file:
   - Client signs request (filename, batch_id, file_hash, content, timestamp)
   - Client sends POST /upload with signature and public key
   - Server verifies signature
   - Server stores file and updates metadata
6. Client saves root hash locally
```

### Download Flow

```
1. Client loads root hash from local storage
2. Client signs request (filename, batch_id, timestamp)
3. Client sends GET /download with signature
4. Server verifies signature
5. Server loads batch metadata (filenames)
6. Server reads all files in batch
7. Server builds Merkle tree (sorted by filename)
8. Server generates proof for requested file
9. Server returns file hash and proof
10. Client verifies proof against stored root hash
```

## Design Decisions

### 1. Merkle Trees for Integrity

**Decision**: Use Merkle trees to prove file integrity without storing full tree on server.

**Rationale**:

- Client only needs to store root hash (32 bytes)
- Server builds tree on-demand (no storage overhead)
- Proofs are compact (log(n) nodes for n files)
- Cryptographically secure (any tampering detected)

**Trade-offs**:

- Server must rebuild tree on each download (acceptable for read-heavy workloads)
- Proof size grows logarithmically with batch size (acceptable for typical batch sizes)

### 2. Ed25519 Signatures

**Decision**: Use Ed25519 for request authentication.

**Rationale**:

- Fast signature generation and verification
- Small key and signature sizes (32 bytes key, 64 bytes signature)
- Cryptographically secure
- Standard library support (ed25519_dalek)

**Trade-offs**:

- No replay attack protection (timestamps included but not validated)
- Public keys stored on server (acceptable for this use case)

### 3. Client ID Derivation

**Decision**: Derive client ID from public key (`SHA256(public_key)`).

**Rationale**:

- Prevents client ID spoofing
- Deterministic (same key = same ID)
- No server-side client registration needed
- Auto-registration on first upload

**Trade-offs**:

- Client ID cannot be changed without new keypair
- Public keys must be stored (acceptable for authentication)

### 4. Batch-Based Storage

**Decision**: Organize files by batch_id within client_id.

**Rationale**:

- Clear isolation between upload sessions
- Easy to manage groups of files
- Supports future batch operations (delete, list)
- Metadata tracks filenames per batch

**Trade-offs**:

- Batch ID chosen by client (must be unique per client)
- No automatic batch expiration

### 5. Filename-Based Storage

**Decision**: Store files by original filename, not content hash.

**Rationale**:

- Client requests files by filename
- Simpler API (no hash lookups)
- Supports multiple files with same content
- Direct file access

**Trade-offs**:

- No deduplication (same file uploaded multiple times = multiple copies)
- Filename must be unique within batch

### 6. Storage Abstraction

**Decision**: Abstract storage behind `Storage` trait with filesystem and database implementations.

**Rationale**:

- Easy to switch backends
- Database enables horizontal scaling
- Filesystem good for development/single-instance
- Testable (mock storage for tests)

**Trade-offs**:

- Some complexity in abstraction layer
- Database requires PostgreSQL

### 7. On-Demand Tree Building

**Decision**: Build Merkle tree on server only when generating proof.

**Rationale**:

- No storage overhead for tree structure
- Tree only needed for proofs
- Files can be added to batch without rebuilding tree
- Simplifies storage backend

**Trade-offs**:

- Performance cost on download (must read all files in batch)
- Acceptable for read-heavy workloads with small batches

### 8. Domain Separation in Hashing

**Decision**: Use different prefixes for leaf nodes (0x00) and internal nodes (0x01).

**Rationale**:

- Prevents second-preimage attacks
- Security best practice for Merkle trees
- Minimal performance impact

## Data Structures

### Merkle Tree

```rust
struct MerkleTree {
    root: [u8; 32],
    leaves: Vec<[u8; 32]>,
    levels: Vec<Vec<[u8; 32]>>,
}
```

- **Root**: Merkle root hash
- **Leaves**: Hashes of individual files
- **Levels**: Tree levels for proof generation

### Merkle Proof

```rust
struct MerkleProof {
    leaf_index: usize,
    leaf_hash: [u8; 32],
    path: Vec<ProofNode>,
}

struct ProofNode {
    hash: [u8; 32],
    is_left: bool,
}
```

- **Leaf Hash**: Hash of the file being proved
- **Path**: Sibling hashes from leaf to root
- **Is Left**: Position of sibling in tree

### Storage Structure

**Filesystem:**

```
server_data/
    {client_id}/
        public_key.hex
        {batch_id}/
            {filename}
            metadata.json
```

**Database:**

- `clients`: client_id, public_key
- `batches`: client_id, batch_id
- `files`: client_id, batch_id, filename, content
- `batch_filenames`: client_id, batch_id, filename (metadata)

## Security Considerations

### 1. Signature Verification

- All requests signed with Ed25519
- Server verifies signatures before processing
- Public keys stored securely (filesystem or database)
- Client ID derived from public key (prevents spoofing)

### 2. File Integrity

- Merkle proofs cryptographically prove file belongs to batch
- Root hash stored locally by client (server cannot tamper)
- Domain separation prevents hash collisions
- Any tampering detected during proof verification

### 3. Client Isolation

- Files isolated by client_id
- Clients cannot access other clients' files
- Signature verification ensures client identity
- Batch_id provides additional isolation layer

### 4. Limitations

- **No Replay Protection**: Timestamps included but not validated
- **No Encryption**: Files stored in plaintext (add encryption layer if needed)
- **No Access Control**: All authenticated clients can upload (add authorization if needed)
- **No Rate Limiting**: No protection against DoS (add rate limiting if needed)

## Performance Characteristics

### Upload

- **Complexity**: O(n) where n = number of files
- **Operations**: Read files, build tree, sign requests, upload files
- **Bottleneck**: Network upload speed, signature generation

### Download

- **Complexity**: O(n) where n = number of files in batch
- **Operations**: Read all files in batch, build tree, generate proof
- **Bottleneck**: File I/O (filesystem) or database queries (database)

### Proof Verification

- **Complexity**: O(log n) where n = number of files in batch
- **Operations**: Hash operations along proof path
- **Bottleneck**: Cryptographic hashing (minimal)

## Scalability

### Filesystem Storage

- **Single Instance**: One server instance
- **Limitations**: File system limits, no horizontal scaling
- **Use Case**: Development, small deployments

### Database Storage

- **Multiple Instances**: Can run multiple server instances
- **Shared State**: Database provides shared storage
- **Use Case**: Horizontal scaling
- **Considerations**: Database connection pooling, query optimization

## Future Improvements

Given more time, the following improvements would be prioritized:

### 1. Performance Optimizations (High Priority)

**Merkle Tree Caching**:

- Cache Merkle trees per batch in memory or Redis
- Invalidate cache when new files are uploaded to batch
- Reduces tree building time from O(n) to O(1) for cached batches
- Estimated impact: 10-100x speedup for repeated downloads

**Parallel File Upload**:

- Upload multiple files concurrently (e.g., 10 concurrent uploads)
- Use async/await with tokio for non-blocking I/O
- Estimated impact: 5-10x faster upload for large batches

**Proof Caching**:

- Cache proofs for recently downloaded files (LRU cache)
- Useful when same file is downloaded multiple times
- Estimated impact: Eliminates tree building for cached proofs

**Batch Size Limits**:

- Configurable maximum files per batch (e.g., 10,000 files)
- Prevents memory issues and timeouts
- Client validation before upload

### 2. Security Enhancements (High Priority)

**Replay Attack Prevention**:

- Validate timestamps (reject requests older than 5 minutes)
- Use nonces stored in database (one-time use)
- Estimated effort: 2-4 hours

**HTTPS/TLS**:

- Add TLS support to server (Let's Encrypt certificates)
- Encrypt traffic between client and server
- Essential for production deployment

**File Encryption**:

- Encrypt files at rest (AES-256)
- Client-side encryption before upload
- Server never sees plaintext files

**Rate Limiting**:

- Limit requests per client per minute
- Protect against DoS attacks
- Use token bucket algorithm

### 3. Feature Additions (Medium Priority)

**File Deletion**:

- Add DELETE endpoint with signature
- Store deletion proof (Merkle proof of deletion)
- Allow client to verify file was deleted

**Batch Operations**:

- List all batches for a client
- Get batch metadata (file count, total size, creation date)
- Delete entire batch

**Key Rotation**:

- Allow clients to rotate keys
- Migrate files from old key to new key
- Maintain backward compatibility

**Monitoring & Observability**:

- Prometheus metrics (request rate, latency, error rate)
- Distributed tracing (Jaeger)
- Structured logging with correlation IDs
- Alerting for errors and performance issues

### 4. Storage Optimizations (Medium Priority)

**Deduplication**:

- Store files by content hash
- Reference files by hash in metadata
- Significant storage savings for duplicate files

**Compression**:

- Compress files before storage (gzip, zstd)
- Transparent to client (server handles compression)
- Reduces storage and bandwidth

**Versioning**:

- Support multiple versions of same file
- Store version history
- Allow downloading specific version

### 5. Architecture Improvements (Low Priority)

**Event Sourcing**:

- Store upload/download events
- Rebuild state from events
- Provides audit trail

**CQRS**:

- Separate read/write models
- Optimize read model for queries
- Optimize write model for updates

**Microservices**:

- Split into upload service, download service, storage service
- Better scalability and fault isolation
- More complex deployment

## Implementation Statistics

- **Lines of Code**: ~2,800 lines of Rust source code
- **Crates**: 4 crates (common, crypto, merkle-tree, storage) + 2 binaries (client, server)
- **Dependencies**: Minimal (ed25519_dalek, sha2, sqlx, actix-web, serde, tokio)
- **Test Coverage**: Unit tests for Merkle tree, integration tests for storage
- **Documentation**: README, architecture document, code comments
- **Merkle Tree**: Custom implementation (not using library) with domain separation

## Deployment

### Docker Compose (Recommended)

The simplest way to run the system is using Docker Compose, which starts both the server and PostgreSQL database:

```bash
docker compose up --build
```

The server will be available at `http://localhost:8080` with PostgreSQL database storage. Configuration is handled through environment variables in `docker-compose.yml`.

### Filesystem Storage

For development or single-instance deployments, you can use filesystem storage instead of a database:

```bash
# Kill any existing server on port 8080
lsof -ti :8080 | xargs kill -9 2>/dev/null || true

# Start server with filesystem storage (default)
cargo run --release --bin server
```

The server will store files in the `server_data/` directory by default. You can specify a custom data directory:

```bash
cargo run --release --bin server -- --storage fs --data-dir /path/to/data
```

**Use Cases for Filesystem Storage:**

- Local development and testing
- Single-instance deployments
- Simple deployments without database infrastructure
- Quick prototyping

**Limitations:**

- No horizontal scaling (single server instance only)
- File system limits apply
- No shared state across multiple server instances

### Database Storage (Local)

To run the server locally with PostgreSQL database storage:

```bash
# Start PostgreSQL database
docker compose up -d postgres

# Set database URL
export DATABASE_URL="postgresql://verifiable_storage:verifiable_storage_password@localhost:5432/verifiable_storage"

# Start server with database storage
cargo run --release --bin server -- --storage db
```

**Use Cases for Database Storage:**

- Horizontal scaling (multiple server instances)
- Shared state across server instances
- Better performance for large datasets

### Configuration

Server configuration can be set via environment variables or command-line arguments:

- `SERVER_HOST`: Server host (default: `127.0.0.1`)
- `SERVER_PORT`: Server port (default: `8080`)
- `DATABASE_URL`: PostgreSQL connection string (required for database storage)
- `RUST_LOG`: Logging level (default: `info`)

Example:

```bash
export SERVER_HOST=0.0.0.0
export SERVER_PORT=8080
export RUST_LOG=debug
cargo run --release --bin server
```

## Conclusion

The verifiable storage system successfully implements all requirements:

- ✅ Custom Merkle tree implementation
- ✅ Client uploads files and stores root hash
- ✅ Client can download arbitrary file with proof
- ✅ Client verifies proof against stored root hash
- ✅ Networking across multiple machines
- ✅ Clean, modular code architecture

The system balances simplicity with security, using well-established cryptographic primitives. Key strengths include the custom Merkle tree implementation, storage abstraction, and networking features like Docker support and database backend.

Main limitations are performance (no caching), security (no replay protection), and features (no file deletion endpoint). These can be addressed in future iterations with the improvements outlined above.

## Diagrams

### System Architecture

```
┌─────────┐         ┌─────────┐
│ Client  │────────▶│ Server  │
│         │◀────────│         │
└─────────┘         └─────────┘
                         │
                         ▼
                   ┌─────────┐
                   │ Storage │
                   │ Backend │
                   └─────────┘
                         │
            ┌────────────┴────────────┐
            ▼                         ▼
      ┌──────────┐             ┌──────────┐
      │Filesystem│             │Database  │
      └──────────┘             └──────────┘
```

### Upload Flow

```
Client                          Server
  │                               │
  ├─ Read files                   │
  ├─ Build Merkle tree            │
  ├─ Compute root hash            │
  │                               │
  ├─ Sign request ───────────────▶│
  │                               ├─ Verify signature
  │                               ├─ Store file
  │                               ├─ Update metadata
  │◀────────────── 200 OK ────────┤
  │                               │
  ├─ Save root hash               │
  │                               │
```

### Download Flow

```
Client                          Server
  │                               │
  ├─ Load root hash               │
  ├─ Sign request ───────────────▶│
  │                               ├─ Verify signature
  │                               ├─ Load batch metadata
  │                               ├─ Read all files
  │                               ├─ Build Merkle tree
  │                               ├─ Generate proof
  │◀────── File + Proof ──────────┤
  │                               │
  ├─ Verify proof                 │
  ├─ Save file                    │
  │                               │
```

### Merkle Tree Structure

```
                    Root
                   /    \
              Hash01    Hash23
              /   \      /   \
          Hash0  Hash1 Hash2 Hash3
            │      │     │     │
          File0  File1 File2 File3
```

Proof for File0: [Hash1, Hash23] (siblings along path to root)
