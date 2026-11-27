use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct SearchSources {
  pub wikimedia: bool, pub openverse: bool, pub web_search: bool, pub safe_search: bool,
  pub custom_urls: Vec<String>, pub local_db_dir: String, pub auto_tag: bool,
  pub brave_api_key: String, pub pexels_api_key: String,
  pub default_limit: usize, pub default_aspect: String, pub default_img_type: String, pub license_free: bool
}
impl Default for SearchSources {
  fn default() -> Self {
    Self {
      wikimedia: true, openverse: true, web_search: true, safe_search: false,
      custom_urls: vec![], local_db_dir: String::new(), auto_tag: true,
      brave_api_key: String::new(), pexels_api_key: String::new(),
      default_limit: 24, default_aspect: "all".into(), default_img_type: "all".into(), license_free: false
    }
  }
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig { pub setup_done: bool, pub output_dir: String, pub models_dir: String, pub backend_choice: String, pub sources: SearchSources }
impl Default for AppConfig { fn default() -> Self { Self { setup_done: false, output_dir: String::new(), models_dir: String::new(), backend_choice: "auto".into(), sources: SearchSources::default() } } }
pub fn app_data_dir() -> PathBuf { let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")); let d = base.join("BytebeatFlux").join("LocalImageGen"); let _ = std::fs::create_dir_all(&d); d }
pub fn config_path() -> PathBuf { app_data_dir().join("config.json") }
pub fn load() -> AppConfig { std::fs::read_to_string(config_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default() }
pub fn save(cfg: &AppConfig) -> Result<(), String> { let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?; std::fs::write(config_path(), s).map_err(|e| e.to_string()) }
