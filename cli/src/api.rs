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

fn test_player_header() -> Option<String> {
    std::env::var("TEST_PLAYER").ok().filter(|s| !s.is_empty())
}

pub fn api_get(endpoint: &str, token: &str) -> Option<String> {
    let url = format!("{}{}", get_api_url(), endpoint);
    let mut cmd = Command::new("curl");
    cmd.args(&["-sSL", "--fail", "-H", &format!("Authorization: Bearer {}", token)]);
    if let Some(player) = test_player_header() {
        cmd.args(&["-H", &format!("X-Test-Player: {}", player)]);
    }
    cmd.arg(&url);
    cmd.output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

pub fn api_post(endpoint: &str, token: &str, body: &str) -> Option<String> {
    let url = format!("{}{}", get_api_url(), endpoint);
    let mut cmd = Command::new("curl");
    cmd.args(&["-sSL", "-X", "POST", "--fail",
        "-H", &format!("Authorization: Bearer {}", token),
        "-H", "Content-Type: application/json",
        "-d", body]);
    if let Some(player) = test_player_header() {
        cmd.args(&["-H", &format!("X-Test-Player: {}", player)]);
    }
    cmd.arg(&url);
    cmd.output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}
