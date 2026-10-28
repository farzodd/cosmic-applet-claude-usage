//! Reads Claude Code's OAuth credentials and fetches plan usage limits.
//!
//! Uses the same endpoint as Claude Code's `/usage` command. The access token is
//! short-lived; when it has expired we refresh it with the stored refresh token and
//! write the new pair back so the `claude` CLI keeps working.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const OAUTH_BETA: &str = "oauth-2025-04-20";
/// Claude Code's public OAuth client id.
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// Refresh this long before the token actually expires.
const EXPIRY_MARGIN_MS: i64 = 60_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Limit {
    /// Percent of the window used, 0–100.
    pub utilization: f32,
    pub resets_at: Option<DateTime<Utc>>,
}

impl Limit {
    /// Utilization, treating a window whose reset time has passed as empty.
    pub fn current(&self, now: DateTime<Utc>) -> f32 {
        match self.resets_at {
            Some(reset) if reset <= now => 0.0,
            _ => self.utilization.clamp(0.0, 100.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub session: Option<Limit>,
    pub weekly: Option<Limit>,
    /// Model-specific weekly windows (label, limit), when the plan has them.
    pub extra: Vec<(String, Limit)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// No credentials file, or it has no OAuth entry.
    NotLoggedIn,
    /// The refresh token was rejected; the user has to log in again.
    LoginExpired,
    RateLimited,
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotLoggedIn => write!(f, "Not logged in. Run `claude` and use /login."),
            Error::LoginExpired => write!(f, "Login expired. Run `claude` and use /login."),
            Error::RateLimited => write!(f, "Rate limited by the API; will retry."),
            Error::Other(e) => write!(f, "{e}"),
        }
    }
}

fn other(e: impl std::fmt::Display) -> Error {
    Error::Other(e.to_string())
}

pub fn credentials_path() -> PathBuf {
    let base = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".claude"));
    base.join(".credentials.json")
}

struct Token {
    access: String,
    refresh: String,
    expires_at_ms: i64,
    scopes: Vec<String>,
}

async fn read_credentials() -> Result<(Value, Token), Error> {
    let raw = tokio::fs::read(credentials_path())
        .await
        .map_err(|_| Error::NotLoggedIn)?;
    let doc: Value = serde_json::from_slice(&raw).map_err(other)?;
    let oauth = doc.get("claudeAiOauth").ok_or(Error::NotLoggedIn)?;
    let str_field = |k: &str| oauth.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let token = Token {
        access: str_field("accessToken"),
        refresh: str_field("refreshToken"),
        expires_at_ms: oauth.get("expiresAt").and_then(Value::as_i64).unwrap_or(0),
        scopes: oauth
            .get("scopes")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
    };
    if token.access.is_empty() && token.refresh.is_empty() {
        return Err(Error::NotLoggedIn);
    }
    Ok((doc, token))
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
}

async fn refresh(client: &reqwest::Client, token: &Token) -> Result<String, Error> {
    if token.refresh.is_empty() {
        return Err(Error::LoginExpired);
    }
    let mut body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": token.refresh,
        "client_id": CLIENT_ID,
    });
    if !token.scopes.is_empty() {
        body["scope"] = token.scopes.join(" ").into();
    }
    let resp = client.post(TOKEN_URL).json(&body).send().await.map_err(other)?;
    match resp.status().as_u16() {
        200 => {}
        400 | 401 => return Err(Error::LoginExpired),
        429 => return Err(Error::RateLimited),
        s => return Err(other(format!("Token refresh failed (HTTP {s})"))),
    }
    let new: RefreshResponse = resp.json().await.map_err(other)?;
    let expires_at = Utc::now().timestamp_millis() + new.expires_in * 1000;
    let refresh_token = new.refresh_token.unwrap_or_else(|| token.refresh.clone());

    // Re-read right before writing: if the CLI rotated the tokens meanwhile, keep its copy.
    let (mut doc, current) = read_credentials().await?;
    if current.refresh == token.refresh {
        let oauth = &mut doc["claudeAiOauth"];
        oauth["accessToken"] = new.access_token.clone().into();
        oauth["refreshToken"] = refresh_token.into();
        oauth["expiresAt"] = expires_at.into();
        write_credentials(&doc).await?;
    }
    Ok(new.access_token)
}

async fn write_credentials(doc: &Value) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    let path = credentials_path();
    let tmp = path.with_extension(format!("json.applet-{}", std::process::id()));
    let bytes = serde_json::to_vec(doc).map_err(other)?;
    tokio::fs::write(&tmp, bytes).await.map_err(other)?;
    tokio::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(other)?;
    tokio::fs::rename(&tmp, &path).await.map_err(other)
}

fn parse_limit(v: Option<&Value>) -> Option<Limit> {
    let v = v?;
    let utilization = v.get("utilization")?.as_f64()? as f32;
    let resets_at = v
        .get("resets_at")
        .and_then(Value::as_str)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc));
    Some(Limit { utilization, resets_at })
}

fn parse_usage(v: &Value) -> Usage {
    let extra = [("seven_day_opus", "Weekly · Opus"), ("seven_day_sonnet", "Weekly · Sonnet")]
        .into_iter()
        .filter_map(|(key, label)| parse_limit(v.get(key)).map(|l| (label.to_owned(), l)))
        .collect();
    Usage {
        session: parse_limit(v.get("five_hour")),
        weekly: parse_limit(v.get("seven_day")),
        extra,
    }
}

async fn get_usage(client: &reqwest::Client, access: &str) -> Result<reqwest::Response, Error> {
    client
        .get(USAGE_URL)
        .bearer_auth(access)
        .header("anthropic-beta", OAUTH_BETA)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(other)
}

pub async fn fetch() -> Result<Usage, Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("cosmic-applet-claude-usage/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(other)?;

    let (_, token) = read_credentials().await?;
    let mut access = token.access.clone();
    let mut refreshed = false;
    if access.is_empty() || Utc::now().timestamp_millis() + EXPIRY_MARGIN_MS >= token.expires_at_ms {
        access = refresh(&client, &token).await?;
        refreshed = true;
    }

    let mut resp = get_usage(&client, &access).await?;
    if resp.status().as_u16() == 401 && !refreshed {
        // Token was revoked or expired early; one refresh attempt.
        access = refresh(&client, &token).await?;
        resp = get_usage(&client, &access).await?;
    }
    match resp.status().as_u16() {
        200 => {}
        401 | 403 => return Err(Error::LoginExpired),
        429 => return Err(Error::RateLimited),
        s => return Err(other(format!("Usage request failed (HTTP {s})"))),
    }
    let body: Value = resp.json().await.map_err(other)?;
    Ok(parse_usage(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usage_response() {
        let v = serde_json::json!({
            "five_hour": {"utilization": 42.0, "resets_at": "2026-10-27T20:00:00+00:00"},
            "seven_day": {"utilization": 18.5, "resets_at": null},
            "seven_day_opus": null,
            "seven_day_sonnet": {"utilization": 3, "resets_at": "2026-10-30T15:00:00Z"},
        });
        let u = parse_usage(&v);
        assert_eq!(u.session.as_ref().unwrap().utilization, 42.0);
        assert!(u.session.unwrap().resets_at.is_some());
        assert_eq!(u.weekly.unwrap().resets_at, None);
        assert_eq!(u.extra.len(), 1);
        assert_eq!(u.extra[0].0, "Weekly · Sonnet");
    }

    #[test]
    fn expired_window_reads_zero() {
        let l = Limit { utilization: 80.0, resets_at: Some(Utc::now() - chrono::Duration::minutes(1)) };
        assert_eq!(l.current(Utc::now()), 0.0);
    }
}
