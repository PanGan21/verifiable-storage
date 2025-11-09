use std::path::Path;

/// Error type for filename validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilenameValidationError {
    Empty,
    ContainsNullByte,
    ContainsPathSeparator,
    IsSpecialDirectory,
    InvalidFileName,
    ContainsInvalidCharacters,
}

impl FilenameValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            FilenameValidationError::Empty => "Filename cannot be empty",
            FilenameValidationError::ContainsNullByte => "Filename cannot contain null bytes",
            FilenameValidationError::ContainsPathSeparator => {
                "Filename cannot contain path separators (/ or \\)"
            }
            FilenameValidationError::IsSpecialDirectory => "Filename cannot be '.' or '..'",
            FilenameValidationError::InvalidFileName => {
                "Invalid filename: must be a valid file name"
            }
            FilenameValidationError::ContainsInvalidCharacters => {
                "Invalid filename: contains invalid characters"
            }
        }
    }
}

impl std::fmt::Display for FilenameValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for FilenameValidationError {}

/// Validate filename to prevent path traversal attacks
/// Checks if:
/// - Filename contains no path separators (/, \)
/// - Path::new(filename).file_name() returns Some(_)
/// - Filename is not empty
/// - Filename is not "." or ".."
pub fn validate_filename(filename: &str) -> Result<(), FilenameValidationError> {
    if filename.is_empty() {
        return Err(FilenameValidationError::Empty);
    }

    // Check for null bytes (not allowed in filenames)
    if filename.contains('\0') {
        return Err(FilenameValidationError::ContainsNullByte);
    }

    // Check for path separators
    if filename.contains('/') || filename.contains('\\') {
        return Err(FilenameValidationError::ContainsPathSeparator);
    }

    // Check for special directory names
    if filename == "." || filename == ".." {
        return Err(FilenameValidationError::IsSpecialDirectory);
    }

    // Check if Path::new(filename).file_name() returns Some(_)
    // This ensures the filename is valid and not a path like "/"
    let path = Path::new(filename);
    if path.file_name().is_none() {
        return Err(FilenameValidationError::InvalidFileName);
    }

    // Additional check: ensure the file_name matches the original filename
    // This prevents cases where the filename might be normalized to something else
    if path.file_name().and_then(|n| n.to_str()) != Some(filename) {
        return Err(FilenameValidationError::ContainsInvalidCharacters);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_filename() {
        assert!(validate_filename("file.txt").is_ok());
        assert!(validate_filename("my-file_123.txt").is_ok());
        assert!(validate_filename("file").is_ok());
    }

    #[test]
    fn test_empty_filename() {
        assert_eq!(validate_filename(""), Err(FilenameValidationError::Empty));
    }

    #[test]
    fn test_path_separators() {
        assert_eq!(
            validate_filename("path/to/file.txt"),
            Err(FilenameValidationError::ContainsPathSeparator)
        );
        assert_eq!(
            validate_filename("path\\to\\file.txt"),
            Err(FilenameValidationError::ContainsPathSeparator)
        );
        assert_eq!(
            validate_filename("/file.txt"),
            Err(FilenameValidationError::ContainsPathSeparator)
        );
        assert_eq!(
            validate_filename("\\file.txt"),
            Err(FilenameValidationError::ContainsPathSeparator)
        );
        assert_eq!(
            validate_filename("file.txt/"),
            Err(FilenameValidationError::ContainsPathSeparator)
        );
        assert_eq!(
            validate_filename("file.txt\\"),
            Err(FilenameValidationError::ContainsPathSeparator)
        );
    }

    #[test]
    fn test_path_traversal() {
        assert_eq!(
            validate_filename(".."),
            Err(FilenameValidationError::IsSpecialDirectory)
        );
        assert_eq!(
            validate_filename("."),
            Err(FilenameValidationError::IsSpecialDirectory)
        );
        assert_eq!(
            validate_filename("../file.txt"),
            Err(FilenameValidationError::ContainsPathSeparator)
        );
        assert_eq!(
            validate_filename("..\\file.txt"),
            Err(FilenameValidationError::ContainsPathSeparator)
        );
    }

    #[test]
    fn test_invalid_characters() {
        // Null byte
        assert_eq!(
            validate_filename("file\0.txt"),
            Err(FilenameValidationError::ContainsNullByte)
        );
    }
}
