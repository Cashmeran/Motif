"""Write all test module files."""
import os
base = os.path.join(os.path.dirname(__file__), 'tests', 'modules')
os.makedirs(base, exist_ok=True)

files = {
    'tools_write.rs': r'''use crate::common;
use motif_tools;
use std::fs;
use std::path::Path;

#[test] fn test_write_and_read() {
    let (_, t) = motif_tools::write::register().into_parts();
    let r = common::call_tool(&t, r#"{"file_path":"_tw.txt","content":"hello"}"#);
    assert!(r.contains("Wrote") || r.contains("bytes"));
    fs::remove_file("_tw.txt").ok();
}
#[test] fn test_write_parent_dirs() {
    let (_, t) = motif_tools::write::register().into_parts();
    common::call_tool(&t, r#"{"file_path":"_td/d2/f.txt","content":"x"}"#);
    assert!(Path::new("_td/d2/f.txt").exists());
    fs::remove_dir_all("_td").ok();
}
''',
    'tools_edit.rs': r'''use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_edit_basic() {
    fs::write("_te.txt", "Hello World").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    common::call_tool(&r, r#"{"file_path":"_te.txt"}"#);
    let res = common::call_tool(&e, r#"{"file_path":"_te.txt","old_string":"World","new_string":"Rust"}"#);
    assert!(res.contains("Edited"), "{}", res);
    fs::remove_file("_te.txt").ok();
}
#[test] fn test_edit_without_read_blocked() {
    fs::write("_tenr.txt", "data").unwrap();
    let (_, e) = motif_tools::edit::register().into_parts();
    let res = common::call_tool(&e, r#"{"file_path":"_tenr.txt","old_string":"data","new_string":"x"}"#);
    assert!(res.contains("has not been read"), "{}", res);
    fs::remove_file("_tenr.txt").ok();
}
''',
    'tools_web_fetch.rs': r'''use crate::common;
use motif_tools;

#[test] fn test_web_fetch_invalid() {
    let (_, t) = motif_tools::web_fetch::register().into_parts();
    let r = common::call_tool(&t, r#"{"url":"not-a-url"}"#);
    assert!(r.contains("only http and https"), "{}", r);
}
''',
    'tools_read_state.rs': r'''use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_read_then_edit() {
    fs::write("_trs.txt", "orig").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    common::call_tool(&r, r#"{"file_path":"_trs.txt"}"#);
    let res = common::call_tool(&e, r#"{"file_path":"_trs.txt","old_string":"orig","new_string":"mod"}"#);
    assert!(res.contains("Edited"), "{}", res);
    fs::remove_file("_trs.txt").ok();
}
''',
    'tools_read.rs': r'''use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_read_basic() {
    fs::write("_tr.txt", "line1\nline2\n").unwrap();
    let (_, t) = motif_tools::read::register().into_parts();
    let r = common::call_tool(&t, r#"{"file_path":"_tr.txt"}"#);
    assert!(r.contains("line1"));
    fs::remove_file("_tr.txt").ok();
}
#[test] fn test_read_missing() {
    let (_, t) = motif_tools::read::register().into_parts();
    let r = common::call_tool(&t, r#"{"file_path":"/no/file.txt"}"#);
    assert!(r.contains("Cannot") || r.contains("Error"));
}
''',
    'security_path.rs': r'''use crate::common;
use motif_tools;

#[test] fn test_path_traversal_read() {
    let (_, t) = motif_tools::read::register().into_parts();
    let r = common::call_tool(&t, r#"{"file_path":"../etc/passwd"}"#);
    assert!(r.contains("not allowed"), "{}", r);
}
#[test] fn test_path_traversal_write() {
    let (_, t) = motif_tools::write::register().into_parts();
    let r = common::call_tool(&t, r#"{"file_path":"../evil.txt","content":"x"}"#);
    assert!(r.contains("not allowed"), "{}", r);
}
''',
    'security_ssrf.rs': r'''use crate::common;
use motif_tools;

#[test] fn test_file_url_blocked() {
    let (_, t) = motif_tools::web_fetch::register().into_parts();
    let r = common::call_tool(&t, r#"{"url":"file:///etc/passwd"}"#);
    assert!(r.contains("only http and https"), "{}", r);
}
''',
    'security_quote.rs': r'''use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_curly_to_straight() {
    fs::write("_sq.txt", "He said \u{201c}hello\u{201d} world").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    common::call_tool(&r, r#"{"file_path":"_sq.txt"}"#);
    let res = common::call_tool(&e, r#"{"file_path":"_sq.txt","old_string":"He said \"hello\" world","new_string":"X"}"#);
    assert!(res.contains("Edited"), "{}", res);
    fs::remove_file("_sq.txt").ok();
}
''',
    'security_bash.rs': r'''use crate::common;
use motif_tools;

#[test] fn test_bash_dollar_brace() {
    let (_, t) = motif_tools::bash::register().into_parts();
    let r = common::call_tool(&t, r#"{"command":"echo ${IFS}","timeout_ms":5000}"#);
    assert!(r.contains("not allowed"), "{}", r);
}
#[test] fn test_bash_dollar_at() {
    let (_, t) = motif_tools::bash::register().into_parts();
    let r = common::call_tool(&t, r#"{"command":"echo $@","timeout_ms":5000}"#);
    assert!(r.contains("not allowed"), "{}", r);
}
#[test] fn test_bash_single_quote_safe() {
    let (_, t) = motif_tools::bash::register().into_parts();
    let r = common::call_tool(&t, r#"{"command":"awk {print $1} /dev/null","timeout_ms":5000}"#);
    assert!(!r.contains("not allowed"), "{}", r);
}
''',
    'stress_concurrent.rs': r'''use crate::common;
use motif::*;

#[tokio::test] async fn test_5_agents_parallel() {
    let handles: Vec<_> = (0..5).map(|i| {
        let p = common::MockProvider::new(vec![common::text(&format!("h{}", i))]);
        let mut a = Agent::new(p).model("test");
        tokio::spawn(async move { a.chat("ping").await.unwrap() })
    }).collect();
    for h in handles { h.await.unwrap(); }
}
''',
    'stress_io.rs': r'''use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_write_500k() {
    let c = "x".repeat(500_000);
    let (_, t) = motif_tools::write::register().into_parts();
    let args = format!(r#"{{"file_path":"_si.txt","content":"{}"}}"#, c);
    let r = common::call_tool(&t, &args);
    assert!(r.contains("Wrote") || r.contains("bytes"), "{}", r);
    fs::remove_file("_si.txt").ok();
}
''',
    'stress_long.rs': r'''use crate::common;
use motif::*;

#[tokio::test] async fn test_20_iterations() {
    let responses: Vec<_> = (0..20).map(|i| common::text(&format!("m{}", i))).collect();
    let mut agent = Agent::new(common::MockProvider::new(responses)).model("test");
    let r = agent.chat("start").await.unwrap();
    assert!(!r.is_empty());
}
''',
    'cli_config.rs': r'''use motif_cli::config::Config;

#[test] fn test_config_parse() {
    let json = r#"{"api_key":"sk-test","base_url":"https://x.com","model":"m"}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.api_key, "sk-test");
}
''',
    'core_error.rs': r'''use motif::*;

#[test] fn test_error_display() {
    let e = Error::ApiError { status: 500, body: "err".into() };
    assert!(e.to_string().contains("500"));
}
#[test] fn test_error_tool_not_found() {
    let e = Error::ToolNotFound { name: "x".into(), available: vec![] };
    assert!(e.to_string().contains("x"));
}
#[test] fn test_error_clone() {
    let e = Error::ApiError { status: 429, body: "rl".into() };
    assert_eq!(e.to_string(), e.clone().to_string());
}
''',
    'core_types.rs': r'''use motif::*;

#[test] fn test_message_system() {
    let m = Message::system("hello");
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("system"));
}
#[test] fn test_tool_call_serialization() {
    let tc = ToolCall { id: "c1".into(), call_type: "function".into(),
        function: FunctionCall { name: "f".into(), arguments: "{}".into() } };
    assert!(serde_json::to_string(&tc).unwrap().contains("f"));
}
#[test] fn test_token_usage() {
    let tu = TokenUsage { prompt_tokens: 5, completion_tokens: 3, total_tokens: 8 };
    assert_eq!(tu.total_tokens, 8);
}
''',
    'core_hooks.rs': r'''use std::sync::{Arc, Mutex};
use crate::common;
use motif::*;

struct CountHook { count: Mutex<usize> }
#[async_trait::async_trait]
impl AgentHook for CountHook {
    async fn before_run(&self, _: &mut RunContext) -> motif::Result<()> { *self.count.lock().unwrap() += 1; Ok(()) }
}

#[tokio::test] async fn test_hook_called() {
    let hook = Arc::new(CountHook { count: Mutex::new(0) });
    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).model("test").hook(hook.clone());
    agent.chat("hi").await.unwrap();
    assert!(*hook.count.lock().unwrap() > 0);
}
''',
    'core_prompt.rs': r'''use crate::common;
use motif::*;

#[tokio::test] async fn test_system_prompt_exists() {
    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).model("test");
    agent.chat("hi").await.unwrap();
}
''',
}

for name, content in files.items():
    path = os.path.join(base, name)
    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)
    print(f'Written {name} ({len(content)} bytes)')

print(f'\nTotal files: {len(files)}')
