use crate::{config::AppConfig, local_db};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct SearchOpts {
  pub source: String, pub img_type: String, pub aspect: String,
  pub license_free: bool, pub safe_search: Option<bool>, pub limit: usize
}
#[derive(Serialize, Clone)]
pub struct SearchItem { pub url: String, pub thumb: String, pub title: String, pub source: String, pub width: u32, pub height: u32 }
#[derive(Serialize, Clone)]
pub struct SearchResult { pub query: String, pub items: Vec<SearchItem> }

fn client() -> reqwest::Client {
  reqwest::Client::builder().user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/128.0.0.0").timeout(std::time::Duration::from_secs(8)).build().unwrap()
}
fn match_aspect(w: u32, h: u32, aspect: &str) -> bool {
  if w == 0 || h == 0 || aspect.is_empty() || aspect == "all" { return true; }
  let r = w as f32 / h as f32;
  match aspect { "wide" => r > 1.15, "tall" => r < 0.87, "square" => (r - 1.0).abs() <= 0.18, _ => true }
}
fn q_with_type(q: &str, t: &str) -> String {
  match t { "bw" => format!("{} black and white vintage", q), "clipart" => format!("{} vector logo", q), "photo" => format!("{} photo", q), _ => q.to_string() }
}
async fn search_wikimedia(cl: &reqwest::Client, q: &str, opts: &SearchOpts) -> Vec<SearchItem> {
  let mut out = Vec::new();
  let u = format!("https://commons.wikimedia.org/w/api.php?action=query&generator=search&gsrnamespace=6&gsrsearch={}&gsrlimit=20&prop=imageinfo&iiprop=url|mime|size&iiurlwidth=540&format=json", urlencoding::encode(&q_with_type(q, &opts.img_type)));
  let Ok(v) = cl.get(&u).header("User-Agent","BytebeatFluxStudio/1.0").send().await else { return out; };
  let Ok(j) = v.json::<serde_json::Value>().await else { return out; };
  if let Some(pages) = j.pointer("/query/pages").and_then(|x| x.as_object()) {
    for (_, p) in pages { if let Some(ii) = p.pointer("/imageinfo/0") {
      let mime = ii.get("mime").and_then(|m| m.as_str()).unwrap_or("");
      if !mime.starts_with("image/") || mime.contains("svg") { continue; }
      let w = ii.get("width").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
      let h = ii.get("height").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
      if !match_aspect(w, h, &opts.aspect) { continue; }
      let full = ii.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
      let th = ii.get("thumburl").and_then(|x| x.as_str()).unwrap_or(&full).to_string();
      let title = p.get("title").and_then(|x| x.as_str()).unwrap_or("Wikimedia").replace("File:","");
      if !full.is_empty() { out.push(SearchItem { url: full, thumb: th, title, source: "Wikimedia".into(), width: w, height: h }); }
    }}
  }
  out
}
async fn search_openverse(cl: &reqwest::Client, q: &str, opts: &SearchOpts) -> Vec<SearchItem> {
  let mut out = Vec::new();
  let mut u = format!("https://api.openverse.org/v1/images/?q={}&page_size=24", urlencoding::encode(q));
  if matches!(opts.aspect.as_str(), "wide" | "tall" | "square") { u.push_str(&format!("&aspect_ratio={}", opts.aspect)); }
  if opts.img_type == "photo" { u.push_str("&category=photograph"); }
  else if opts.img_type == "art" { u.push_str("&category=illustration,digitized_artwork"); }
  if opts.license_free { u.push_str("&license_type=commercial,modification"); }
  let Ok(r) = cl.get(&u).header("User-Agent","BytebeatFluxStudio/1.0").send().await else { return out; };
  let Ok(j) = r.json::<serde_json::Value>().await else { return out; };
  if let Some(arr) = j.get("results").and_then(|x| x.as_array()) {
    for it in arr {
      let full = it.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
      let th = it.get("thumbnail").and_then(|x| x.as_str()).unwrap_or(&full).to_string();
      let title = it.get("title").and_then(|x| x.as_str()).unwrap_or("Openverse").to_string();
      let w = it.get("width").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
      let h = it.get("height").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
      if full.starts_with("http") && match_aspect(w, h, &opts.aspect) { out.push(SearchItem { url: full, thumb: th, title, source: "Openverse".into(), width: w, height: h }); }
    }
  }
  out
}
#[derive(Deserialize)] struct DdgImg { image: String, thumbnail: Option<String>, title: Option<String>, width: Option<u32>, height: Option<u32> }
#[derive(Deserialize)] struct DdgResp { results: Vec<DdgImg> }
async fn search_ddg(cl: &reqwest::Client, q: &str, safe: bool, opts: &SearchOpts) -> Vec<SearchItem> {
  let mut out = Vec::new();
  let qq = if opts.img_type == "bw" { format!("{} black and white", q) } else { q.to_string() };
  let Ok(html) = cl.get(format!("https://duckduckgo.com/?q={}", urlencoding::encode(&qq))).send().await else { return out; };
  let Ok(txt) = html.text().await else { return out; };
  let re = Regex::new(r"vqd=[^0-9-]*([\d-]+)").unwrap();
  let Some(cap) = re.captures(&txt) else { return out; };
  let tf = match opts.img_type.as_str() { "photo" => "type:photo", "clipart" => "type:clipart", "transparent" => "type:transparent", _ => "" };
  let lf = match opts.aspect.as_str() { "wide" => "layout:Wide", "tall" => "layout:Tall", "square" => "layout:Square", _ => "" };
  let lic = if opts.license_free { "license:Share" } else { "" };
  let u = format!("https://duckduckgo.com/i.js?l=wt-wt&o=json&q={}&vqd={}&f={}&p={}", urlencoding::encode(&qq), &cap[1], urlencoding::encode(&format!(",{},,{},{}", tf, lf, lic)), if safe { "1" } else { "-1" });
  let Ok(r) = cl.get(&u).header("Referer", "https://duckduckgo.com/").send().await else { return out; };
  if let Ok(d) = r.json::<DdgResp>().await { for it in d.results.into_iter().take(28) {
    let w = it.width.unwrap_or(0); let h = it.height.unwrap_or(0);
    if !match_aspect(w, h, &opts.aspect) { continue; }
    let th = it.thumbnail.unwrap_or_else(|| it.image.clone());
    out.push(SearchItem { url: it.image, thumb: th, title: it.title.unwrap_or_else(|| "Web".into()), source: "Websuche".into(), width: w, height: h });
  }}
  out
}
async fn search_pexels(cl: &reqwest::Client, key: &str, q: &str, opts: &SearchOpts) -> Vec<SearchItem> {
  let mut out = Vec::new(); if key.trim().is_empty() { return out; }
  let ori = match opts.aspect.as_str() { "wide" => "&orientation=landscape", "tall" => "&orientation=portrait", "square" => "&orientation=square", _ => "" };
  let u = format!("https://api.pexels.com/v1/search?query={}&per_page=18{}", urlencoding::encode(q), ori);
  let Ok(r) = cl.get(&u).header("Authorization", key.trim()).send().await else { return out; };
  let Ok(j) = r.json::<serde_json::Value>().await else { return out; };
  if let Some(arr) = j.get("photos").and_then(|x| x.as_array()) {
    for it in arr {
      let full = it.pointer("/src/large2x").or_else(|| it.pointer("/src/original")).and_then(|x| x.as_str()).unwrap_or("").to_string();
      let th = it.pointer("/src/medium").and_then(|x| x.as_str()).unwrap_or(&full).to_string();
      let title = it.get("alt").and_then(|x| x.as_str()).unwrap_or("Pexels").to_string();
      let w = it.get("width").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
      let h = it.get("height").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
      if full.starts_with("http") { out.push(SearchItem { url: full, thumb: th, title, source: "Pexels".into(), width: w, height: h }); }
    }
  }
  out
}
async fn search_brave(cl: &reqwest::Client, key: &str, q: &str, safe: bool, opts: &SearchOpts) -> Vec<SearchItem> {
  let mut out = Vec::new(); if key.trim().is_empty() { return out; }
  let u = format!("https://api.search.brave.com/res/v1/images/search?q={}&count=20&safesearch={}", urlencoding::encode(&q_with_type(q, &opts.img_type)), if safe { "strict" } else { "off" });
  let Ok(r) = cl.get(&u).header("X-Subscription-Token", key.trim()).send().await else { return out; };
  let Ok(j) = r.json::<serde_json::Value>().await else { return out; };
  if let Some(arr) = j.get("results").and_then(|x| x.as_array()) {
    for it in arr {
      let full = it.pointer("/properties/url").and_then(|x| x.as_str()).unwrap_or("").to_string();
      let th = it.pointer("/thumbnail/src").and_then(|x| x.as_str()).unwrap_or(&full).to_string();
      let title = it.get("title").and_then(|x| x.as_str()).unwrap_or("Brave").to_string();
      let w = it.pointer("/properties/width").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
      let h = it.pointer("/properties/height").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
      if full.starts_with("http") && match_aspect(w, h, &opts.aspect) { out.push(SearchItem { url: full, thumb: th, title, source: "Brave".into(), width: w, height: h }); }
    }
  }
  out
}
async fn search_custom_site(cl: &reqwest::Client, site: &str, q: &str, safe: bool, opts: &SearchOpts) -> Vec<SearchItem> {
  if !site.starts_with("http") { return search_ddg(cl, &format!("site:{} {}", site.trim(), q), safe, opts).await; }
  let mut out = Vec::new(); let Ok(r) = cl.get(site).send().await else { return out; };
  let Ok(body) = r.text().await else { return out; }; let doc = Html::parse_document(&body);
  let Ok(sel) = Selector::parse("img[src]") else { return out; };
  let base = url::Url::parse(site).ok();
  for el in doc.select(&sel).take(16) { if let Some(src) = el.value().attr("src") {
    let full = if src.starts_with("http") { src.to_string() } else if let Some(ref b) = base { b.join(src).map(|u| u.to_string()).unwrap_or_default() } else { String::new() };
    if full.starts_with("http") { out.push(SearchItem { url: full.clone(), thumb: full, title: el.value().attr("alt").unwrap_or(site).into(), source: "Eigene Seite".into(), width: 0, height: 0 }); }
  }}
  out
}
pub async fn run_search(cfg: &AppConfig, query: &str, opts: SearchOpts) -> SearchResult {
  let cl = client(); let mut items = Vec::new(); let mut seen = HashSet::new();
  let src = if opts.source.is_empty() { "all" } else { opts.source.as_str() };
  let safe = opts.safe_search.unwrap_or(cfg.sources.safe_search);
  let lim = (if opts.limit > 0 { opts.limit } else { cfg.sources.default_limit }).clamp(6, 60);
  if src == "all" || src == "local" {
    for l in local_db::search_text(query, 10) { items.push(SearchItem { url: l.path, thumb: l.data_url, title: l.title, source: "Lokal-Archiv".into(), width: 0, height: 0 }); }
  }
  let use_web = (src == "all" && cfg.sources.web_search) || src == "web";
  let use_wiki = (src == "all" && cfg.sources.wikimedia) || src == "wikimedia";
  let use_open = (src == "all" && cfg.sources.openverse) || src == "openverse";
  let use_pexels = (src == "all" || src == "pexels") && !cfg.sources.pexels_api_key.trim().is_empty();
  let use_brave = (src == "all" || src == "brave") && !cfg.sources.brave_api_key.trim().is_empty();
  let (r_web, r_wiki, r_open, r_pex, r_brv) = tokio::join!(
    async { if use_web { search_ddg(&cl, query, safe, &opts).await } else { vec![] } },
    async { if use_wiki { search_wikimedia(&cl, query, &opts).await } else { vec![] } },
    async { if use_open { search_openverse(&cl, query, &opts).await } else { vec![] } },
    async { if use_pexels { search_pexels(&cl, &cfg.sources.pexels_api_key, query, &opts).await } else { vec![] } },
    async { if use_brave { search_brave(&cl, &cfg.sources.brave_api_key, query, safe, &opts).await } else { vec![] } }
  );
  for it in r_pex.into_iter().chain(r_brv).chain(r_web).chain(r_wiki).chain(r_open) { if seen.insert(it.url.clone()) { items.push(it); } }
  if src == "all" || src == "custom" {
    for u in &cfg.sources.custom_urls { let s = u.trim(); if !s.is_empty() { for it in search_custom_site(&cl, s, query, safe, &opts).await { if seen.insert(it.url.clone()) { items.push(it); } } } }
  }
  items.truncate(lim); SearchResult { query: query.into(), items }
}
pub async fn fetch_image_b64(url: &str, fallback: &str) -> Result<String, String> {
  if url.starts_with("data:image/") { return Ok(url.to_string()); }
  if Path::new(url).exists() { let raw = std::fs::read(url).map_err(|e| e.to_string())?; return Ok(format!("data:image/png;base64,{}", B64.encode(&raw))); }
  let cl = client();
  for cand in [url, fallback] { if !cand.starts_with("http") { continue; }
    if let Ok(resp) = cl.get(cand).send().await { if !resp.status().is_success() { continue; }
      let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
      if ct.contains("html") { continue; }
      if let Ok(bytes) = resp.bytes().await { if bytes.len() < 800 || bytes.starts_with(b"<") { continue; }
        let mime = if bytes.starts_with(&[137,80,78,71]) { "image/png" } else if bytes.starts_with(&[255,216]) { "image/jpeg" } else if bytes.starts_with(b"RIFF") { "image/webp" } else { "image/jpeg" };
        return Ok(format!("data:{};base64,{}", mime, B64.encode(&bytes)));
      }
    }
  }
  Err("Bild konnte nicht geladen werden".into())
}
