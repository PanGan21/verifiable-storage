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

1. **Custom Merkle Tree**: Domain-separated implementation (0x00 for leaves, 0x01 for internal nodes)
2. **Storage Abstraction**: Trait-based design supports filesystem and PostgreSQL backends
3. **Authentication**: Ed25519 signatures on all requests with auto-registration
4. **Client-Side Encryption**: AES-256-GCM encryption before upload, decryption after download
5. **Security**: Filename validation, timestamp validation, log sanitization, atomic operations
6. **Batch Organization**: Files organized by batch_id for logical grouping
7. **Multipart File Uploads**: Standard HTTP multipart/form-data format for efficient file transfers

## Key Features

### 1. Merkle Tree Implementation

- Domain separation (0x00 for leaves, 0x01 for internal nodes) prevents second-preimage attacks
- Efficient proof generation (O(log n) space complexity)
- Handles odd numbers of files correctly (duplicates last node)

### 2. Storage Abstraction

The `Storage` trait allows switching between filesystem and database backends:

- **Filesystem**: Development and single-instance deployments
- **Database**: Horizontal scaling with PostgreSQL

### 3. Authentication

Ed25519 signature-based authentication:

- Client ID derived from public key (`SHA256(public_key)`)
- Auto-registration on first upload
- All requests signed and verified

### 4. Security Features

**Filename Validation**: Prevents path traversal attacks by validating filenames (no path separators, no special directories)

**Replay Attack Prevention**: Timestamp validation on all requests (default: 5 minutes max age, 1 minute clock skew)

**Log Sanitization**: Uses `tracing` structured logging with Debug formatter to automatically escape control characters and prevent log injection

**Atomic Operations**:

- Database: PostgreSQL transactions ensure file and metadata are stored atomically
- Filesystem: `fsync()` ensures data persistence

## Limitations

### 1. Performance

Server must rebuild Merkle tree on every download request. For large batches (thousands of files), tree building becomes expensive. Future improvement: precompute and persist tree nodes.

### 2. Batch Size Limits

No limit on batch size. Very large batches could cause memory issues or timeouts. Future improvement: configurable batch size limits.

### 3. Concurrent Uploads

Files uploaded sequentially. Future improvement: parallel uploads with bounded concurrency.

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

## System Flow

### Upload Flow

```
1. Client reads plaintext files from directory
2. Client validates each filename (prevents path traversal)
3. Client encrypts each file using AES-256-GCM (key derived from Ed25519 signing key)
4. Client sorts filenames (deterministic order)
5. Client builds Merkle tree from encrypted files
6. Client computes root hash from encrypted data
7. For each encrypted file:
   - Client builds message: filename || batch_id || file_hash || encrypted_content || timestamp
   - Client signs message with Ed25519 private key
   - Client sends POST /upload with multipart/form-data (encrypted file + metadata fields)
   - Server validates form fields (length, format)
   - Server validates filename (path traversal protection)
   - Server validates timestamp (replay attack prevention)
   - Server verifies signature
   - Server stores encrypted file and metadata atomically
8. Client saves root hash locally (hash of encrypted Merkle tree)
```

### Download Flow

```
1. Client loads root hash from local storage (hash of encrypted Merkle tree)
2. Client validates filename (prevents path traversal)
3. Client builds message: filename || batch_id || timestamp
4. Client signs message with Ed25519 private key
5. Client sends GET /download with signature (query parameters)
6. Server validates filename (path traversal protection)
7. Server validates timestamp (replay attack prevention)
8. Server verifies signature
9. Server loads batch metadata (filenames)
10. Server reads all encrypted files in batch
11. Server builds Merkle tree from encrypted files (sorted by filename)
12. Server generates proof for requested encrypted file
13. Server returns encrypted file hash and proof (JSON response with base64-encoded encrypted file)
14. Client verifies encrypted file hash matches downloaded encrypted content
15. Client verifies Merkle proof against stored root hash (proof is for encrypted data)
16. Client decrypts encrypted file to get plaintext
17. Client saves both encrypted (.encrypted suffix) and decrypted files (for demo purposes)
```

## Design Decisions

### 1. Merkle Trees for Integrity

Use Merkle trees to prove file integrity without storing full tree on server. Client stores only root hash (32 bytes), server builds tree on-demand. Proofs are compact (log(n) nodes for n files).

**Trade-off**: Server must rebuild tree on each download, but this avoids storage overhead.

### 2. Ed25519 Signatures

Use Ed25519 for request authentication. Fast verification, small key/signature sizes, cryptographically secure.

### 3. Client ID Derivation

Derive client ID from public key (`SHA256(public_key)`). This design prevents users from uploading files to other users' batches, which would break Merkle proofs.

**Security Rationale**: If a user could upload files to another user's batch, the Merkle tree would include files from multiple clients. When the original client downloads a file and verifies the proof against their stored root hash, the proof would fail because the tree structure has changed (files from different clients have different hashes and would produce a different root). By deriving client ID from the public key and enforcing that all uploads to a batch must be signed by the same key, we ensure batch integrity: all files in a batch belong to the same client who computed the original root hash.

**Additional Benefits**: Prevents client ID spoofing, deterministic (same key = same ID), enables auto-registration.

**Trade-off**: Client ID cannot be changed without new keypair.

### 4. Batch-Based Storage

Organize files by batch_id within client_id. Clear isolation between upload sessions, supports future batch operations.

**Trade-off**: Batch ID chosen by client, must be unique per client.

### 5. Filename-Based Storage

Store files by original filename, not content hash. Simpler API, supports multiple files with same content.

**Trade-off**: No deduplication (same file uploaded multiple times = multiple copies).

### 6. Storage Abstraction

Abstract storage behind `Storage` trait. Enables switching between filesystem (development) and database (scaling) backends.

### 7. On-Demand Tree Building

Build Merkle tree on server only when generating proof. No storage overhead, but requires reading all files in batch.

### 8. Domain Separation

Use different prefixes for leaf nodes (0x00) and internal nodes (0x01) to prevent second-preimage attacks.

## Data Structures

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

### 4. Path Traversal Protection

- **Filename Validation**: All filenames validated to prevent path traversal attacks
- Validates no path separators (`/`, `\`) in filenames
- Rejects special directory names (`.`, `..`)
- Ensures filenames are valid file names (not paths)
- Returns 400 Bad Request for invalid filenames
- Implemented in both client and server for defense in depth

### 5. Replay Attack Prevention

- **Timestamp Validation**: All requests validated for timestamp freshness
- Default: 5 minutes maximum age, 1 minute clock skew tolerance
- Configurable via constants
- Prevents replay of old requests
- Returns 401 Unauthorized for expired or future-dated requests

### 6. Atomic Operations

- **Database**: Transactions ensure file and metadata are stored atomically
- **Filesystem**: Fsync ensures data is persisted before returning success
- Prevents inconsistent state (file without metadata or corrupted files)

### 7. Encryption at Rest

- **Client-Side Encryption**: Files encrypted before upload using AES-256-GCM
- **Encryption Key**: Derived from Ed25519 signing key using HKDF
- **Deterministic Nonce**: Nonce derived from filename + batch_id (no nonce storage needed)
- **Server Never Sees Plaintext**: Server only stores encrypted bytes
- **Merkle Tree from Encrypted Data**: Root hash computed from encrypted files
- **Transparent Decryption**: Files automatically decrypted on download

### 8. Limitations
- **No Access Control**: All authenticated clients can upload
- **No Rate Limiting**: No protection against DoS
- **No TLS**: Server does not enforce TLS (must be behind TLS proxy in production)

## Performance Characteristics

- **Upload**: O(n) - Read files, build tree, sign requests, upload files
- **Download**: O(n) - Read all files in batch, build tree, generate proof
- **Proof Verification**: O(log n) - Hash operations along proof path

## Scalability

- **Filesystem**: Single instance, development/small deployments
- **Database**: Multiple instances, horizontal scaling, shared state via PostgreSQL

## Future Improvements

### High Priority

- **Precompute and Persist Merkle Tree Nodes**: Store tree structure for large batches to generate proofs quickly
- **Cache Leaf Hashes**: Store leaf hashes on upload to avoid reading content to recompute
- **TLS & Hardened Deployment**: Server must be behind TLS in production (document setup)
- **Rate Limiting**:
  - Per-client rate limiting (requests per minute/hour)
  - Per-IP rate limiting for unauthenticated endpoints (health check)
  - Configurable limits via environment variables or config file
  - Token bucket or sliding window algorithm
  - Return 429 Too Many Requests with Retry-After header
  - Integration with Actix Web middleware (e.g., `actix-governor` or custom middleware)
- **Storage Quotas**: Per-client storage quotas (max files, max total size per batch)
- **Key Rotation, Revocation, and Admin Controls**: Key rotation endpoint, revocation, admin controls for batch management

### Medium Priority

- **Batch Uploads in Parallel**: Upload multiple files concurrently with bounded concurrency
- **Get Root Hash Endpoint**: Endpoint to GET persisted root hash for a batch (owner-only)
- **File Deletion**: DELETE endpoint with signature and deletion proof
- **Batch Operations**: List batches, get batch metadata, delete entire batch
- **Monitoring & Observability**: Prometheus metrics, storage backend IO timings, health checks
- **Deduplication**: Store files by content hash to save storage
- **Compression**: Compress files before storage (gzip, zstd)

### Low Priority

- **Versioning**: Support multiple versions of same file
- **Event Sourcing**: Store upload/download events for audit trail
- **CQRS**: Separate read/write models for scalability
- **Microservices**: Split into upload/download/storage services

## Deployment

### TLS Required in Production

**IMPORTANT**: The server does not include built-in TLS support. In production, the server **MUST** be deployed behind a TLS-terminating reverse proxy (nginx, traefik, etc.) or load balancer with TLS enabled.

**Do NOT expose the server directly to the internet without TLS**. All traffic should be encrypted in transit.

### Docker Compose (Recommended)

The simplest way to run the system is using Docker Compose, which starts both the server and PostgreSQL database:

```bash
docker compose up --build
```

The server will be available at `http://localhost:8080` with PostgreSQL database storage. Configuration is handled through environment variables in `docker-compose.yml`.

**For Production**:

1. Deploy behind nginx/traefik with TLS certificates (Let's Encrypt)
2. Configure reverse proxy to forward requests to server
3. Enable rate limiting at the proxy level
4. Set up monitoring and logging
5. Use environment variables for sensitive configuration

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

Server configuration via environment variables:

- `SERVER_HOST`: Server host (default: `0.0.0.0`)
- `SERVER_PORT`: Server port (default: `8080`)
- `DATABASE_URL`: PostgreSQL connection string (required for database storage)
- `RUST_LOG`: Logging level (default: `info`)

### Production Deployment

1. Deploy behind TLS-terminating reverse proxy (nginx/traefik)
2. Configure rate limiting at proxy level
3. Set up monitoring and health checks
4. Use connection pooling for database
5. Regular backups of database and filesystem storage
