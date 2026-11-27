const inv = (c, a) => window.__TAURI__.core.invoke(c, a || {});
const ev = window.__TAURI__.event;
const el = id => document.getElementById(id);
let cfg = null, readyModels = [], catItems = [], activeDls = {}, curModel = null, refs = [], hist = [], curItem = null, srvReady = false, zoomFn = null, batchAbort = false;

async function pickDir(targetEl) { const p = await window.__TAURI__.dialog.open({ directory: true, multiple: false }); if (p) targetEl.value = p; }
function openDir(path) { if (path) inv("open_folder", { path }).catch(e => alert(e)); }
function showView(v) {
  ["viewWizard", "viewStudio", "viewModels", "viewBatch"].forEach(id => el(id).hidden = true);
  el("navTabs").hidden = (v === "wizard"); el("srvBar").hidden = (v === "wizard");
  document.querySelectorAll(".tab").forEach(t => t.classList.toggle("act", t.dataset.tab === v || (v === "wizard" && t.dataset.tab === "settings")));
  if (v === "wizard") el("viewWizard").hidden = false;
  else if (v === "models") { el("viewModels").hidden = false; refreshModels(); }
  else if (v === "batch") el("viewBatch").hidden = false;
  else el("viewStudio").hidden = false;
}
document.querySelectorAll(".tab").forEach(t => t.onclick = () => showView(t.dataset.tab === "settings" ? "wizard" : t.dataset.tab));

async function init() {
  const hw = await inv("detect_hardware"); cfg = await inv("get_config");
  const es = await inv("get_engine_status", { backendChoice: cfg.backend_choice || "auto" });
  el("hwInfo").textContent = "GPU: " + hw.gpu_name + " (" + hw.vendor + ") | RAM: " + hw.ram_gb + " GB | Empfohlen: " + hw.recommended_backend + " | Engine: " + (es.installed ? "Bereit" : "Fehlt (1 Klick laden)");
  el("wizBackend").value = cfg.backend_choice || "auto"; el("wizOut").value = cfg.output_dir || ""; el("wizMod").value = cfg.models_dir || "";
  el("srcWiki").checked = cfg.sources.wikimedia; el("srcOpen").checked = cfg.sources.openverse;
  el("srcWeb").checked = cfg.sources.web_search; el("srcSafe").checked = cfg.sources.safe_search;
  el("srcLicFree").checked = !!cfg.sources.license_free;
  el("srcDefLimit").value = String(cfg.sources.default_limit || 24);
  el("srcDefType").value = cfg.sources.default_img_type || "all";
  el("srcDefAspect").value = cfg.sources.default_aspect || "all";
  el("srchSafe").checked = !!cfg.sources.safe_search;
  el("srchLicFree").checked = !!cfg.sources.license_free;
  el("srchLimit").value = String(cfg.sources.default_limit || 24);
  el("srchType").value = cfg.sources.default_img_type || "all";
  el("srchAspect").value = cfg.sources.default_aspect || "all";
  el("wizDb").value = cfg.sources.local_db_dir || ""; el("wizUrls").value = (cfg.sources.custom_urls || []).join("\n");
  el("keyBrave").value = cfg.sources.brave_api_key || ""; el("keyPexels").value = cfg.sources.pexels_api_key || "";
  el("outLabel").textContent = cfg.output_dir || "-";
  await refreshModels(); showView(cfg.setup_done ? "studio" : "wizard"); pollServer();
}
ev.listen("engine_progress", e => { const p = e.payload; el("engProg").hidden = false; el("engBar").style.width = p.percent + "%"; el("engTxt").textContent = p.file + " (" + p.detail + ")"; });
ev.listen("model_progress", e => { const p = e.payload; activeDls[p.model_id] = p; renderDownloads(); if (p.percent >= 100) setTimeout(() => { delete activeDls[p.model_id]; renderDownloads(); }, 2500); });
function renderDownloads() {
  const q = el("dlQueue"); if (!q) return; const keys = Object.keys(activeDls); q.hidden = keys.length === 0;
  keys.forEach(id => {
    const p = activeDls[id]; let row = document.getElementById("dl_" + id);
    if (!row) { row = document.createElement("div"); row.id = "dl_" + id; row.className = "dl-item"; row.innerHTML = "<div class='dl-top'><span class='dl-title'></span><span class='dl-meta'></span></div><div class='barbg'><div class='bar'></div></div>"; q.append(row); }
    const st = p.file_total ? ("Datei " + p.file_idx + " von " + p.file_total) : "Vorbereitung";
    row.querySelector(".dl-title").textContent = (p.model_name || p.model_id) + " — " + st;
    row.querySelector(".dl-meta").textContent = p.detail + "  (" + p.percent + " %)"; row.querySelector(".bar").style.width = p.percent + "%";
  });
  Array.from(q.children).forEach(ch => { if (!activeDls[ch.id.replace("dl_", "")]) ch.remove(); });
}
ev.listen("gen_progress", e => { const p = e.payload; el("genProg").hidden = false; const pct = p.total ? Math.round((p.completed * 100) / p.total) : 15; el("genBar").style.width = pct + "%"; el("genMsg").textContent = p.total ? ("Berechne Schritt " + p.completed + " von " + p.total + "...") : ("Status: " + p.status); });
ev.listen("batch_progress", e => { const p = e.payload; el("batProg").hidden = false; const pct = p.total ? Math.round(p.done * 100 / p.total) : 0; el("batBar").style.width = pct + "%"; el("batTxt").textContent = p.done + "/" + p.total + " - " + p.current + " (" + p.status + ")"; });
el("pickOut").onclick = () => pickDir(el("wizOut")); el("pickMod").onclick = () => pickDir(el("wizMod"));
el("pickDb").onclick = () => pickDir(el("wizDb")); el("pickBatIn").onclick = () => pickDir(el("batIn"));
el("openOutTop").onclick = () => openDir(cfg && cfg.output_dir);
el("openOutSet").onclick = () => openDir(el("wizOut").value.trim());
el("openModSet").onclick = () => openDir(el("wizMod").value.trim());
el("wizEngBtn").onclick = async () => { el("wizEngBtn").disabled = true; try { const b = el("wizBackend").value; const bk = b === "auto" ? (await inv("detect_hardware")).recommended_backend : b; const st = await inv("install_engine", { backend: bk }); el("engTxt").textContent = "Engine bereit in: " + st.engine_dir; } catch (err) { alert(err); } finally { el("wizEngBtn").disabled = false; } };
el("scanBtn").onclick = async () => { const r = await inv("scan_models", { modelsDir: el("wizMod").value }); el("scanOut").textContent = "Gefunden: " + r.ready.length + " einsatzbereit | Hinweise: " + (r.loose_hints.join(", ") || "keine"); };
el("idxBtn").onclick = async () => { el("dbOut").textContent = "Indiziere Bilder & berechne dHash..."; const s = await inv("index_local_db", { dir: el("wizDb").value }); el("dbOut").textContent = "Neu indiziert: " + s.indexed + " | Gesamt in Datenbank: " + s.total; };
el("wizSave").onclick = async () => {
  cfg.setup_done = true; cfg.output_dir = el("wizOut").value.trim(); cfg.models_dir = el("wizMod").value.trim(); cfg.backend_choice = el("wizBackend").value;
  cfg.sources = {
    wikimedia: el("srcWiki").checked, openverse: el("srcOpen").checked, web_search: el("srcWeb").checked,
    safe_search: el("srcSafe").checked, license_free: el("srcLicFree").checked,
    default_limit: Number(el("srcDefLimit").value) || 24, default_img_type: el("srcDefType").value, default_aspect: el("srcDefAspect").value,
    local_db_dir: el("wizDb").value.trim(), custom_urls: el("wizUrls").value.split(/\r?\n/).map(x => x.trim()).filter(Boolean),
    auto_tag: true, brave_api_key: el("keyBrave").value.trim(), pexels_api_key: el("keyPexels").value.trim()
  };
  if (!cfg.output_dir || !cfg.models_dir) { alert("Bitte Ausgabe- und Modellordner waehlen."); return; }
  await inv("save_config", { cfg }); el("outLabel").textContent = cfg.output_dir;
  el("srchSafe").checked = cfg.sources.safe_search; el("srchLicFree").checked = cfg.sources.license_free;
  el("srchLimit").value = String(cfg.sources.default_limit); el("srchType").value = cfg.sources.default_img_type; el("srchAspect").value = cfg.sources.default_aspect;
  if (cfg.sources.local_db_dir) inv("index_local_db", { dir: cfg.sources.local_db_dir });
  el("wizSavedToast").hidden = false; setTimeout(() => el("wizSavedToast").hidden = true, 2500);
  await refreshModels(); showView("studio");
};
async function refreshModels() {
  const dir = cfg ? cfg.models_dir : ""; const sr = await inv("scan_models", { modelsDir: dir }); readyModels = sr.ready || [];
  [el("modelSel"), el("batModel")].forEach(sel => { sel.innerHTML = ""; readyModels.forEach(m => { const o = document.createElement("option"); o.value = m.id; o.textContent = m.name; sel.append(o); }); });
  const cv = await inv("get_catalog", { modelsDir: dir });
  el("catHwBanner").innerHTML = "<strong>System-Check:</strong> " + cv.hw_summary + " &bull; <strong>" + cv.items.length + " passende Modelle</strong>" + (cv.hidden_count ? " (" + cv.hidden_count + " zu grosse Workstation-Modelle ausgeblendet)" : "");
  catItems = cv.items || []; renderCatalog(); el("scanHints").textContent = (sr.loose_hints || []).join(" | ");
}
function renderCatalog() {
  const q = (el("catSearch").value || "").toLowerCase().trim(), f = el("catFilter").value, cg = el("catGrid"); cg.innerHTML = "";
  catItems.filter(c => (!q || (c.name + " " + c.summary + " " + c.tag).toLowerCase().includes(q)) && (f === "all" || (f === "fast" && c.speed.includes("Sehr schnell")) || (f === "ref" && c.supports_ref) || (f === "inst" && c.installed))).forEach(c => {
    const d = document.createElement("div"); d.className = "mcard" + (c.installed ? " inst" : "");
    d.innerHTML = "<div><span class='mtag'>" + c.tag + "</span> <span class='mtag'>" + c.speed + "</span>" + (c.supports_ref ? " <span class='mtag'>Referenz ✓</span>" : "") + "<h3 style='margin-top:8px'>" + c.name + "</h3><div style='font-size:12.5px;color:#94a3b8;margin-top:4px'>" + c.summary + "</div><div style='font-size:11.5px;color:#cbd5e1;margin-top:6px'>Groesse: " + c.size_gb + " GB | Min RAM: " + c.min_ram_gb + " GB | " + c.license + "</div></div>";
    const b = document.createElement("button"); const busy = !!activeDls[c.id];
    b.className = c.installed ? "btn sec" : "btn primary"; b.textContent = c.installed ? "Installiert ✓" : (busy ? "Wird geladen..." : "Aus Hugging Face laden (" + c.size_gb + " GB)"); b.disabled = c.installed || busy;
    b.onclick = async () => {
      activeDls[c.id] = { model_id: c.id, model_name: c.name, file_idx: 0, file_total: 0, percent: 1, detail: "Starte..." }; renderDownloads(); renderCatalog();
      try { await inv("download_model", { modelsDir: cfg.models_dir, modelId: c.id }); await refreshModels(); } catch (e) { delete activeDls[c.id]; renderDownloads(); renderCatalog(); alert(e); }
    };
    d.append(b); cg.append(d);
  });
}
if (el("catSearch")) { el("catSearch").oninput = renderCatalog; el("catFilter").onchange = renderCatalog; }
el("rescanBtn").onclick = refreshModels;
let lastSrvKey = "";
async function pollServer() {
  let busy = false;
  try {
    const s = await inv("get_server_status"); srvReady = s.ready; busy = s.loading;
    const key = [s.ready, s.loading, s.model_name, s.error].join("|");
    if (key !== lastSrvKey) {
      lastSrvKey = key;
      el("genBtn").disabled = !srvReady; el("loadBtn").disabled = s.loading; el("unloadBtn").hidden = !(s.ready || s.loading);
      el("srvBadge").className = "badge " + (s.ready ? "" : (s.loading ? "busy" : "idle"));
      el("srvBadge").textContent = s.ready ? "Aktiv: " + s.model_name : (s.loading ? "Lade Modell..." : (s.error ? "Ladefehler" : "Kein Modell geladen"));
      if (s.error && !s.loading && !s.ready) el("genMsg").textContent = "Server-Fehler: " + s.error;
    }
  } catch (e) {}
  setTimeout(pollServer, srvReady ? 4000 : (busy ? 1000 : 2800));
}
el("loadBtn").onclick = async () => {
  const m = readyModels.find(x => x.id === el("modelSel").value); if (!m) { alert("Kein Modell verfuegbar."); return; }
  curModel = m; el("steps").value = String(m.steps); el("cfg").value = String(m.cfg);
  el("loadBtn").disabled = true; el("unloadBtn").hidden = false; el("srvBadge").className = "badge busy"; el("srvBadge").textContent = "Lade Modell...";
  el("genMsg").textContent = "Lade " + m.name + " in den Speicher (10-30 Sek.)..."; checkRefWarn();
  try { await inv("load_model_server", { cfg, model: m }); } catch (e) { el("genMsg").textContent = "Fehler: " + e; el("loadBtn").disabled = false; }
};
el("unloadBtn").onclick = () => inv("unload_model_server");
el("clrBtn").onclick = () => { el("prompt").value = ""; };
document.querySelectorAll(".ktile").forEach(k => k.onclick = () => { document.querySelectorAll(".ktile").forEach(x => x.classList.remove("act")); k.classList.add("act"); if (k.dataset.preset) el("prompt").value = k.dataset.preset; });
document.querySelectorAll(".chip[data-add]").forEach(b => b.onclick = () => { const p = el("prompt"); p.value = p.value.trim() ? p.value.trim() + ", " + b.dataset.add : b.dataset.add; });
document.querySelectorAll(".qset").forEach(b => b.onclick = () => { document.querySelectorAll(".qset").forEach(x => x.classList.remove("act")); b.classList.add("act"); el("steps").value = b.dataset.steps; el("cfg").value = b.dataset.cfg; });
document.querySelectorAll(".bpreset").forEach(b => b.onclick = () => { el("batPrompt").value = b.dataset.p; el("batSteps").value = b.dataset.st; });
function openZoom(src, fn) { el("zoomImg").src = src; zoomFn = fn || null; el("zoomPick").hidden = !zoomFn; el("zoomModal").hidden = false; }
el("zoomClose").onclick = () => el("zoomModal").hidden = true;
el("zoomPick").onclick = async () => { if (zoomFn) await zoomFn(); el("zoomModal").hidden = true; };
document.addEventListener("keydown", e => { if (e.key === "Escape") el("zoomModal").hidden = true; });
function scaleAdd(du) {
  return new Promise(res => { const im = new Image(); im.onload = () => {
    let w = im.width, h = im.height, mx = 768;
    if (w > mx || h > mx) { if (w >= h) { h = Math.round(h * mx / w); w = mx; } else { w = Math.round(w * mx / h); h = mx; } }
    const c = document.createElement("canvas"); c.width = w; c.height = h; c.getContext("2d").drawImage(im, 0, 0, w, h); refs.push(c.toDataURL("image/png")); renderRefs(); res();
  }; im.src = du; });
}
function checkRefWarn() { const w = el("refWarn"); if (!w) return; if (refs.length > 0 && curModel && !curModel.supports_ref) { w.hidden = false; w.textContent = "Hinweis: " + curModel.name + " nutzt keine Referenzbilder. Fuer Umbau/Kolorierung bitte Qwen-Image 2.1, FLUX.3 Klein, FLUX.2 Klein, MiniMax-H3 oder Qwen-Edit laden."; } else { w.hidden = true; } }
function renderRefs() {
  checkRefWarn(); const b = el("refs"); b.innerHTML = "";
  refs.forEach((u, i) => { const d = document.createElement("div"); d.className = "rcard"; const ig = document.createElement("img"); ig.src = u; ig.onclick = () => openZoom(u, null); const x = document.createElement("button"); x.textContent = "×"; x.onclick = () => { refs.splice(i, 1); renderRefs(); }; d.append(ig, x); b.append(d); });
}
const drp = el("drop"), fi = el("fileIn"); drp.onclick = () => fi.click(); drp.ondragover = e => e.preventDefault();
drp.ondrop = e => { e.preventDefault(); addFiles(e.dataTransfer.files); }; fi.onchange = () => { addFiles(fi.files); fi.value = ""; };
function addFiles(fs) { Array.from(fs).forEach(f => { if (!f.type.startsWith("image/")) return; const r = new FileReader(); r.onload = ev => scaleAdd(ev.target.result); r.readAsDataURL(f); }); }
el("clrRefs").onclick = () => { refs = []; renderRefs(); };
function showSearchItems(items) {
  const wb = el("webImgs"); wb.innerHTML = "";
  items.forEach(it => {
    const d = document.createElement("div"); d.className = "wcard"; const ig = document.createElement("img"); ig.src = it.thumb || it.url;
    const dim = (it.width && it.height) ? (" · " + it.width + "×" + it.height) : "";
    const tg = document.createElement("span"); tg.className = "src"; tg.textContent = it.source + dim;
    const bt = document.createElement("button"); bt.className = "pick"; bt.textContent = "+ Als Referenz";
    const pk = async () => { bt.textContent = "Laedt..."; try { const du = await inv("fetch_image_b64", { url: it.url, fallback: it.thumb || it.url }); await scaleAdd(du); d.classList.add("picked"); bt.textContent = "Ausgewaehlt ✓"; } catch (e) { bt.textContent = "Fehler"; } };
    ig.onclick = async () => { openZoom(it.thumb || it.url, pk); try { el("zoomImg").src = await inv("fetch_image_b64", { url: it.url, fallback: it.thumb || it.url }); } catch (e) {} };
    bt.onclick = pk; d.append(ig, tg, bt); wb.append(d);
  });
}
el("searchClrBtn").onclick = () => { el("webImgs").innerHTML = ""; el("searchInfo").textContent = ""; };
el("searchBtn").onclick = async () => {
  const q = el("searchQ").value.trim() || el("prompt").value.trim(); if (!q) return;
  el("searchInfo").textContent = "Suche laeuft im Hintergrund...";
  const opts = { source: el("srchSrc").value, img_type: el("srchType").value, aspect: el("srchAspect").value, limit: Number(el("srchLimit").value) || 24, license_free: el("srchLicFree").checked, safe_search: el("srchSafe").checked };
  const r = await inv("search_images", { cfg, query: q, opts });
  el("searchInfo").textContent = "Gefunden: " + r.items.length + " Treffer fuer: " + r.query; showSearchItems(r.items);
};
el("simBtn").onclick = async () => { if (!refs.length) { alert("Erst ein Referenzbild ablegen."); return; } const hits = await inv("search_local_similar", { dataUrl: refs[0] }); el("searchInfo").textContent = "Aehnliche im Lokal-Archiv: " + hits.length; showSearchItems(hits.map(h => ({ url: h.path, thumb: h.data_url, title: h.title, source: "dHash " + h.distance, width: 0, height: 0 }))); };
function showHistItem(item) { curItem = item; el("outImg").src = item.data_url; el("outImg").hidden = false; el("empty").hidden = true; el("outTools").hidden = false; el("outMeta").textContent = "Seed: " + item.seed + " | " + item.size + " | " + item.saved_path; }
function renderHist() { const h = el("hist"); h.innerHTML = ""; hist.slice(0, 16).forEach(it => { const ig = document.createElement("img"); ig.src = it.data_url; ig.title = it.prompt; ig.onclick = () => showHistItem(it); h.append(ig); }); }
el("genBtn").onclick = async () => {
  const p = el("prompt").value.trim(); if (!p) return;
  const [w, h] = el("size").value.split("x").map(Number), cnt = Number(el("batchCount").value) || 1;
  batchAbort = false; el("genBtn").disabled = true; el("cancelBtn").hidden = false; el("genProg").hidden = false;
  try {
    for (let i = 0; i < cnt; i++) {
      if (batchAbort) break;
      el("genBar").style.width = "8%"; el("genMsg").textContent = cnt > 1 ? ("Variante " + (i + 1) + " von " + cnt + "...") : "Berechne Bild...";
      const sd = Number(el("seed").value); const useSeed = (cnt > 1 && sd >= 0) ? (sd + i) : sd;
      const res = await inv("generate_image", { cfg, req: { prompt: p, negative_prompt: el("neg").value.trim(), width: w, height: h, steps: Number(el("steps").value), cfg: Number(el("cfg").value) || 1.0, seed: useSeed, ref_images: refs } });
      const item = { data_url: res.data_url, saved_path: res.saved_path, seed: res.seed, prompt: p, size: el("size").value };
      hist.unshift(item); renderHist(); showHistItem(item); el("genBar").style.width = "100%"; el("genMsg").textContent = "Fertig!";
    }
  } catch (e) { el("genMsg").textContent = "Fehler: " + e; } finally { el("genBtn").disabled = !srvReady; el("cancelBtn").hidden = true; }
};
el("cancelBtn").onclick = () => { batchAbort = true; inv("cancel_generate"); };
el("jpgBtn").onclick = async () => { if (!curItem) return; const p = await inv("export_jpg", { cfg, dataUrl: curItem.data_url }); el("outMeta").textContent = "JPG gespeichert: " + p; };
el("svgBtn").onclick = async () => { if (!curItem) return; const r = await inv("export_svg", { cfg, dataUrl: curItem.data_url }); el("outMeta").textContent = "SVG gespeichert: " + r.saved_path; };
el("toRefBtn").onclick = () => { if (curItem) scaleAdd(curItem.data_url); };
el("restoreBtn").onclick = () => { if (!curItem) return; el("prompt").value = curItem.prompt; el("seed").value = String(curItem.seed); el("size").value = curItem.size; };
el("outImg").onclick = () => { if (curItem) openZoom(curItem.data_url, null); };
el("batStart").onclick = async () => {
  const m = readyModels.find(x => x.id === el("batModel").value); if (!m || !el("batIn").value.trim()) { alert("Eingabeordner und Modell waehlen."); return; }
  const [w, h] = el("batSize").value.split("x").map(Number); el("batStart").disabled = true; el("batStop").hidden = false;
  try { const n = await inv("run_batch", { cfg, model: m, req: { input_dir: el("batIn").value.trim(), prompt: el("batPrompt").value.trim(), negative_prompt: el("batNeg").value.trim(), width: w, height: h, steps: Number(el("batSteps").value), cfg: m.cfg, skip_existing: el("batSkip").checked, max_ref_px: Number(el("batMaxPx").value) } }); el("batTxt").textContent = "Batch fertig: " + n + " Bilder verarbeitet."; }
  catch (e) { el("batTxt").textContent = "Fehler: " + e; } finally { el("batStart").disabled = false; el("batStop").hidden = true; }
};
el("batStop").onclick = () => inv("cancel_batch");
init();

