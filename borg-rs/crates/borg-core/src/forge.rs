use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The type of git hosting platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForgeType {
    GitHub,
    GitLab,
    Codeberg,
}

impl std::fmt::Display for ForgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::GitLab => write!(f, "gitlab"),
            Self::Codeberg => write!(f, "codeberg"),
        }
    }
}

/// Detect forge type from a remote URL.
pub fn detect_forge(remote_url: &str) -> ForgeType {
    let lower = remote_url.to_lowercase();
    if lower.contains("gitlab") {
        ForgeType::GitLab
    } else if lower.contains("codeberg") {
        ForgeType::Codeberg
    } else {
        ForgeType::GitHub
    }
}

/// Merge strategy for pull/merge requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

/// Info about a pull/merge request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrInfo {
    pub number: i64,
    pub url: String,
    pub title: String,
    pub state: String,
    pub head_branch: String,
    pub base_branch: String,
}

/// Status of a pull/merge request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrStatus {
    pub number: i64,
    pub state: String,
    pub mergeable: bool,
    pub ci_status: Option<String>,
}

/// A repository issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

/// Abstraction over git hosting platforms (GitHub, GitLab, Codeberg).
/// Handles PR/MR operations, issues, and branch management.
#[async_trait]
pub trait GitForge: Send + Sync {
    async fn create_pr(
        &self,
        repo: &str,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<PrInfo>;

    async fn get_pr_status(&self, repo: &str, pr_number: i64) -> Result<PrStatus>;

    async fn merge_pr(
        &self,
        repo: &str,
        pr_number: i64,
        method: MergeMethod,
    ) -> Result<()>;

    async fn close_pr(&self, repo: &str, pr_number: i64) -> Result<()>;

    async fn list_open_issues(&self, repo: &str, limit: usize) -> Result<Vec<Issue>>;

    async fn get_branch_diff(
        &self,
        repo: &str,
        base: &str,
        head: &str,
    ) -> Result<String>;

    async fn delete_branch(&self, repo: &str, branch: &str) -> Result<()>;

    fn forge_type(&self) -> ForgeType;
}

// ── GitHub Implementation ────────────────────────────────────────────────

/// GitForge implementation using the `gh` CLI.
pub struct GitHubForge {
    token: String,
}

impl GitHubForge {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }

    async fn gh(&self, args: &[&str], repo: Option<&str>) -> Result<String> {
        let mut cmd = tokio::process::Command::new("gh");
        if let Some(r) = repo {
            cmd.args(["--repo", r]);
        }
        cmd.args(args);
        if !self.token.is_empty() {
            cmd.env("GH_TOKEN", &self.token);
        }
        let output = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gh {} failed: {}", args.join(" "), stderr.trim());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[async_trait]
impl GitForge for GitHubForge {
    async fn create_pr(
        &self,
        repo: &str,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<PrInfo> {
        let url = self
            .gh(
                &[
                    "pr", "create",
                    "--head", branch,
                    "--base", base,
                    "--title", title,
                    "--body", body,
                ],
                Some(repo),
            )
            .await?;

        Ok(PrInfo {
            number: extract_pr_number(&url),
            url,
            title: title.to_string(),
            state: "open".into(),
            head_branch: branch.to_string(),
            base_branch: base.to_string(),
        })
    }

    async fn get_pr_status(&self, repo: &str, pr_number: i64) -> Result<PrStatus> {
        let json_str = self
            .gh(
                &[
                    "pr", "view",
                    &pr_number.to_string(),
                    "--json", "number,state,mergeable,statusCheckRollup",
                ],
                Some(repo),
            )
            .await?;

        let v: serde_json::Value = serde_json::from_str(&json_str)?;
        let state = v["state"].as_str().unwrap_or("UNKNOWN").to_lowercase();
        let mergeable = v["mergeable"].as_str().unwrap_or("") == "MERGEABLE";
        let ci_status = v["statusCheckRollup"]
            .as_array()
            .and_then(|checks| {
                if checks.iter().all(|c| {
                    c["conclusion"].as_str().unwrap_or("") == "SUCCESS"
                        || c["status"].as_str().unwrap_or("") == "COMPLETED"
                }) {
                    Some("success".to_string())
                } else if checks.iter().any(|c| {
                    c["conclusion"].as_str().unwrap_or("") == "FAILURE"
                }) {
                    Some("failure".to_string())
                } else {
                    Some("pending".to_string())
                }
            });

        Ok(PrStatus {
            number: pr_number,
            state,
            mergeable,
            ci_status,
        })
    }

    async fn merge_pr(
        &self,
        repo: &str,
        pr_number: i64,
        method: MergeMethod,
    ) -> Result<()> {
        let method_flag = match method {
            MergeMethod::Merge => "--merge",
            MergeMethod::Squash => "--squash",
            MergeMethod::Rebase => "--rebase",
        };
        self.gh(
            &["pr", "merge", &pr_number.to_string(), method_flag, "--auto"],
            Some(repo),
        )
        .await?;
        Ok(())
    }

    async fn close_pr(&self, repo: &str, pr_number: i64) -> Result<()> {
        self.gh(
            &["pr", "close", &pr_number.to_string()],
            Some(repo),
        )
        .await?;
        Ok(())
    }

    async fn list_open_issues(&self, repo: &str, limit: usize) -> Result<Vec<Issue>> {
        let json_str = self
            .gh(
                &[
                    "issue", "list",
                    "--state", "open",
                    "--limit", &limit.to_string(),
                    "--json", "number,title,body,labels",
                ],
                Some(repo),
            )
            .await?;

        let items: Vec<serde_json::Value> = serde_json::from_str(&json_str)?;
        let issues = items
            .iter()
            .map(|v| Issue {
                number: v["number"].as_i64().unwrap_or(0),
                title: v["title"].as_str().unwrap_or("").to_string(),
                body: v["body"].as_str().unwrap_or("").to_string(),
                labels: v["labels"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect();

        Ok(issues)
    }

    async fn get_branch_diff(
        &self,
        repo: &str,
        base: &str,
        head: &str,
    ) -> Result<String> {
        self.gh(
            &["api", &format!("repos/{repo}/compare/{base}...{head}"), "--jq", ".files[].filename"],
            None,
        )
        .await
    }

    async fn delete_branch(&self, repo: &str, branch: &str) -> Result<()> {
        self.gh(
            &["api", "--method", "DELETE", &format!("repos/{repo}/git/refs/heads/{branch}")],
            None,
        )
        .await?;
        Ok(())
    }

    fn forge_type(&self) -> ForgeType {
        ForgeType::GitHub
    }
}

fn extract_pr_number(url: &str) -> i64 {
    url.rsplit('/')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Select the right forge for a repo based on its remote URL.
pub fn forge_for_repo(remote_url: &str, github_token: &str) -> Box<dyn GitForge> {
    match detect_forge(remote_url) {
        ForgeType::GitHub => Box::new(GitHubForge::new(github_token)),
        ForgeType::GitLab => {
            tracing::warn!("GitLab forge not yet implemented, falling back to GitHub CLI");
            Box::new(GitHubForge::new(github_token))
        }
        ForgeType::Codeberg => {
            tracing::warn!("Codeberg forge not yet implemented, falling back to GitHub CLI");
            Box::new(GitHubForge::new(github_token))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_forge_github() {
        assert_eq!(detect_forge("https://github.com/user/repo"), ForgeType::GitHub);
        assert_eq!(detect_forge("git@github.com:user/repo.git"), ForgeType::GitHub);
    }

    #[test]
    fn detect_forge_gitlab() {
        assert_eq!(detect_forge("https://gitlab.com/user/repo"), ForgeType::GitLab);
    }

    #[test]
    fn detect_forge_codeberg() {
        assert_eq!(detect_forge("https://codeberg.org/user/repo"), ForgeType::Codeberg);
    }

    #[test]
    fn detect_forge_unknown_defaults_to_github() {
        assert_eq!(detect_forge("https://example.com/repo"), ForgeType::GitHub);
    }

    #[test]
    fn extract_pr_number_from_url() {
        assert_eq!(extract_pr_number("https://github.com/user/repo/pull/42"), 42);
        assert_eq!(extract_pr_number("not-a-url"), 0);
    }

    #[test]
    fn forge_type_display() {
        assert_eq!(ForgeType::GitHub.to_string(), "github");
        assert_eq!(ForgeType::GitLab.to_string(), "gitlab");
    }
}
