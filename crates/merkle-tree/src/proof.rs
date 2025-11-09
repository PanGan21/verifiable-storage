use crate::MerkleTreeError;
use serde::{Deserialize, Serialize};
use sha2::Digest;

/// A node in a Merkle proof path, containing a hash and its position.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofNode {
    /// The hash of the sibling node.
    pub hash: [u8; 32],
    /// Whether this sibling is on the left (true) or right (false).
    pub is_left: bool,
}

/// A Merkle proof for a specific leaf node.
/// The proof contains the leaf hash and a path of sibling hashes
/// from the leaf to the root, allowing reconstruction of the root hash.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MerkleProof {
    /// The index of the leaf node this proof is for.
    pub leaf_index: usize,
    /// The hash of the leaf node.
    pub leaf_hash: [u8; 32],
    /// The path of sibling hashes from leaf to root.
    pub path: Vec<ProofNode>,
}

impl MerkleProof {
    /// Compute the root hash from this proof.
    /// This reconstructs the root hash by following the proof path
    /// and hashing pairs of nodes together.
    pub fn compute_root(&self) -> Result<[u8; 32], MerkleTreeError> {
        let mut current_hash = self.leaf_hash;

        for node in &self.path {
            current_hash = if node.is_left {
                // Sibling is on the left, current is on the right
                hash_pair(&node.hash, &current_hash)
            } else {
                // Sibling is on the right, current is on the left
                hash_pair(&current_hash, &node.hash)
            };
        }

        Ok(current_hash)
    }

    /// Get the leaf hash as a hex string.
    pub fn leaf_hash_hex(&self) -> String {
        hex::encode(self.leaf_hash)
    }
}

/// Hash a pair of hashes together (internal node) using SHA-256.
/// Uses domain separation prefix 0x01 for internal nodes.
/// The hashes are concatenated (0x01 || left || right) before hashing.
fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hasher.update([0x01]); // Domain separation prefix for internal nodes
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_serialization() {
        let proof = MerkleProof {
            leaf_index: 0,
            leaf_hash: [0u8; 32],
            path: vec![ProofNode {
                hash: [1u8; 32],
                is_left: true,
            }],
        };

        let json = serde_json::to_string(&proof).unwrap();
        let deserialized: MerkleProof = serde_json::from_str(&json).unwrap();
        assert_eq!(proof, deserialized);
    }
}
