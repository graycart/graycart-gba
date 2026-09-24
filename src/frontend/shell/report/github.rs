//! GitHub Issues REST client (user PAT → `graycart/graycart-gba`).

use serde::Deserialize;
use serde_json::json;

pub const OWNER: &str = "graycart";
pub const REPO: &str = "graycart-gba";
const API_BASE: &str = "https://api.github.com";

#[derive(Debug, Clone)]
pub struct CreatedIssue {
    #[allow(dead_code)]
    pub number: u64,
    pub html_url: String,
}

#[derive(Debug, Deserialize)]
struct IssueResponse {
    number: u64,
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    message: Option<String>,
}

/// Trait so unit tests can assert dismiss never calls the API.
pub trait IssueFiler {
    fn create_issue(&self, title: &str, body: &str) -> Result<CreatedIssue, String>;
}

#[derive(Debug, Default)]
pub struct GithubIssueFiler {
    pub token: String,
    pub owner: String,
    pub repo: String,
}

impl GithubIssueFiler {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            owner: OWNER.into(),
            repo: REPO.into(),
        }
    }
}

impl IssueFiler for GithubIssueFiler {
    fn create_issue(&self, title: &str, body: &str) -> Result<CreatedIssue, String> {
        create_issue(&self.token, &self.owner, &self.repo, title, body)
    }
}

/// `POST /repos/{owner}/{repo}/issues` with a fine-grained user PAT.
pub fn create_issue(
    token: &str,
    owner: &str,
    repo: &str,
    title: &str,
    body: &str,
) -> Result<CreatedIssue, String> {
    let url = format!("{API_BASE}/repos/{owner}/{repo}/issues");
    let payload = json!({ "title": title, "body": body });
    let response = ureq::post(&url)
        .set("Accept", "application/vnd.github+json")
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", "graycart-gb-report")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .send_json(payload)
        .map_err(map_ureq_err)?;

    let status = response.status();
    let text = response.into_string().map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        let msg = serde_json::from_str::<ErrorBody>(&text)
            .ok()
            .and_then(|e| e.message)
            .unwrap_or(text);
        return Err(format!("GitHub API HTTP {status}: {msg}"));
    }
    let issue: IssueResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(CreatedIssue {
        number: issue.number,
        html_url: issue.html_url,
    })
}

fn map_ureq_err(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<ErrorBody>(&body)
                .ok()
                .and_then(|e| e.message)
                .unwrap_or(body);
            format!("GitHub API HTTP {code}: {msg}")
        }
        other => format!("GitHub request failed: {other}"),
    }
}

/// Prefill URL for the bug issue form (fallback when token/API fails).
pub fn bug_new_issue_url() -> String {
    format!("https://github.com/{OWNER}/{REPO}/issues/new?template=bug.yml")
}

pub fn feature_new_issue_url() -> String {
    format!("https://github.com/{OWNER}/{REPO}/issues/new?template=feature.yml")
}

pub fn open_url(url: &str) -> Result<(), String> {
    open::that(url).map_err(|e| e.to_string())
}

/// Recording filer for tests — counts calls; never hits the network.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct RecordingFiler {
    pub calls: std::sync::Mutex<Vec<(String, String)>>,
    pub fail: bool,
}

#[cfg(test)]
impl IssueFiler for RecordingFiler {
    fn create_issue(&self, title: &str, body: &str) -> Result<CreatedIssue, String> {
        self.calls
            .lock()
            .unwrap()
            .push((title.to_string(), body.to_string()));
        if self.fail {
            return Err("simulated API failure".into());
        }
        Ok(CreatedIssue {
            number: 42,
            html_url: format!("https://github.com/{OWNER}/{REPO}/issues/42"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_filer_tracks_send_only() {
        let filer = RecordingFiler::default();
        assert!(filer.calls.lock().unwrap().is_empty());
        let created = filer.create_issue("t", "b").unwrap();
        assert_eq!(created.number, 42);
        assert_eq!(filer.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn template_urls_point_at_graycart_gb() {
        assert!(bug_new_issue_url().contains("graycart/graycart-gb"));
        assert!(bug_new_issue_url().contains("bug.yml"));
        assert!(feature_new_issue_url().contains("feature.yml"));
    }
}
