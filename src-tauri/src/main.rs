// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{DateTime, TimeZone, Utc};
use glob::glob;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use std::collections::HashMap;
use once_cell::sync::Lazy;

struct PricingInfo {
  input_cost_per_token: f64,
  output_cost_per_token: f64,
  cache_read_cost: f64,
  cache_write_cost: f64,
  // Tiered pricing for 200k+ tokens (Claude models)
  input_cost_above_200k: f64,
  output_cost_above_200k: f64,
  cache_read_cost_above_200k: f64,
  cache_write_cost_above_200k: f64,
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
    let input_above_200k = val
      .get("input_cost_per_token_above_200k_tokens")
      .and_then(|v| v.as_f64())
      .unwrap_or(0.0);
    let output_above_200k = val
      .get("output_cost_per_token_above_200k_tokens")
      .and_then(|v| v.as_f64())
      .unwrap_or(0.0);
    let cache_read_above_200k = val
      .get("cache_read_input_token_cost_above_200k_tokens")
      .and_then(|v| v.as_f64())
      .unwrap_or(0.0);
    let cache_write_above_200k = val
      .get("cache_creation_input_token_cost_above_200k_tokens")
      .and_then(|v| v.as_f64())
      .unwrap_or(0.0);
    map.insert(
      key.clone(),
      PricingInfo {
        input_cost_per_token: input,
        output_cost_per_token: output,
        cache_read_cost: cache_read,
        cache_write_cost: cache_write,
        input_cost_above_200k: input_above_200k,
        output_cost_above_200k: output_above_200k,
        cache_read_cost_above_200k: cache_read_above_200k,
        cache_write_cost_above_200k: cache_write_above_200k,
      },
    );
  }
  map
}

static PRICING: Lazy<HashMap<String, PricingInfo>> = Lazy::new(load_pricing);

fn find_pricing(model: &str) -> Option<&'static PricingInfo> {
  fn try_find(name: &str) -> Option<&'static PricingInfo> {
    // exact match
    if let Some(p) = PRICING.get(name) {
      return Some(p);
    }
    // with provider prefix
    for prefix in ["anthropic/", "openai/", "azure/", "google/", "vertex_ai/", "gemini/"] {
      let key = format!("{prefix}{name}");
      if let Some(p) = PRICING.get(&key) {
        return Some(p);
      }
    }
    // fuzzy: bidirectional includes (matching original ccusage logic)
    let lower = name.to_lowercase();
    for (key, p) in PRICING.iter() {
      let key_lower = key.to_lowercase();
      if key_lower.contains(&lower) || lower.contains(&key_lower) {
        return Some(p);
      }
    }
    None
  }

  fn strip_date_suffix(name: &str) -> Option<&str> {
    let (base, suffix) = name.rsplit_once('-')?;
    if suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_digit()) {
      Some(base)
    } else {
      None
    }
  }

  // Try original name first.
  if let Some(p) = try_find(model) {
    return Some(p);
  }

  // Normalize "-thinking" suffix and retry.
  if let Some(base_model) = model.strip_suffix("-thinking") {
    if let Some(p) = try_find(base_model) {
      return Some(p);
    }
    // If still not found, also try stripping a trailing date version (e.g. "-20250918").
    if let Some(no_date) = strip_date_suffix(base_model) {
      if let Some(p) = try_find(no_date) {
        return Some(p);
      }
    }
  }

  // Handle variants where "-thinking" appears before the date: "...-thinking-20250918".
  if let Some(no_date) = strip_date_suffix(model) {
    if let Some(p) = try_find(no_date) {
      return Some(p);
    }
    if let Some(no_date_no_thinking) = no_date.strip_suffix("-thinking") {
      if let Some(p) = try_find(no_date_no_thinking) {
        return Some(p);
      }
    }
  }

  // Strip quality suffixes like "-high", "-low", "-medium" (e.g. gemini-3-pro-high → gemini-3-pro)
  for suffix in ["-high", "-low", "-medium"] {
    if let Some(base) = model.strip_suffix(suffix) {
      if let Some(p) = try_find(base) {
        return Some(p);
      }
    }
  }

  None
}

const TIERED_THRESHOLD: u64 = 200_000;

fn tiered_cost(tokens: u64, base_price: f64, above_price: f64) -> f64 {
  if tokens == 0 {
    return 0.0;
  }
  if above_price > 0.0 && tokens > TIERED_THRESHOLD {
    let below = TIERED_THRESHOLD as f64 * base_price;
    let above = (tokens - TIERED_THRESHOLD) as f64 * above_price;
    below + above
  } else {
    tokens as f64 * base_price
  }
}

fn estimate_cost(model: &str, input: u64, output: u64, cache_read: u64, cache_write: u64) -> f64 {
  let Some(p) = find_pricing(model) else {
    return 0.0;
  };
  // Match original ccusage: input_tokens * price (NO subtraction of cache_read)
  let input_cost = tiered_cost(input, p.input_cost_per_token, p.input_cost_above_200k);
  let output_cost = tiered_cost(output, p.output_cost_per_token, p.output_cost_above_200k);
  let cache_read_cost = tiered_cost(cache_read, p.cache_read_cost, p.cache_read_cost_above_200k);
  let cache_write_cost = tiered_cost(cache_write, p.cache_write_cost, p.cache_write_cost_above_200k);
  input_cost + output_cost + cache_read_cost + cache_write_cost
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

struct ScanState {
  file_offsets: HashMap<String, u64>,
  codex_file_models: HashMap<String, String>,
  cached_entries: Vec<UsageEntry>,
}

static SCAN_STATE: Lazy<Mutex<ScanState>> = Lazy::new(|| {
  Mutex::new(ScanState {
    file_offsets: HashMap::new(),
    codex_file_models: HashMap::new(),
    cached_entries: Vec::new(),
  })
});

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
  scan_claude_incremental(&mut HashMap::new())
}

fn scan_claude_incremental(offsets: &mut HashMap<String, u64>) -> Vec<UsageEntry> {
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
    let key = path.to_string_lossy().to_string();
    let file_len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let prev_offset = offsets.get(&key).copied().unwrap_or(0);

    // Skip if no new content; reset if file was truncated
    let start_offset = if file_len <= prev_offset && prev_offset > 0 {
      if file_len == prev_offset { continue; }
      0 // file was truncated, re-read
    } else {
      prev_offset
    };

    let fallback_ts = file_mtime_rfc3339(&path).unwrap_or_default();
    let mut file = match File::open(&path) {
      Ok(f) => f,
      Err(_) => continue,
    };

    if start_offset > 0 {
      if file.seek(SeekFrom::Start(start_offset)).is_err() {
        continue;
      }
    }

    let reader = BufReader::new(&mut file);
    for line in reader.lines().flatten() {
      let line = line.trim();
      if line.is_empty() {
        continue;
      }

      let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => continue,
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
      let model = {
        let from_message = v
          .get("message")
          .and_then(|m| m.get("model"))
          .and_then(|m| m.as_str())
          .unwrap_or("unknown");
        if from_message != "unknown" {
          from_message.to_string()
        } else {
          v.get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown")
            .to_string()
        }
      };

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

    // Update offset to current file position
    let new_offset = file.stream_position().unwrap_or(file_len);
    offsets.insert(key, new_offset);
  }

  out
}

fn extract_codex_model(v: &Value) -> Option<String> {
  // Try payload.info.model, payload.info.model_name
  for ptr in ["/payload/info/model", "/payload/info/model_name"] {
    if let Some(s) = v.pointer(ptr).and_then(|m| m.as_str()) {
      let s = s.trim();
      if !s.is_empty() { return Some(s.to_string()); }
    }
  }
  // Try payload.info.metadata.model
  if let Some(s) = v.pointer("/payload/info/metadata/model").and_then(|m| m.as_str()) {
    let s = s.trim();
    if !s.is_empty() { return Some(s.to_string()); }
  }
  // Try payload.model
  if let Some(s) = v.pointer("/payload/model").and_then(|m| m.as_str()) {
    let s = s.trim();
    if !s.is_empty() { return Some(s.to_string()); }
  }
  // Try payload.metadata.model
  if let Some(s) = v.pointer("/payload/metadata/model").and_then(|m| m.as_str()) {
    let s = s.trim();
    if !s.is_empty() { return Some(s.to_string()); }
  }
  None
}

fn scan_codex_usage_impl() -> Vec<UsageEntry> {
  scan_codex_incremental(&mut HashMap::new(), &mut HashMap::new())
}

fn scan_codex_incremental(offsets: &mut HashMap<String, u64>, file_models: &mut HashMap<String, String>) -> Vec<UsageEntry> {
  let Some(home) = home_glob_prefix() else {
    return Vec::new();
  };

  let patterns = vec![format!("{home}/.codex/sessions/**/*.jsonl")];
  let files = glob_paths(&patterns);
  let mut out = Vec::new();

  for path in files {
    let key = path.to_string_lossy().to_string();
    let file_len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let prev_offset = offsets.get(&key).copied().unwrap_or(0);

    let start_offset = if file_len <= prev_offset && prev_offset > 0 {
      if file_len == prev_offset { continue; }
      0
    } else {
      prev_offset
    };

    let fallback_ts = file_mtime_rfc3339(&path).unwrap_or_default();
    let mut file = match File::open(&path) {
      Ok(f) => f,
      Err(_) => continue,
    };

    if start_offset > 0 {
      if file.seek(SeekFrom::Start(start_offset)).is_err() {
        continue;
      }
    }

    // Restore last known model for this file (for incremental reads)
    let mut current_model: Option<String> = file_models.get(&key).cloned();
    let mut prev_total: Option<(u64, u64, u64)> = None;

    let reader = BufReader::new(&mut file);
    for line in reader.lines().flatten() {
      let line = line.trim();
      if line.is_empty() {
        continue;
      }

      let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => continue,
      };

      let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

      if ty == "turn_context" {
        if let Some(m) = extract_codex_model(&v) {
          current_model = Some(m);
        } else if let Some(s) = v.pointer("/payload/model").and_then(|m| m.as_str()) {
          let s = s.trim();
          if !s.is_empty() { current_model = Some(s.to_string()); }
        }
        continue;
      }

      if ty != "event_msg" {
        continue;
      }

      let payload_type = v.pointer("/payload/type").and_then(|t| t.as_str()).unwrap_or("");
      if payload_type != "token_count" {
        continue;
      }

      let (input_tokens, output_tokens, cache_read_tokens) =
        if let Some(last) = v.pointer("/payload/info/last_token_usage") {
          (
            value_u64(last.get("input_tokens")),
            value_u64(last.get("output_tokens")),
            value_u64(last.get("cached_input_tokens").or(last.get("cache_read_input_tokens"))),
          )
        } else if let Some(total) = v.pointer("/payload/info/total_token_usage") {
          let cur_in = value_u64(total.get("input_tokens"));
          let cur_out = value_u64(total.get("output_tokens"));
          let cur_cached = value_u64(total.get("cached_input_tokens").or(total.get("cache_read_input_tokens")));
          let delta = if let Some((pi, po, pc)) = prev_total {
            (cur_in.saturating_sub(pi), cur_out.saturating_sub(po), cur_cached.saturating_sub(pc))
          } else {
            (cur_in, cur_out, cur_cached)
          };
          prev_total = Some((cur_in, cur_out, cur_cached));
          delta
        } else {
          continue;
        };

      if input_tokens == 0 && output_tokens == 0 && cache_read_tokens == 0 {
        continue;
      }

      let model = extract_codex_model(&v)
        .or_else(|| current_model.clone())
        .unwrap_or_else(|| "gpt-5".to_string());

      if let Some(m) = extract_codex_model(&v) {
        current_model = Some(m);
      }

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
        cost: estimate_cost(&model, input_tokens, output_tokens, cache_read_tokens, 0),
        model,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens: 0,
      });
    }

    let new_offset = file.stream_position().unwrap_or(file_len);
    offsets.insert(key.clone(), new_offset);
    if let Some(m) = current_model {
      file_models.insert(key, m);
    }
  }

  out
}

fn scan_opencode_usage_impl() -> Vec<UsageEntry> {
  scan_opencode_incremental(&mut HashMap::new())
}

fn scan_opencode_incremental(seen_files: &mut HashMap<String, u64>) -> Vec<UsageEntry> {
  let Some(home) = home_glob_prefix() else {
    return Vec::new();
  };

  let patterns = vec![format!(
    "{home}/.local/share/opencode/storage/message/**/*.json"
  )];
  let files = glob_paths(&patterns);
  let mut out = Vec::new();

  for path in files {
    let key = path.to_string_lossy().to_string();
    let file_len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

    // For JSON files (not JSONL), skip if already processed and same size
    if let Some(&prev_len) = seen_files.get(&key) {
      if file_len == prev_len { continue; }
    }

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
      seen_files.insert(key, file_len);
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

    seen_files.insert(key, file_len);
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
  let entries = tauri::async_runtime::spawn_blocking(|| {
    let claude = scan_claude_usage_impl();
    let codex = scan_codex_usage_impl();
    let opencode = scan_opencode_usage_impl();
    let mut out = Vec::new();
    out.extend(claude);
    out.extend(codex);
    out.extend(opencode);
    out
  }).await.unwrap_or_default();

  // Store full results and reset offsets for future incremental scans
  if let Ok(mut state) = SCAN_STATE.lock() {
    state.file_offsets.clear();
    state.codex_file_models.clear();
    state.cached_entries = entries.clone();
  }
  entries
}

#[tauri::command]
async fn scan_all_usage_incremental() -> Vec<UsageEntry> {
  tauri::async_runtime::spawn_blocking(|| {
    let mut state = match SCAN_STATE.lock() {
      Ok(s) => s,
      Err(_) => return Vec::new(),
    };

    let ScanState { file_offsets, codex_file_models, cached_entries } = &mut *state;

    let claude_new = scan_claude_incremental(file_offsets);
    let codex_new = scan_codex_incremental(file_offsets, codex_file_models);
    let opencode_new = scan_opencode_incremental(file_offsets);

    cached_entries.extend(claude_new);
    cached_entries.extend(codex_new);
    cached_entries.extend(opencode_new);

    cached_entries.clone()
  }).await.unwrap_or_default()
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
      scan_all_usage,
      scan_all_usage_incremental
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
