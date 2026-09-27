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

#[cfg(any(target_os = "macos", test))]
fn restart_command(codex_home: &Path) -> std::io::Result<String> {
    // The user's terminal may have a different working directory than Cockpit.
    let codex_home = std::path::absolute(codex_home)?;
    // Quote the exact profile, including spaces, apostrophes and shell metacharacters.
    let home = codex_home.to_string_lossy().replace('\'', "'\"'\"'");
    Ok(format!(
        "CODEX_HOME='{home}' codex app-server daemon restart"
    ))
}

/// Call only after credentials have been committed. The shared daemon may have
/// active work, so leave restarting it to the user and keep the switch successful.
pub async fn restart_notice(codex_home: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    if is_running(codex_home).await {
        return restart_command(codex_home).ok();
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
            Some(restart_command(&home.0).unwrap())
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
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn restart_command_quotes_the_switched_profile() {
        assert_eq!(
            restart_command(Path::new("/tmp/Alice's Codex/$profile")).unwrap(),
            "CODEX_HOME='/tmp/Alice'\"'\"'s Codex/$profile' codex app-server daemon restart",
        );
    }

    #[test]
    fn restart_command_resolves_relative_home_against_cockpit_working_directory() {
        let relative = Path::new("profiles/team-a");
        let absolute = std::env::current_dir().unwrap().join(relative);
        assert_eq!(
            restart_command(relative).unwrap(),
            restart_command(&absolute).unwrap(),
        );
    }

    #[cfg(unix)]
    #[test]
    fn restart_command_keeps_profile_and_arguments_in_a_different_terminal_directory() {
        let relative = Path::new("Alice's Codex/$profile;$(echo wrong)`echo wrong`");
        let expected_home = std::env::current_dir().unwrap().join(relative);
        // A shell function captures arguments only; no real Codex is started.
        let script = format!(
            "codex() {{ printf '%s\\n' \"$CODEX_HOME\" \"$@\"; }}; {}",
            restart_command(relative).unwrap(),
        );
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .current_dir("/")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{}\napp-server\ndaemon\nrestart\n", expected_home.display()),
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn unsupported_platform_does_not_request_a_daemon_restart() {
        assert_eq!(restart_notice(Path::new("profiles/team-a")).await, None);
    }
}
