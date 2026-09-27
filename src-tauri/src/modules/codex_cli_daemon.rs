//! Detect the shared CLI daemon without restarting it or interrupting its sessions.

use std::path::Path;

/// The daemon control socket is scoped to CODEX_HOME (Codex CLI 0.157.1).
/// A successful connection distinguishes a live daemon from a stale socket file.
#[cfg(any(target_os = "macos", all(test, unix)))]
async fn is_running(codex_home: &Path) -> bool {
    let socket = codex_home.join("app-server-control/app-server-control.sock");
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            tokio::net::UnixStream::connect(socket),
        )
        .await,
        Ok(Ok(_))
    )
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn restart_command(codex_home: &Path) -> String {
    // Quote the exact profile, including spaces, apostrophes and shell metacharacters.
    let home = codex_home.to_string_lossy().replace('\'', "'\"'\"'");
    format!("CODEX_HOME='{home}' codex app-server daemon restart")
}

/// Call only after credentials have been committed. The shared daemon may have
/// active work, so leave restarting it to the user and keep the switch successful.
pub async fn restart_notice(codex_home: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    if is_running(codex_home).await {
        return Some(restart_command(codex_home));
    }
    let _ = codex_home;
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestHome(PathBuf);

    impl TestHome {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            // Keep the Unix socket path below macOS's sockaddr_un limit.
            let path = PathBuf::from("/tmp").join(format!(
                "cockpit-daemon-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir_all(path.join("app-server-control")).unwrap();
            Self(path)
        }

        fn socket(&self) -> PathBuf {
            self.0.join("app-server-control/app-server-control.sock")
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn detects_only_a_live_daemon_in_the_switched_home() {
        let home = TestHome::new();
        let other_home = TestHome::new();
        assert!(!is_running(&home.0).await);

        let listener = UnixListener::bind(home.socket()).unwrap();
        assert!(is_running(&home.0).await);
        assert!(!is_running(&other_home.0).await);

        #[cfg(target_os = "macos")]
        assert_eq!(
            restart_notice(&home.0).await,
            Some(restart_command(&home.0))
        );

        drop(listener);
        assert!(home.socket().exists());
        assert!(!is_running(&home.0).await);
        assert_eq!(restart_notice(&home.0).await, None);
    }

    #[tokio::test]
    async fn a_regular_file_is_not_a_running_daemon() {
        let home = TestHome::new();
        std::fs::write(home.socket(), b"not a socket").unwrap();
        assert!(!is_running(&home.0).await);
    }

    #[test]
    fn restart_command_quotes_the_switched_profile() {
        assert_eq!(
            restart_command(Path::new("/tmp/Alice's Codex/$profile")),
            "CODEX_HOME='/tmp/Alice'\"'\"'s Codex/$profile' codex app-server daemon restart",
        );
    }
}
