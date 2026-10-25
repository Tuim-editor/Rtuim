use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitFileEntry {
    pub path: String,
    pub index_status: char,
    pub worktree_status: char,
}

impl GitFileEntry {
    pub fn is_staged(&self) -> bool {
        self.index_status != ' ' && self.index_status != '?'
    }

    pub fn is_unstaged(&self) -> bool {
        self.worktree_status != ' ' || self.index_status == '?'
    }

    pub fn status_badge(&self, staged: bool) -> char {
        if staged {
            self.index_status
        } else if self.index_status == '?' {
            '?'
        } else {
            self.worktree_status
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitCommitEntry {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: String,
    pub head: String,
    pub branch: String,
    pub is_bare: bool,
}

#[derive(Clone, Debug, Default)]
pub struct GitSnapshot {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<GitFileEntry>,
    pub commits: Vec<GitCommitEntry>,
    pub branches: Vec<String>,
    pub worktrees: Vec<WorktreeEntry>,
}

impl GitSnapshot {
    pub fn parse_worktrees(output: &str) -> Vec<WorktreeEntry> {
        let mut list = Vec::new();
        let mut cur_path: Option<String> = None;
        let mut cur_head = String::new();
        let mut cur_branch = String::new();
        let mut cur_bare = false;

        for line in output.lines() {
            if line.trim().is_empty() {
                if let Some(p) = cur_path.take() {
                    list.push(WorktreeEntry {
                        path: p,
                        head: std::mem::take(&mut cur_head),
                        branch: std::mem::take(&mut cur_branch),
                        is_bare: cur_bare,
                    });
                    cur_bare = false;
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("worktree ") {
                if let Some(p) = cur_path.take() {
                    list.push(WorktreeEntry {
                        path: p,
                        head: std::mem::take(&mut cur_head),
                        branch: std::mem::take(&mut cur_branch),
                        is_bare: cur_bare,
                    });
                    cur_bare = false;
                }
                cur_path = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("HEAD ") {
                cur_head = rest.trim().chars().take(7).collect();
            } else if let Some(rest) = line.strip_prefix("branch ") {
                let b = rest.trim().strip_prefix("refs/heads/").unwrap_or(rest.trim());
                cur_branch = b.to_string();
            } else if line.trim() == "bare" {
                cur_bare = true;
            }
        }
        if let Some(p) = cur_path.take() {
            list.push(WorktreeEntry {
                path: p,
                head: cur_head,
                branch: cur_branch,
                is_bare: cur_bare,
            });
        }
        list
    }
}

pub fn run_git_in(cwd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn run_git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn collect_snapshot(cwd: &str) -> GitSnapshot {
    let mut snap = GitSnapshot::default();

    if let Some(b) = run_git_in(cwd, &["branch", "--show-current"]) {
        snap.branch = b.trim().to_string();
    }

    if let Some(status_raw) = run_git_in(cwd, &["status", "--porcelain=v1", "-z"]) {
        let sep = if status_raw.as_bytes().contains(&0) {
            '\0'
        } else {
            '\n'
        };
        let mut iter = status_raw.split(sep);
        while let Some(entry) = iter.next() {
            if entry.len() < 4 {
                continue;
            }
            let bytes = entry.as_bytes();
            let x = bytes[0] as char;
            let y = bytes[1] as char;
            let path = entry[3..].to_string();
            if x == 'R' || x == 'C' {
                let _ = iter.next();
            }
            snap.files.push(GitFileEntry {
                path,
                index_status: x,
                worktree_status: y,
            });
        }
    }

    if let Some(log_raw) = run_git_in(
        cwd,
        &["log", "--pretty=format:%h|%an|%ar|%s", "-n", "50"],
    ) {
        for line in log_raw.lines() {
            let mut parts = line.splitn(4, '|');
            if let (Some(h), Some(a), Some(d), Some(m)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            {
                snap.commits.push(GitCommitEntry {
                    hash: h.to_string(),
                    author: a.to_string(),
                    date: d.to_string(),
                    message: m.to_string(),
                });
            }
        }
    }

    if let Some(br_raw) = run_git_in(cwd, &["branch", "-a", "--format=%(refname:short)"]) {
        for line in br_raw.lines() {
            let t = line.trim();
            if !t.is_empty() && !t.contains("/HEAD") {
                snap.branches.push(t.to_string());
            }
        }
    }

    if let Some(wt_raw) = run_git_in(cwd, &["worktree", "list", "--porcelain"]) {
        snap.worktrees = GitSnapshot::parse_worktrees(&wt_raw);
    }

    snap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_parses_worktrees() {
        let raw = "worktree /repo/main\nHEAD 1234567890abcdef\nbranch refs/heads/main\n\nworktree /repo/feat\nHEAD fedcba0987654321\nbranch refs/heads/feat-x\n";
        let wts = GitSnapshot::parse_worktrees(raw);
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].path, "/repo/main");
        assert_eq!(wts[0].branch, "main");
        assert_eq!(wts[1].path, "/repo/feat");
        assert_eq!(wts[1].branch, "feat-x");
    }
}
