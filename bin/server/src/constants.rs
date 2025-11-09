/// Default maximum age for request timestamps in seconds (5 minutes)
pub const DEFAULT_MAX_AGE_SECONDS: u64 = 300;

/// Default maximum allowed clock skew in seconds (1 minute)
pub const DEFAULT_MAX_CLOCK_SKEW_SECONDS: u64 = 60;

/// Default data directory for filesystem storage
pub const DEFAULT_DATA_DIR: &str = "server_data";

/// Default server host
pub const DEFAULT_HOST: &str = "0.0.0.0";

/// Default server port
pub const DEFAULT_PORT: &str = "8080";

/// Storage type identifier for database
pub const STORAGE_TYPE_DATABASE: &str = "db";

/// Storage type identifier for filesystem (also used as the default storage type)
pub const STORAGE_TYPE_FILESYSTEM: &str = "fs";
