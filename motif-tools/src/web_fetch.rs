//! HTTP GET with content extraction: HTML → text, JSON → pretty-printed.

use motif::RegisteredTool;
use motif::ToolDef;

const MAX_CONTENT_BYTES: usize = 1_048_576;
const TIMEOUT_MS: u64 = 15_000;
const MAX_REDIRECTS: usize = 5;

pub fn register() -> RegisteredTool {
    ToolDef::new(
        "web_fetch",
        "Fetch a URL and return its content. HTML pages are converted to plain text.",
    )
    .param::<String>("url", "URL to fetch (http or https)")
    .param::<u64>("timeout_ms", "Timeout in milliseconds (default 15000)")
    .build(web_fetch_impl)
}

fn web_fetch_impl(
    args: String,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    Box::pin(async move {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        let url = v["url"].as_str().unwrap_or("").to_string();
        if url.is_empty() {
            return "Error: 'url' is required".to_string();
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return "Error: only http and https URLs are supported".to_string();
        }
        let timeout_ms = v["timeout_ms"].as_u64().unwrap_or(TIMEOUT_MS);

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .user_agent("motif/0.2")
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()
        {
            Ok(c) => c,
            Err(e) => return format!("Failed to build HTTP client: {}", e),
        };

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return format!("Request failed: {}", e),
        };
        let status = response.status();
        // Security: check for cross-domain redirects
        let final_url = response.url().to_string();
        if let (Ok(orig), Ok(final_url_parsed)) =
            (reqwest::Url::parse(&url), reqwest::Url::parse(&final_url))
        {
            if final_url_parsed.host_str() != orig.host_str() {
                return format!("Cross-domain redirect blocked: {} → {}", url, final_url);
            }
        }
        // SSRF: block resolves to private IP (nanobot pattern)
        if let Some(addr) = response.remote_addr() {
            if is_private_ip(&addr.ip()) {
                return format!("SSRF blocked: resolved to private IP {}", addr.ip());
            }
        }
        if !status.is_success() {
            return format!(
                "HTTP {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("")
            );
        }
        let bytes = match response.bytes().await {
            Ok(b) => b,
            Err(e) => return format!("Read error: {}", e),
        };
        if bytes.len() > MAX_CONTENT_BYTES {
            return format!(
                "Content too large ({} bytes, limit {})",
                bytes.len(),
                MAX_CONTENT_BYTES
            );
        }

        content_type_dispatch(&bytes)
    })
}

fn content_type_dispatch(bytes: &[u8]) -> String {
    let ct = if let Ok(s) = std::str::from_utf8(&bytes[..bytes.len().min(512)]) {
        if s.trim_start().starts_with('{') || s.trim_start().starts_with('[') {
            "json"
        } else if s.trim_start().starts_with("<!") || s.trim_start().starts_with("<htm") {
            "html"
        } else {
            "text"
        }
    } else {
        "text"
    };

    let text = match ct {
        "json" => serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or_else(|| String::from_utf8_lossy(bytes).to_string()),
        "html" => html_to_text(&String::from_utf8_lossy(bytes)),
        _ => String::from_utf8_lossy(bytes).to_string(),
    };

    if text.len() > MAX_CONTENT_BYTES {
        format!("{}...(truncated)", &text[..MAX_CONTENT_BYTES])
    } else {
        text
    }
}

fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        std::net::IpAddr::V6(v6) => v6.is_loopback(),
    }
}

fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut in_skip = false;
    let mut tag = String::new();
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            tag.clear();
            continue;
        }
        if ch == '>' {
            in_tag = false;
            let t = tag.trim().to_lowercase();
            if t == "script" || t == "style" {
                in_skip = true;
                continue;
            }
            if t == "/script" || t == "/style" {
                in_skip = false;
                continue;
            }
            if t == "br"
                || t.starts_with("br ")
                || t == "p"
                || t.starts_with("/p")
                || t == "li"
                || t == "tr"
                || t == "h1"
                || t == "h2"
                || t.starts_with("div")
                || t.starts_with("/div")
            {
                out.push('\n');
            }
            continue;
        }
        if in_tag {
            tag.push(ch);
            continue;
        }
        if in_skip {
            continue;
        }
        if ch.is_ascii_whitespace() && out.ends_with(' ') {
            continue;
        }
        out.push(if ch.is_ascii_whitespace() && ch != '\n' {
            ' '
        } else {
            ch
        });
    }
    let text = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ");
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join("\n")
}
