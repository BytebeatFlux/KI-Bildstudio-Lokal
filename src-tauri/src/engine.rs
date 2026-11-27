use crate::{config, hardware};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{fs, io::Cursor, path::{Path, PathBuf}};
use tauri::{AppHandle, Emitter};
#[derive(Serialize, Clone)] pub struct EngineStatus { pub installed: bool, pub engine_dir: String, pub backend: String }
#[derive(Serialize, Clone)] pub struct ProgressEvt { pub target: String, pub file: String, pub percent: u8, pub detail: String }
#[derive(Deserialize)] struct GhRelease { assets: Vec<GhAsset> }
#[derive(Deserialize)] struct GhAsset { name: String, browser_download_url: String }
pub fn engine_dir() -> PathBuf { let d = config::app_data_dir().join("engine"); let _ = fs::create_dir_all(&d); d }
pub fn find_server_exe() -> Option<PathBuf> {
  let p = engine_dir().join("sd-server.exe"); if p.exists() { return Some(p); }
  let dev = PathBuf::from("C:/KI/sd/sd-server.exe"); if dev.exists() { return Some(dev); }
  None
}
pub fn status(backend_choice: &str) -> EngineStatus {
  let b = if backend_choice == "auto" { hardware::detect().recommended_backend } else { backend_choice.to_string() };
  let installed = find_server_exe().is_some();
  EngineStatus { installed, engine_dir: engine_dir().to_string_lossy().into(), backend: b }
}
async fn dl_zip(app: &AppHandle, url: &str, label: &str, dest: &Path) -> Result<(), String> {
  let client = reqwest::Client::builder().user_agent("BytebeatFluxStudio/0.5").build().map_err(|e| e.to_string())?;
  let res = client.get(url).send().await.map_err(|e| e.to_string())?;
  if !res.status().is_success() { return Err(format!("HTTP {}", res.status())); }
  let total = res.content_length().unwrap_or(0); let mut got = 0u64; let mut buf = Vec::new();
  let mut st = res.bytes_stream();
  while let Some(ch) = st.next().await { let b = ch.map_err(|e| e.to_string())?; got += b.len() as u64; buf.extend_from_slice(&b);
    let pct = if total > 0 { ((got * 100) / total).min(99) as u8 } else { 50 };
    let _ = app.emit("engine_progress", ProgressEvt { target: "engine".into(), file: label.into(), percent: pct, detail: format!("{:.1} MB", got as f64 / 1048576.0) });
  }
  let mut ar = zip::ZipArchive::new(Cursor::new(buf)).map_err(|e| e.to_string())?;
  for i in 0..ar.len() { let mut f = ar.by_index(i).map_err(|e| e.to_string())?; if f.is_dir() { continue; }
    if let Some(name) = Path::new(f.name()).file_name() { let mut out = fs::File::create(dest.join(name)).map_err(|e| e.to_string())?; std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?; }
  }
  Ok(())
}
pub async fn install(app: AppHandle, backend: String) -> Result<EngineStatus, String> {
  let client = reqwest::Client::builder().user_agent("BytebeatFluxStudio/0.5").build().map_err(|e| e.to_string())?;
  let rel: GhRelease = client.get("https://api.github.com/repos/leejet/stable-diffusion.cpp/releases/latest").send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
  let key = match backend.as_str() { "cuda12" => "-bin-win-cuda12-x64.zip", "cpu" => "-bin-win-cpu-x64.zip", _ => "-bin-win-vulkan-x64.zip" };
  let main_a = rel.assets.iter().find(|a| a.name.ends_with(key)).ok_or("Kein passendes Engine-Paket gefunden")?;
  let dest = engine_dir();
  dl_zip(&app, &main_a.browser_download_url, &main_a.name, &dest).await?;
  if backend == "cuda12" { if let Some(rt) = rel.assets.iter().find(|a| a.name.contains("cudart") && a.name.ends_with(".zip")) { dl_zip(&app, &rt.browser_download_url, &rt.name, &dest).await?; } }
  let _ = app.emit("engine_progress", ProgressEvt { target: "engine".into(), file: "Fertig".into(), percent: 100, detail: "Bereit".into() });
  Ok(status(&backend))
}
