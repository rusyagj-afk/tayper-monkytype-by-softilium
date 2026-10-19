use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Deserialize, Debug, Clone)]
pub struct ChromeTab {
    #[serde(default)]
    pub title: String,
    pub url: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Browser {
    pub name: String,
    pub path: String,
    pub process_name: String,
}

pub fn detect_browsers() -> Vec<Browser> {
    let mut list = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let check = |name: &str, path: &str, proc: &str| -> Option<Browser> {
            if Path::new(path).exists() {
                Some(Browser {
                    name: name.to_string(),
                    path: path.to_string(),
                    process_name: proc.to_string(),
                })
            } else {
                None
            }
        };

        if let Some(b) = check(
            "Google Chrome",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            "chrome.exe",
        ) {
            list.push(b);
        }
        if let Some(b) = check(
            "Microsoft Edge",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            "msedge.exe",
        ) {
            list.push(b);
        }
        if let Some(b) = check(
            "Brave",
            r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
            "brave.exe",
        ) {
            list.push(b);
        }
        if let Some(b) = check(
            "Yandex Browser",
            r"C:\Program Files (x86)\Yandex\YandexBrowser\Application\browser.exe",
            "browser.exe",
        ) {
            list.push(b);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let check = |name: &str, path: &str, proc: &str| -> Option<Browser> {
            if Path::new(path).exists() {
                Some(Browser {
                    name: name.to_string(),
                    path: path.to_string(),
                    process_name: proc.to_string(),
                })
            } else {
                None
            }
        };

        if let Some(b) = check("Helium", "/opt/helium/helium", "helium") {
            list.push(b);
        }
        if let Some(b) = check("Google Chrome", "/usr/bin/google-chrome-stable", "chrome") {
            list.push(b);
        }
        if let Some(b) = check("Google Chrome", "/usr/bin/google-chrome", "chrome") {
            list.push(b);
        }
        if let Some(b) = check("Chromium", "/usr/bin/chromium", "chromium") {
            list.push(b);
        }
        if let Some(b) = check("Brave", "/usr/bin/brave-browser", "brave") {
            list.push(b);
        }
        if let Some(b) = check(
            "Yandex Browser",
            "/usr/bin/yandex-browser",
            "yandex-browser",
        ) {
            list.push(b);
        }
    }

    list
}

pub fn is_running(process_pattern: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("tasklist")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
            .unwrap_or_default();
        output.contains(&process_pattern.to_lowercase())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("pgrep")
            .arg("-f")
            .arg(process_pattern)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

pub fn kill(process_pattern: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", process_pattern])
            .output();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("pkill").args(["-f", process_pattern]).output();
    }
}

pub async fn ping_cdp(cdp_host: &str) -> bool {
    let url = format!("http://{cdp_host}/json");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(600))
        .build();

    match client {
        Ok(c) => c.get(&url).send().await.is_ok(),
        Err(_) => false,
    }
}

pub async fn launch_instance(
    browser: &Browser,
    cdp_host: &str,
    isolate_profile: bool,
) -> Result<(), String> {
    let mut cmd = Command::new(&browser.path);
    cmd.arg("--remote-debugging-port=9222");

    if isolate_profile {
        let profile_dir = std::env::temp_dir().join("softilium-profile");
        cmd.arg(format!("--user-data-dir={}", profile_dir.display()));
    }

    cmd.arg("https://monkeytype.com");

    #[cfg(not(target_os = "windows"))]
    {
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }

    cmd.spawn()
        .map_err(|e| format!("Failed to spawn {}: {e}", browser.path))?;

    let url = format!("http://{cdp_host}/json");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(tabs) = resp.json::<Vec<ChromeTab>>().await {
                if tabs.iter().any(|t| t.url.contains("monkeytype.com")) {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    return Ok(());
                }
            }
        }
    }

    Err("Browser failed to expose port 9222 within 20s timeout".to_string())
}

pub async fn fetch_active_tab(cdp_host: &str) -> Result<ChromeTab, String> {
    let url = format!("http://{cdp_host}/json");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| format!("HTTP client init failed: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Unable to reach CDP endpoint {url}: {e}"))?;

    let tabs: Vec<ChromeTab> = resp.json().await.map_err(|e| e.to_string())?;

    tabs.into_iter()
        .find(|t| t.url.contains("monkeytype.com"))
        .ok_or_else(|| "No active monkeytype.com tab found".to_string())
}
