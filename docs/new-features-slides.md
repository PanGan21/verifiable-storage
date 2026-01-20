# New Features - PowerPoint Bullet Points

## Feature 1: Merkle Tree Storage

### What it does:
• Server computes and stores Merkle tree structure on every upload
• Tree rebuilt automatically after each file upload
• Stored in database (JSONB) or filesystem (JSON)

### Benefits:
• **Fast proof generation** - O(log n) operations, no file I/O needed
• **Efficient storage** - Only stores hashes, not file contents
• **Automatic updates** - Tree rebuilt on re-uploads
• **Download performance** - Proofs generated instantly from stored tree

### How it works:
• Upload: Store leaf hash → Load all leaf hashes → Rebuild tree → Store tree
• Download: Load stored tree → Generate proof (no file reading)
• Re-upload: Update leaf hash → Rebuild tree → Update stored tree

---

## Feature 2: Client-Side Encryption

### What it does:
• Files encrypted before upload using AES-256-GCM
• Encryption key derived from Ed25519 signing key (HKDF)
• Merkle tree built from encrypted data
• Server never sees plaintext

### Benefits:
• **End-to-end encryption** - Server stores only encrypted bytes
• **Zero-knowledge storage** - Server cannot read file contents
• **Cryptographic security** - AES-256-GCM with deterministic nonces
• **Transparent decryption** - Automatic decryption on download

### How it works:
• Client: Encrypt files → Build Merkle tree from encrypted data → Upload
• Server: Stores encrypted files and builds tree from encrypted data
• Client: Downloads encrypted file → Verifies proof → Decrypts to plaintext

### Security:
• Encryption key: Derived from Ed25519 signing key using HKDF
• Nonce: Deterministic (filename + batch_id) - no storage needed
• Domain separation: Merkle tree uses encrypted data for integrity
