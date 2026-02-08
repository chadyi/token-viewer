// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{DateTime, TimeZone, Utc};
use glob::glob;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use std::collections::HashMap;
use once_cell::sync::Lazy;

struct PricingInfo {
  input_cost_per_token: f64,
  output_cost_per_token: f64,
  cache_read_cost: f64,
  cache_write_cost: f64,
}

fn load_pricing() -> HashMap<String, PricingInfo> {
  let url = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
  let resp = match reqwest::blocking::get(url) {
    Ok(r) => r,
    Err(e) => {
      log::warn!("Failed to fetch LiteLLM pricing: {e}");
      return HashMap::new();
    }
  };
  let json: Value = match resp.json() {
    Ok(v) => v,
    Err(e) => {
      log::warn!("Failed to parse LiteLLM pricing: {e}");
      return HashMap::new();
    }
  };
  let obj = match json.as_object() {
    Some(o) => o,
    None => return HashMap::new(),
  };
  let mut map = HashMap::new();
  for (key, val) in obj {
    let input = val
      .get("input_cost_per_token")
      .and_then(|v| v.as_f64())
      .unwrap_or(0.0);
    let output = val
      .get("output_cost_per_token")
      .and_then(|v| v.as_f64())
      .unwrap_or(0.0);
    if input == 0.0 && output == 0.0 {
      continue;
    }
    let cache_read = val
      .get("cache_read_input_token_cost")
      .and_then(|v| v.as_f64())
      .unwrap_or(0.0);
    let cache_write = val
      .get("cache_creation_input_token_cost")
      .and_then(|v| v.as_f64())
      .unwrap_or(0.0);
    map.insert(
      key.clone(),
      PricingInfo {
        input_cost_per_token: input,
        output_cost_per_token: output,
        cache_read_cost: cache_read,
        cache_write_cost: cache_write,
      },
    );
  }
  map
}

static PRICING: Lazy<HashMap<String, PricingInfo>> = Lazy::new(load_pricing);

fn find_pricing(model: &str) -> Option<&'static PricingInfo> {
  // exact match
  if let Some(p) = PRICING.get(model) {
    return Some(p);
  }
  // with provider prefix
  for prefix in ["anthropic/", "openai/", "azure/", "google/", "vertex_ai/"] {
    let key = format!("{prefix}{model}");
    if let Some(p) = PRICING.get(&key) {
      return Some(p);
    }
  }
  // fuzzy: model contains key or key contains model
  for (key, p) in PRICING.iter() {
    if key.contains(model) || model.contains(key.as_str()) {
      return Some(p);
    }
  }
  None
}

fn estimate_cost(model: &str, input: u64, output: u64, cache_read: u64, cache_write: u64) -> f64 {
  let Some(p) = find_pricing(model) else {
    return 0.0;
  };
  let base_input = if input > cache_read { input - cache_read } else { 0 };
  base_input as f64 * p.input_cost_per_token
    + output as f64 * p.output_cost_per_token
    + cache_read as f64 * p.cache_read_cost
    + cache_write as f64 * p.cache_write_cost
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageEntry {
  pub timestamp: String,
  pub tool: String,
  pub model: String,
  pub input_tokens: u64,
  pub output_tokens: u64,
  pub cache_read_tokens: u64,
  pub cache_write_tokens: u64,
  pub cost: f64,
}

fn home_glob_prefix() -> Option<String> {
  let home = dirs::home_dir()?;
  Some(home.to_string_lossy().replace('\\', "/"))
}

fn glob_paths(patterns: &[String]) -> Vec<PathBuf> {
  let mut out = Vec::new();
  let mut seen = HashSet::<String>::new();

  for pattern in patterns {
    let entries = match glob(pattern) {
      Ok(it) => it,
      Err(err) => {
        log::debug!("Invalid glob pattern '{pattern}': {err}");
        continue;
      }
    };

    for entry in entries {
      match entry {
        Ok(path) => {
          let key = path.to_string_lossy().to_string();
          if seen.insert(key) {
            out.push(path);
          }
        }
        Err(err) => {
          log::debug!("Glob error for pattern '{pattern}': {err}");
        }
      }
    }
  }

  out
}

fn file_mtime_rfc3339(path: &Path) -> Option<String> {
  let st = fs::metadata(path).ok()?.modified().ok()?;
  let dt: DateTime<Utc> = st.into();
  Some(dt.to_rfc3339())
}

fn normalize_epoch(epoch: i64) -> Option<String> {
  let dt = if epoch.unsigned_abs() >= 1_000_000_000_000 {
    Utc.timestamp_millis_opt(epoch).single()?
  } else {
    Utc.timestamp_opt(epoch, 0).single()?
  };
  Some(dt.to_rfc3339())
}

fn normalize_timestamp(value: Option<&Value>) -> Option<String> {
  match value? {
    Value::String(s) => {
      let s = s.trim();
      if s.is_empty() {
        return None;
      }
      if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc).to_rfc3339());
      }
      if s.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(epoch) = s.parse::<i64>() {
          return normalize_epoch(epoch);
        }
      }
      Some(s.to_string())
    }
    Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        normalize_epoch(i)
      } else if let Some(u) = n.as_u64() {
        if u <= i64::MAX as u64 {
          normalize_epoch(u as i64)
        } else {
          None
        }
      } else {
        None
      }
    }
    _ => None,
  }
}

fn value_u64(value: Option<&Value>) -> u64 {
  match value {
    Some(Value::Number(n)) => n.as_u64().unwrap_or_else(|| n.as_i64().unwrap_or(0).max(0) as u64),
    Some(Value::String(s)) => s.trim().parse::<u64>().unwrap_or(0),
    _ => 0,
  }
}

fn value_f64(value: Option<&Value>) -> f64 {
  match value {
    Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
    Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
    _ => 0.0,
  }
}

fn scan_claude_usage_impl() -> Vec<UsageEntry> {
  let Some(home) = home_glob_prefix() else {
    return Vec::new();
  };

  let patterns = vec![
    format!("{home}/.config/claude/projects/**/*.jsonl"),
    format!("{home}/.claude/projects/**/*.jsonl"),
  ];

  let files = glob_paths(&patterns);
  let mut out = Vec::new();

  for path in files {
    let fallback_ts = file_mtime_rfc3339(&path).unwrap_or_default();
    let file = match File::open(&path) {
      Ok(f) => f,
      Err(_) => continue,
    };

    let reader = BufReader::new(file);
    for line in reader.lines().flatten() {
      let line = line.trim();
      if line.is_empty() {
        continue;
      }

      let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => continue, // skip parse-failed lines
      };

      let usage = v.get("message").and_then(|m| m.get("usage"));
      let input_tokens = value_u64(usage.and_then(|u| u.get("input_tokens")));
      let output_tokens = value_u64(usage.and_then(|u| u.get("output_tokens")));
      let cache_write_tokens = value_u64(usage.and_then(|u| u.get("cache_creation_input_tokens")));
      let cache_read_tokens = value_u64(usage.and_then(|u| u.get("cache_read_input_tokens")));
      let cost = value_f64(v.get("costUSD"));

      if input_tokens == 0
        && output_tokens == 0
        && cache_write_tokens == 0
        && cache_read_tokens == 0
        && cost == 0.0
      {
        continue;
      }

      let timestamp =
        normalize_timestamp(v.get("timestamp")).unwrap_or_else(|| fallback_ts.clone());
      let model = v
        .get("message")
        .and_then(|m| m.get("model"))
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string();

      let cost = if cost == 0.0 && model != "unknown" { estimate_cost(&model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens) } else { cost };

      out.push(UsageEntry {
        timestamp,
        tool: "Claude".to_string(),
        model,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost,
      });
    }
  }

  out
}

fn scan_codex_usage_impl() -> Vec<UsageEntry> {
  let Some(home) = home_glob_prefix() else {
    return Vec::new();
  };

  let patterns = vec![format!("{home}/.codex/sessions/**/*.jsonl")];
  let files = glob_paths(&patterns);
  let mut out = Vec::new();

  for path in files {
    let fallback_ts = file_mtime_rfc3339(&path).unwrap_or_default();
    let file = match File::open(&path) {
      Ok(f) => f,
      Err(_) => continue,
    };

    let reader = BufReader::new(file);
    for line in reader.lines().flatten() {
      let line = line.trim();
      if line.is_empty() {
        continue;
      }

      let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => continue, // skip parse-failed lines
      };

      let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
      if ty != "event_msg" {
        continue;
      }

      let payload_type = v.pointer("/payload/type").and_then(|t| t.as_str()).unwrap_or("");
      if payload_type != "token_count" {
        continue;
      }

      let last = match v.pointer("/payload/info/last_token_usage") {
        Some(x) => x,
        None => continue,
      };

      let input_tokens = value_u64(last.get("input_tokens"));
      let output_tokens = value_u64(last.get("output_tokens"));
      let cache_read_tokens = value_u64(last.get("cached_input_tokens"));
      let cache_write_tokens = 0;

      let model = v
        .pointer("/payload/info/model")
        .and_then(|m| m.as_str())
        .or_else(|| v.pointer("/payload/info/model_name").and_then(|m| m.as_str()))
        .unwrap_or("unknown")
        .to_string();

      let ts_val = v
        .get("timestamp")
        .or_else(|| v.get("time"))
        .or_else(|| v.get("created_at"))
        .or_else(|| v.pointer("/payload/info/time"))
        .or_else(|| v.pointer("/payload/time"));
      let timestamp = normalize_timestamp(ts_val).unwrap_or_else(|| fallback_ts.clone());

      out.push(UsageEntry {
        timestamp,
        tool: "Codex".to_string(),
        cost: estimate_cost(&model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens),
        model,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
      });
    }
  }

  out
}

fn scan_opencode_usage_impl() -> Vec<UsageEntry> {
  let Some(home) = home_glob_prefix() else {
    return Vec::new();
  };

  let patterns = vec![format!(
    "{home}/.local/share/opencode/storage/message/**/*.json"
  )];
  let files = glob_paths(&patterns);
  let mut out = Vec::new();

  for path in files {
    let fallback_ts = file_mtime_rfc3339(&path).unwrap_or_default();
    let raw = match fs::read_to_string(&path) {
      Ok(s) => s,
      Err(_) => continue,
    };

    let v: Value = match serde_json::from_str(&raw) {
      Ok(v) => v,
      Err(_) => continue,
    };

    let input_tokens = value_u64(v.pointer("/tokens/input"));
    let output_tokens = value_u64(v.pointer("/tokens/output"));
    let cache_read_tokens = value_u64(v.pointer("/tokens/cache/read"));
    let cache_write_tokens = value_u64(v.pointer("/tokens/cache/write"));
    let cost = value_f64(v.get("cost"));

    if input_tokens == 0
      && output_tokens == 0
      && cache_write_tokens == 0
      && cache_read_tokens == 0
      && cost == 0.0
    {
      continue;
    }

    let timestamp = normalize_timestamp(v.pointer("/time/created")).unwrap_or(fallback_ts);
    let model = v
      .get("modelID")
      .and_then(|m| m.as_str())
      .unwrap_or("unknown")
      .to_string();

    let cost = if cost == 0.0 && model != "unknown" { estimate_cost(&model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens) } else { cost };

    out.push(UsageEntry {
      timestamp,
      tool: "OpenCode".to_string(),
      model,
      input_tokens,
      output_tokens,
      cache_read_tokens,
      cache_write_tokens,
      cost,
    });
  }

  out
}

#[tauri::command]
async fn scan_claude_usage() -> Vec<UsageEntry> {
  tauri::async_runtime::spawn_blocking(scan_claude_usage_impl)
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn scan_codex_usage() -> Vec<UsageEntry> {
  tauri::async_runtime::spawn_blocking(scan_codex_usage_impl)
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn scan_opencode_usage() -> Vec<UsageEntry> {
  tauri::async_runtime::spawn_blocking(scan_opencode_usage_impl)
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn scan_all_usage() -> Vec<UsageEntry> {
  let claude = tauri::async_runtime::spawn_blocking(scan_claude_usage_impl);
  let codex = tauri::async_runtime::spawn_blocking(scan_codex_usage_impl);
  let opencode = tauri::async_runtime::spawn_blocking(scan_opencode_usage_impl);

  let mut out = Vec::new();
  out.extend(claude.await.unwrap_or_default());
  out.extend(codex.await.unwrap_or_default());
  out.extend(opencode.await.unwrap_or_default());
  out
}

fn main() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      scan_claude_usage,
      scan_codex_usage,
      scan_opencode_usage,
      scan_all_usage
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
