#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod config; mod engine; mod hardware; mod local_db; mod models; mod runner; mod search;
use std::sync::{atomic::Ordering, Arc};
use tauri::RunEvent;
#[tauri::command] fn detect_hardware() -> hardware::HardwareInfo { hardware::detect() }
#[tauri::command] fn get_config() -> config::AppConfig { config::load() }
#[tauri::command] fn save_config(cfg: config::AppConfig) -> Result<(), String> { config::save(&cfg) }
#[tauri::command] fn get_engine_status(backend_choice: String) -> engine::EngineStatus { engine::status(&backend_choice) }
#[tauri::command] async fn install_engine(app: tauri::AppHandle, backend: String) -> Result<engine::EngineStatus, String> { engine::install(app, backend).await }
#[tauri::command] fn scan_models(models_dir: String) -> models::ScanResult { models::scan_models(&models_dir) }
#[tauri::command] fn get_catalog(models_dir: String) -> models::CatalogView { models::get_catalog(&models_dir) }
#[tauri::command] async fn download_model(app: tauri::AppHandle, models_dir: String, model_id: String) -> Result<models::ModelManifest, String> { models::download_model(app, models_dir, model_id).await }
#[tauri::command] fn index_local_db(dir: String) -> Result<local_db::IndexStats, String> { local_db::index_folder(&dir) }
#[tauri::command] fn search_local_similar(data_url: String) -> Result<Vec<local_db::LocalHit>, String> { local_db::search_similar(&data_url, 20) }
#[tauri::command] async fn search_images(cfg: config::AppConfig, query: String, opts: Option<search::SearchOpts>) -> search::SearchResult { search::run_search(&cfg, &query, opts.unwrap_or_default()).await }
#[tauri::command] fn open_folder(path: String) -> Result<(), String> { if path.trim().is_empty() { return Err("Pfad ist leer".into()); } std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?; Ok(()) }
#[tauri::command] async fn fetch_image_b64(url: String, fallback: String) -> Result<String, String> { search::fetch_image_b64(&url, &fallback).await }
#[tauri::command] fn load_model_server(st: tauri::State<'_, Arc<runner::RunnerState>>, cfg: config::AppConfig, model: models::ModelManifest) -> Result<(), String> { runner::start_server(&st, cfg, model) }
#[tauri::command] fn unload_model_server(st: tauri::State<'_, Arc<runner::RunnerState>>) { runner::stop_server(&st); }
#[tauri::command] async fn get_server_status(st: tauri::State<'_, Arc<runner::RunnerState>>) -> Result<runner::ServerStatus, String> { Ok(runner::server_status(&st).await) }
#[tauri::command] async fn generate_image(app: tauri::AppHandle, st: tauri::State<'_, Arc<runner::RunnerState>>, cfg: config::AppConfig, req: runner::GenReq) -> Result<runner::GenResult, String> { runner::generate_image(app, &st, cfg, req).await }
#[tauri::command] async fn cancel_generate(st: tauri::State<'_, Arc<runner::RunnerState>>) -> Result<(), String> { runner::cancel_generate(&st).await; Ok(()) }
#[tauri::command] fn export_jpg(cfg: config::AppConfig, data_url: String) -> Result<String, String> { runner::export_jpg(&cfg, &data_url) }
#[tauri::command] fn export_svg(cfg: config::AppConfig, data_url: String) -> Result<runner::SvgResult, String> { runner::export_svg(&cfg, &data_url) }
#[tauri::command] async fn run_batch(app: tauri::AppHandle, st: tauri::State<'_, Arc<runner::RunnerState>>, cfg: config::AppConfig, model: models::ModelManifest, req: runner::BatchReq) -> Result<usize, String> { runner::run_batch(app, st.inner().clone(), cfg, model, req).await }
#[tauri::command] fn cancel_batch(st: tauri::State<'_, Arc<runner::RunnerState>>) { st.batch_cancel.store(true, Ordering::SeqCst); }
fn main() {
  let st = Arc::new(runner::RunnerState::default());
  let app = tauri::Builder::default().manage(st.clone()).plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![detect_hardware,get_config,save_config,get_engine_status,install_engine,scan_models,get_catalog,download_model,index_local_db,search_local_similar,search_images,fetch_image_b64,open_folder,load_model_server,unload_model_server,get_server_status,generate_image,cancel_generate,export_jpg,export_svg,run_batch,cancel_batch])
    .build(tauri::generate_context!()).expect("Tauri Build Fehler");
  app.run(move |_h, ev| { if let RunEvent::Exit = ev { runner::stop_server(&st); } });
}


