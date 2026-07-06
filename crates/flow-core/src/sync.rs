use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get the board path from env or default.
pub fn board_path() -> PathBuf {
    if let Ok(p) = std::env::var("FLOW_BOARD_PATH") {
        PathBuf::from(p)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".flow/boards/principal")
    } else {
        PathBuf::from(".flow/boards/principal")
    }
}

/// Check if git is installed.
pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if the board directory is already a git repo.
pub fn is_git_repo(path: &PathBuf) -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Initialize a git repo in the board directory.
pub fn init(path: &PathBuf) -> Result<(), String> {
    if !git_available() {
        return Err("Git is not installed. Install git first.".to_string());
    }
    let out = Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to run git init: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

/// Add or update the git remote origin.
pub fn set_remote(path: &PathBuf, url: &str) -> Result<(), String> {
    // Remove existing origin if any
    let _ = Command::new("git")
        .args(["remote", "remove", "origin"])
        .current_dir(path)
        .output();

    let out = Command::new("git")
        .args(["remote", "add", "origin", url])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to add remote: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

/// Get the remote URL.
pub fn get_remote(path: &PathBuf) -> Result<String, String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to get remote: {e}"))?;
    if !out.status.success() {
        return Err("No remote configured. Use `flow sync init --remote <url>` first.".to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Default commit message with timestamp.
fn default_message() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("flow: sync {}", ts)
}

/// Stage all changes, commit, and push to remote.
pub fn push(path: &PathBuf, message: Option<&str>) -> Result<String, String> {
    let default = default_message();
    let msg = message.unwrap_or(&default);

    // git add -A
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to git add: {e}"))?;
    if !add.status.success() {
        return Err(format!("git add failed: {}", String::from_utf8_lossy(&add.stderr)));
    }

    // git commit (may fail if nothing to commit — that's fine)
    let _commit = Command::new("git")
        .args(["commit", "-m", msg])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to git commit: {e}"))?;

    // git push
    let push = Command::new("git")
        .args(["push", "-u", "origin", "HEAD"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to git push: {e}"))?;
    if !push.status.success() {
        let stderr = String::from_utf8_lossy(&push.stderr);
        if stderr.contains("nothing to commit") {
            return Ok("Nothing to commit — board is up to date.".to_string());
        }
        return Err(format!("git push failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&push.stdout).to_string();
    let stderr = String::from_utf8_lossy(&push.stderr).to_string();
    Ok(format!("{}{}", stdout, stderr).trim().to_string())
}

/// Pull latest changes from remote (fast-forward only).
pub fn pull(path: &PathBuf) -> Result<String, String> {
    // First fetch
    let fetch = Command::new("git")
        .args(["fetch", "origin"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to git fetch: {e}"))?;
    if !fetch.status.success() {
        return Err(format!("git fetch failed: {}", String::from_utf8_lossy(&fetch.stderr)));
    }

    // Try to rebase or merge
    let merge = Command::new("git")
        .args(["merge", "--ff-only", "origin/HEAD"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to git merge: {e}"))?;
    if !merge.status.success() {
        let stderr = String::from_utf8_lossy(&merge.stderr);
        return Err(format!("git merge failed (resolve conflicts manually): {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&merge.stdout).to_string();
    let stderr = String::from_utf8_lossy(&merge.stderr).to_string();
    let result = format!("{}{}", stdout, stderr).trim().to_string();
    if result.is_empty() {
        Ok("Already up to date.".to_string())
    } else {
        Ok(result)
    }
}

/// Show working tree status.
pub fn status(path: &PathBuf) -> Result<String, String> {
    let out = Command::new("git")
        .args(["status"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to git status: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Show recent commit log.
pub fn log(path: &PathBuf, n: usize) -> Result<String, String> {
    let out = Command::new("git")
        .args(["log", &format!("-{n}"), "--oneline"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to git log: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Full init: creates git repo, adds all files, commits, sets remote, pushes.
/// If the repo already exists, just sets the remote and pulls.
pub fn full_init(path: &PathBuf, remote_url: &str) -> Result<String, String> {
    let mut output = Vec::new();

    if !is_git_repo(path) {
        init(path)?;
        output.push("Git repo initialized.".to_string());

        // Initial commit of existing files
        let add = Command::new("git")
            .args(["add", "-A"])
            .current_dir(path)
            .output()
            .map_err(|e| format!("git add failed: {e}"))?;
        if !add.status.success() {
            return Err(format!("git add failed: {}", String::from_utf8_lossy(&add.stderr)));
        }

        let commit = Command::new("git")
            .args(["commit", "-m", "flow: initial board"])
            .current_dir(path)
            .output()
            .map_err(|e| format!("git commit failed: {e}"))?;
        if commit.status.success() {
            output.push("Initial commit created.".to_string());
        }
    } else {
        output.push("Git repo already exists.".to_string());
    }

    // Set remote
    set_remote(path, remote_url)?;
    output.push(format!("Remote set: {remote_url}"));

    // Push (or pull if remote already has data)
    let push_out = Command::new("git")
        .args(["push", "-u", "origin", "HEAD"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("git push failed: {e}"))?;

    if push_out.status.success() {
        output.push("Pushed to remote.".to_string());
    } else {
        let stderr = String::from_utf8_lossy(&push_out.stderr);
        if stderr.contains("failed to push") || stderr.contains("fetch first") {
            // Remote has data, pull instead
            output.push("Remote has existing data — pulling...".to_string());
            let pull_out = Command::new("git")
                .args(["pull", "--rebase", "--autostash"])
                .current_dir(path)
                .output()
                .map_err(|e| format!("git pull failed: {e}"))?;
            if pull_out.status.success() {
                output.push("Pull successful.".to_string());
                // Push after pull
                let push2 = Command::new("git")
                    .args(["push", "-u", "origin", "HEAD"])
                    .current_dir(path)
                    .output()
                    .map_err(|e| format!("git push failed: {e}"))?;
                if push2.status.success() {
                    output.push("Pushed to remote.".to_string());
                } else {
                    output.push(format!(
                        "Push failed (may need manual resolve): {}",
                        String::from_utf8_lossy(&push2.stderr)
                    ));
                }
            } else {
                output.push(format!(
                    "Pull failed: {}",
                    String::from_utf8_lossy(&pull_out.stderr)
                ));
            }
        } else {
            output.push(format!("Warning: {}", stderr.trim()));
        }
    }

    Ok(output.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_message_format() {
        let msg = default_message();
        assert!(msg.starts_with("flow: sync "));
        let ts_part = msg.trim_start_matches("flow: sync ");
        assert!(ts_part.parse::<u64>().is_ok());
    }

    #[test]
    fn board_path_uses_env() {
        // Test with FLOW_BOARD_PATH set
        std::env::set_var("FLOW_BOARD_PATH", "/tmp/test-flow-board");
        let p = board_path();
        assert_eq!(p, PathBuf::from("/tmp/test-flow-board"));
        std::env::remove_var("FLOW_BOARD_PATH");
    }
}
