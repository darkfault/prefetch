//! Security utilities for daemon mode.
//!
//! Provides safe socket creation and file permission enforcement.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Create a Unix domain socket path with restrictive permissions.
///
/// The socket file and its parent directory are set to 0700 (owner-only)
/// to prevent other local users from connecting and issuing commands.
///
/// # Security
///
/// Without restrictive permissions, any local user could:
/// - Trigger excessive IO by issuing warm commands
/// - Query cache status to learn which models other users are running
/// - Cause denial-of-service by flooding the socket with requests
pub fn prepare_socket_path(socket_path: &Path) -> anyhow::Result<()> {
    // Create parent directory if needed
    if let Some(parent) = socket_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
        // Set directory permissions to 0700 (owner only)
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        tracing::debug!(
            path = %parent.display(),
            "set socket directory permissions to 0700"
        );
    }

    // Remove stale socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    Ok(())
}

/// Verify that a socket file has secure permissions after creation.
///
/// Call this after binding the socket to ensure the file permissions
/// are restrictive.
pub fn verify_socket_permissions(socket_path: &Path) -> anyhow::Result<()> {
    if !socket_path.exists() {
        anyhow::bail!("socket file does not exist: {}", socket_path.display());
    }

    let metadata = std::fs::metadata(socket_path)?;
    let mode = metadata.permissions().mode();

    // Check that group and other have no access
    if mode & 0o077 != 0 {
        tracing::warn!(
            path = %socket_path.display(),
            mode = format!("{:04o}", mode),
            "socket file has overly permissive permissions, tightening to 0700"
        );
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}

/// Get the default IPC socket path.
pub fn default_socket_path() -> std::path::PathBuf {
    // Prefer XDG_RUNTIME_DIR (Linux) — it's per-user, tmpfs-backed, and
    // automatically cleaned up on logout.
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return std::path::PathBuf::from(runtime_dir).join("prefetch.sock");
    }

    // macOS fallback: use TMPDIR (per-user temp directory)
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        return std::path::PathBuf::from(tmpdir).join("prefetch.sock");
    }

    // Last resort: include PID to avoid collisions
    std::path::PathBuf::from("/tmp").join(format!(
        "prefetch-{}.sock",
        std::process::id()
    ))
}

/// Validate that a data directory has appropriate permissions.
///
/// The history database contains usage patterns that could reveal
/// what models a user is working with.
pub fn ensure_data_dir_permissions(data_dir: &Path) -> anyhow::Result<()> {
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir)?;
    }

    // Set to 0700 — only the owner should access usage history
    std::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o700))?;

    tracing::debug!(
        path = %data_dir.display(),
        "ensured data directory has 0700 permissions"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_socket_path_not_empty() {
        let path = default_socket_path();
        assert!(!path.as_os_str().is_empty());
        assert!(path.file_name().is_some());
    }

    #[test]
    fn test_socket_path_contains_identifier() {
        let path = default_socket_path();
        let filename = path.file_name().unwrap().to_string_lossy();
        assert!(filename.contains("prefetch"));
    }
}
