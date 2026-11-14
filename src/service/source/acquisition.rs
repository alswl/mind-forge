//! Bounded HTTP acquisition for Source imports.

use std::io::Read;
use std::time::Duration;

use crate::error::{MfError, Result};

pub fn acquire_http(location: &str, max_bytes: u64, timeout_seconds: u32, max_redirects: u32) -> Result<Vec<u8>> {
    let url =
        reqwest::Url::parse(location).map_err(|_| MfError::usage("registered URL is invalid".to_string(), None))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(MfError::usage("registered URL must use HTTP or HTTPS".to_string(), None));
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds as u64))
        .redirect(reqwest::redirect::Policy::limited(max_redirects as usize))
        .build()
        .map_err(|e| MfError::Io(std::io::Error::other(format!("failed to initialize HTTP client: {e}"))))?;
    let response = client
        .get(url)
        .send()
        .map_err(|e| MfError::Io(std::io::Error::other(format!("HTTP acquisition failed: {e}"))))?;
    if !response.status().is_success() {
        return Err(MfError::Io(std::io::Error::other(format!("HTTP acquisition returned {}", response.status()))));
    }
    if let Some(length) = response.content_length()
        && length > max_bytes
    {
        return Err(MfError::Io(std::io::Error::other("HTTP response exceeds configured byte limit")));
    }
    let mut raw_bytes = Vec::new();
    response
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut raw_bytes)
        .map_err(|e| MfError::Io(std::io::Error::other(format!("failed to read HTTP response: {e}"))))?;
    if raw_bytes.len() as u64 > max_bytes {
        return Err(MfError::Io(std::io::Error::other("HTTP response exceeds configured byte limit")));
    }
    Ok(raw_bytes)
}
