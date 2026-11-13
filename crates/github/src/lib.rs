//! The GitHub connector (LC-28). U2 contains the token test: who the token belongs to, when it
//! expires, and which repositories it can reach (BR-17.3). Clone, push and pull-request calls
//! arrive in U5 behind the same connector interface.
//!
//! The API base is fixed to `https://api.github.com` (BR-17.1) and the token goes only in the
//! `Authorization` header, marked sensitive. Pagination links in answers are never followed: the
//! page number is read from them and our own request is built.

use async_trait::async_trait;
use serde_json::Value;
use tandem_core::config::{
    MAX_REPOSITORIES_LISTED, ProbeDetails, ProbeResult, ProbeSubject, TestStatus,
};
use tandem_core::outbound_url::OutboundUrl;
use tandem_outbound::{OutboundClient, OutboundError, Request, Response, Target};
use tandem_ports::{ConnectorConnection, ConnectorProbe};
use time::OffsetDateTime;
use time::macros::format_description;

const API_BASE: &str = "https://api.github.com";
const REPOSITORIES_PER_PAGE: u32 = 100;

/// Tests a GitHub personal access token.
#[derive(Debug, Clone)]
pub struct GithubConnector {
    client: OutboundClient,
    api_base: OutboundUrl,
}

impl GithubConnector {
    /// A connector for the real GitHub API.
    pub fn new(client: OutboundClient) -> Self {
        let api_base = match OutboundUrl::parse(API_BASE) {
            Ok(url) => url,
            Err(_) => unreachable!("the GitHub API base is a valid public https URL"),
        };
        Self { client, api_base }
    }

    #[cfg(test)]
    fn with_api_base(mut self, api_base: OutboundUrl) -> Self {
        self.api_base = api_base;
        self
    }

    async fn get(
        &self,
        connection: &ConnectorConnection,
        path: &str,
    ) -> Result<Response, OutboundError> {
        let request = Request::get()
            .header("accept", "application/vnd.github+json", false)?
            .header("x-github-api-version", "2022-11-28", false)?
            .bearer(connection.token.expose())?;
        self.client
            .send(Target::Github, &self.api_base, path, request)
            .await
    }

    async fn inspect(&self, connection: &ConnectorConnection) -> Result<ProbeDetails, TestStatus> {
        let user = self
            .get(connection, "/user")
            .await
            .map_err(OutboundError::test_status)?;
        check(&user)?;
        let login = serde_json::from_slice::<Value>(&user.body)
            .ok()
            .and_then(|body| body.get("login")?.as_str().map(str::to_owned))
            .ok_or(TestStatus::BadResponse)?;
        let expires_at = user
            .header("github-authentication-token-expiration")
            .and_then(parse_expiry);

        let first = self.repositories(connection, 1).await?;
        let total = match first.last_page {
            Some(last) if last > 1 && first.names.len() == REPOSITORIES_PER_PAGE as usize => {
                let last_page = self.repositories(connection, last).await?;
                (last - 1) * REPOSITORIES_PER_PAGE + count(last_page.names.len())
            }
            _ => count(first.names.len()),
        };
        let mut names = first.names;
        names.truncate(MAX_REPOSITORIES_LISTED);
        Ok(ProbeDetails {
            account_login: Some(login),
            token_expires_at: expires_at,
            ..ProbeDetails::default()
        }
        .with_repositories(names, total))
    }

    async fn repositories(
        &self,
        connection: &ConnectorConnection,
        page: u32,
    ) -> Result<RepositoryPage, TestStatus> {
        let path = format!("/user/repos?per_page={REPOSITORIES_PER_PAGE}&page={page}");
        let response = self
            .get(connection, &path)
            .await
            .map_err(OutboundError::test_status)?;
        check(&response)?;
        let list = serde_json::from_slice::<Value>(&response.body)
            .ok()
            .and_then(|body| {
                body.as_array().map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.get("full_name")?.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
            })
            .ok_or(TestStatus::BadResponse)?;
        Ok(RepositoryPage {
            names: list,
            last_page: response.header("link").and_then(last_page_number),
        })
    }
}

struct RepositoryPage {
    names: Vec<String>,
    last_page: Option<u32>,
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

/// Map an error answer to a status; remote text is never kept.
fn check(response: &Response) -> Result<(), TestStatus> {
    match response.status {
        200..=299 => Ok(()),
        401 | 403 => Err(TestStatus::Unauthorised),
        _ => Err(TestStatus::BadResponse),
    }
}

/// GitHub's expiry header looks like `2026-12-01 10:00:00 UTC`.
fn parse_expiry(text: &str) -> Option<OffsetDateTime> {
    let trimmed = text.trim().strip_suffix(" UTC")?;
    time::PrimitiveDateTime::parse(
        trimmed,
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
    )
    .ok()
    .map(time::PrimitiveDateTime::assume_utc)
}

/// The page number of the `rel="last"` link in a `Link` header.
fn last_page_number(link: &str) -> Option<u32> {
    let entry = link
        .split(',')
        .find(|entry| entry.contains("rel=\"last\""))?;
    let query = entry.split_once('?')?.1;
    let query = query.split('>').next()?;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("page=")?.parse().ok())
}

#[async_trait]
impl ConnectorProbe for GithubConnector {
    async fn test(&self, connection: &ConnectorConnection) -> ProbeResult {
        match self.inspect(connection).await {
            Ok(details) => {
                let mut result = ProbeResult::new(ProbeSubject::Connector, TestStatus::Ok);
                result.details = details;
                result
            }
            Err(status) => ProbeResult::new(ProbeSubject::Connector, status),
        }
    }
}

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-github";

#[cfg(test)]
mod tests;
