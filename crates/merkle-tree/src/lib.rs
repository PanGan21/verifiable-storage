use sha2::{Digest, Sha256};
use std::fmt;
use thiserror::Error;

pub mod proof;
pub use proof::*;

#[derive(Debug, Error)]
pub enum MerkleTreeError {
    #[error("Empty data provided")]
    EmptyData,
    #[error("Invalid leaf index: {0}")]
    InvalidLeafIndex(usize),
}

/// A Merkle tree that can be used to verify data integrity.
/// The tree is built bottom-up from a collection of data items.
/// Each leaf node is the hash of a data item, and internal nodes
/// are hashes of their children.
#[derive(Clone, Debug)]
pub struct MerkleTree {
    root: [u8; 32],
    leaves: Vec<[u8; 32]>,
    levels: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    /// Build a Merkle tree from a collection of data items.
    /// Each item is hashed to create a leaf node. If there's an odd number
    /// of nodes at any level, the last node is duplicated.
    pub fn from_data(data: &[Vec<u8>]) -> Result<Self, MerkleTreeError> {
        if data.is_empty() {
            return Err(MerkleTreeError::EmptyData);
        }

        // Hash each data item to create leaf nodes
        let leaves: Vec<[u8; 32]> = data.iter().map(|item| hash_data(item)).collect();

        // Build the tree level by level
        let mut levels = Vec::new();
        levels.push(leaves.clone());

        let mut current_level = leaves.clone();
        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            // Process pairs of nodes
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    // Two siblings: hash them together
                    let hash = hash_pair(&current_level[i], &current_level[i + 1]);
                    next_level.push(hash);
                } else {
                    // Odd number: duplicate the last node
                    let hash = hash_pair(&current_level[i], &current_level[i]);
                    next_level.push(hash);
                }
            }

            levels.push(next_level.clone());
            current_level = next_level;
        }

        let root = current_level[0];

        Ok(MerkleTree {
            root,
            leaves,
            levels,
        })
    }

    /// Get the root hash of the Merkle tree.
    pub fn root_hash(&self) -> [u8; 32] {
        self.root
    }

    /// Get the number of leaf nodes (data items) in the tree.
    pub fn num_leaves(&self) -> usize {
        self.leaves.len()
    }

    /// Generate a Merkle proof for the leaf at the given index.
    /// A Merkle proof consists of sibling hashes along the path from
    /// the leaf to the root, along with their positions (left or right).
    pub fn generate_proof(&self, leaf_index: usize) -> Result<MerkleProof, MerkleTreeError> {
        if leaf_index >= self.leaves.len() {
            return Err(MerkleTreeError::InvalidLeafIndex(leaf_index));
        }

        let mut path = Vec::new();
        let mut current_index = leaf_index;

        // Traverse from leaf to root
        for level in 0..(self.levels.len() - 1) {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            // Check if sibling exists at this level
            if sibling_index < self.levels[level].len() {
                let sibling_hash = self.levels[level][sibling_index];
                let is_left = sibling_index < current_index;
                path.push(ProofNode {
                    hash: sibling_hash,
                    is_left,
                });
            } else {
                // Odd node at the end: sibling is itself
                let sibling_hash = self.levels[level][current_index];
                let is_left = false; // Convention: duplicate is always on the right
                path.push(ProofNode {
                    hash: sibling_hash,
                    is_left,
                });
            }

            current_index /= 2;
        }

        Ok(MerkleProof {
            leaf_index,
            leaf_hash: self.leaves[leaf_index],
            path,
        })
    }

    /// Verify a Merkle proof against the stored root hash.
    /// This reconstructs the root hash from the proof and compares it
    /// with the stored root hash.
    pub fn verify_proof(&self, proof: &MerkleProof) -> Result<bool, MerkleTreeError> {
        let computed_root = proof.compute_root()?;
        Ok(computed_root == self.root)
    }

    /// Verify a Merkle proof given only the root hash.
    /// This is a static method that can be used when only the root hash
    /// and a proof are provided, without the full tree.
    pub fn verify_proof_with_root(
        root_hash: &[u8; 32],
        proof: &MerkleProof,
    ) -> Result<bool, MerkleTreeError> {
        let computed_root = proof.compute_root()?;
        Ok(computed_root == *root_hash)
    }
}

impl fmt::Display for MerkleTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MerkleTree(root: {})", hex::encode(self.root))
    }
}

/// Hash a single data item (leaf node) using SHA-256.
///
/// Uses domain separation prefix 0x00 for leaves to prevent
/// second-preimage attacks between leaf and internal nodes.
fn hash_data(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(&[0x00]); // Domain separation prefix for leaves
    hasher.update(data);
    hasher.finalize().into()
}

/// Hash a pair of hashes together (internal node) using SHA-256.
///
/// Uses domain separation prefix 0x01 for internal nodes.
/// The hashes are concatenated (0x01 || left || right) before hashing.
fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(&[0x01]); // Domain separation prefix for internal nodes
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_item() {
        let data = vec![b"hello".to_vec()];
        let tree = MerkleTree::from_data(&data).unwrap();
        let root = tree.root_hash();

        // Single item: root should be hash of the item
        let expected = hash_data(b"hello");
        assert_eq!(root, expected);
    }

    #[test]
    fn test_two_items() {
        let data = vec![b"file1".to_vec(), b"file2".to_vec()];
        let tree = MerkleTree::from_data(&data).unwrap();

        // Verify proof for first item
        let proof = tree.generate_proof(0).unwrap();
        assert!(tree.verify_proof(&proof).unwrap());

        // Verify proof for second item
        let proof2 = tree.generate_proof(1).unwrap();
        assert!(tree.verify_proof(&proof2).unwrap());
    }

    #[test]
    fn test_three_items() {
        let data = vec![b"file1".to_vec(), b"file2".to_vec(), b"file3".to_vec()];
        let tree = MerkleTree::from_data(&data).unwrap();

        // Verify all proofs
        for i in 0..3 {
            let proof = tree.generate_proof(i).unwrap();
            assert!(tree.verify_proof(&proof).unwrap());
        }
    }

    #[test]
    fn test_verify_with_root() {
        let data = vec![b"file1".to_vec(), b"file2".to_vec(), b"file3".to_vec()];
        let tree = MerkleTree::from_data(&data).unwrap();
        let root = tree.root_hash();
        let proof = tree.generate_proof(1).unwrap();

        // Verify using static method
        assert!(MerkleTree::verify_proof_with_root(&root, &proof).unwrap());

        // Also verify that wrong root fails
        let mut wrong_root = root;
        wrong_root[0] ^= 0xFF;
        assert!(!MerkleTree::verify_proof_with_root(&wrong_root, &proof).unwrap());
    }

    #[test]
    fn test_invalid_proof() {
        let data = vec![b"file1".to_vec(), b"file2".to_vec()];
        let tree = MerkleTree::from_data(&data).unwrap();
        let proof = tree.generate_proof(0).unwrap();

        // Modify the proof
        let mut bad_proof = proof.clone();
        bad_proof.path[0].hash[0] ^= 0xFF;

        // Should fail verification
        assert!(!tree.verify_proof(&bad_proof).unwrap());
    }

    #[test]
    fn test_empty_data() {
        let data: Vec<Vec<u8>> = vec![];
        let result = MerkleTree::from_data(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_leaf_index() {
        let data = vec![b"file1".to_vec(), b"file2".to_vec()];
        let tree = MerkleTree::from_data(&data).unwrap();
        let result = tree.generate_proof(10);
        assert!(result.is_err());
    }
}
