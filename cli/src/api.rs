use std::process::Command;

use crate::crypto::api_url;

pub fn get_api_url() -> &'static str {
    api_url()
}

pub fn get_github_token() -> Option<String> {
    Command::new("gh")
        .args(&["auth", "token"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn api_get(endpoint: &str, token: &str) -> Option<String> {
    let url = format!("{}{}", get_api_url(), endpoint);
    Command::new("curl")
        .args(&["-sSL", "--fail", "-H", &format!("Authorization: Bearer {}", token), &url])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

pub fn api_post(endpoint: &str, token: &str, body: &str) -> Option<String> {
    let url = format!("{}{}", get_api_url(), endpoint);
    Command::new("curl")
        .args(&["-sSL", "-X", "POST", "--fail",
            "-H", &format!("Authorization: Bearer {}", token),
            "-H", "Content-Type: application/json",
            "-d", body,
            &url])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}
