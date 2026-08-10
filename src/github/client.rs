use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::models::{CIStatus, PRRequest, ReviewStatus};

pub struct GithubClient;

impl GithubClient {
    pub fn new() -> Self {
        Self
    }

    pub fn my_open_prs(&self) -> Result<Vec<PRRequest>> {
        let output = Command::new("gh")
            .args([
                "search",
                "prs",
                "--author=@me",
                "--state=open",
                "--json",
                "repository,number,title,url,isDraft,author",
            ])
            .output()
            .context("failed to execute gh CLI")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            anyhow::bail!("gh command failed: {stderr}");
        }

        let github_prs: Vec<GhPullRequest> =
            serde_json::from_slice(&output.stdout).context("failed to parse gh JSON output")?;

        let prs = github_prs.into_iter().map(PRRequest::from).collect();

        Ok(prs)
    }
}

#[derive(Debug, Deserialize)]
struct GhPullRequest {
    repository: GhRepository,
    number: u64,
    title: String,
    url: String,

    #[serde(rename = "isDraft")]
    is_draft: bool,

    author: GhAuthor,
}

#[derive(Debug, Deserialize)]
struct GhRepository {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

#[derive(Debug, Deserialize)]
struct GhAuthor {
    login: String,
}

impl From<GhPullRequest> for PRRequest {
    fn from(pr: GhPullRequest) -> Self {
        Self {
            repo: pr.repository.name_with_owner,
            number: pr.number,
            title: pr.title,
            url: pr.url,
            author: pr.author.login,
            is_draft: pr.is_draft,

            // We haven't fetched these yet.
            review_status: ReviewStatus::Unknown,
            ci_status: CIStatus::Unknown,
        }
    }
}
