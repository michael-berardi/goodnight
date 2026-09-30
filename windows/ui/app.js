"use strict";
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const win = window.__TAURI__.window.getCurrentWindow();
const $ = (id) => document.getElementById(id);
const view = new URLSearchParams(location.search).get("view") || "main";
document.body.dataset.view = view;

// Same curve as the Rust engine (src-tauri/src/warmth.rs), for colours in the UI.
const clamp = (x, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, x));
const smoothstep = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const kelvin = (l) => 6500 * Math.pow(1200 / 6500, l);
const dim = (l) => 1 - 0.42 * smoothstep(0.7, 1, l);
function whitePoint(t) {
  const u = (0.860117757 + 1.54118254e-4 * t + 1.28641212e-7 * t * t) / (1 + 8.42420235e-4 * t + 7.08145163e-7 * t * t);
  const v = (0.317398726 + 4.22806245e-5 * t + 4.20481691e-8 * t * t) / (1 - 2.89741816e-5 * t + 1.61456053e-7 * t * t);
  const d = 2 * u - 8 * v + 4, x = (3 * u) / d, y = (2 * v) / d, X = x / y, Z = (1 - x - y) / y;
  return [3.2406 * X - 1.5372 - 0.4986 * Z, -0.9689 * X + 1.8758 + 0.0415 * Z, 0.0557 * X - 0.204 + 1.057 * Z].map((c) => Math.max(0, c));
}
function gains(l) {
  const c = whitePoint(kelvin(l)), w = whitePoint(6500);
  let g = c.map((v, i) => v / w[i]);
  const m = Math.max(...g);
  g = g.map((v) => Math.pow(v / m, 1 / 2.2));
  g[2] = Math.max(0.15, g[2]);
  return g;
}
const rgb = (c) => `rgb(${c.map((v) => Math.round(clamp(v) * 255)).join(",")})`;
const tint = (l) => rgb(gains(l).map((v) => v * dim(l)));

const PRESETS = [
  { name: "Day", level: 0, icon: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>' },
  { name: "Golden", level: 0.25, icon: '<path d="M7 13a5 5 0 0 1 10 0z"/><path d="M12 4v2M5.6 7.1l1.4 1.4M18.4 7.1L17 8.5M4 17h16M7 20.5h10"/>' },
  { name: "Sunset", level: 0.45, icon: '<path d="M7 17a5 5 0 0 1 10 0z"/><path d="M12 7v3M5.3 10.3l1.8 1.8M18.7 10.3l-1.8 1.8M2.5 17h19"/>' },
  { name: "Candle", level: 0.68, icon: '<path d="M12 2.8c1.2 3.3 5.2 5.4 5.2 10.2a5.2 5.2 0 0 1-10.4 0c0-2.5 1.3-4.2 2.6-5.2.2 1.9 1 3.1 2.1 3.6-.6-2.8-.4-5.6.5-8.6z"/>' },
  { name: "Night", level: 0.85, icon: '<path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/>' },
  { name: "Midnight", level: 1, icon: '<path d="M18 15.5A7.5 7.5 0 0 1 8.5 6a7.5 7.5 0 1 0 9.5 9.5z"/><path d="M17.5 2.5l.7 1.6 1.6.7-1.6.7-.7 1.6-.7-1.6-1.6-.7 1.6-.7z"/>' },
];

let S = null;          // state from the backend
let skyLevel = 0;      // what the sky shows, eased toward the effective level
let dragging = false;
const timeFmt = new Intl.DateTimeFormat(undefined, { timeStyle: "short" });

// Sky
const SKY = [
  [0.0, [0.29, 0.55, 0.91], [0.7, 0.84, 0.98]],
  [0.45, [0.33, 0.29, 0.56], [0.98, 0.6, 0.4]],
  [0.75, [0.09, 0.09, 0.24], [0.52, 0.24, 0.26]],
  [1.0, [0.02, 0.03, 0.08], [0.1, 0.07, 0.16]],
];
function drawSky(l) {
  const i = Math.max(1, SKY.findIndex((k) => k[0] >= l));
  const a = SKY[i - 1], b = SKY[i], t = clamp((l - a[0]) / (b[0] - a[0]));
  const mix = (x, y) => rgb(x.map((v, j) => v + (y[j] - v) * t));
  const sky = $("sky"), w = sky.clientWidth, h = sky.clientHeight;
  const flyout = view === "flyout";
  const d = h * (flyout ? 0.22 : 0.16), rest = h * (flyout ? 0.3 : 0.37);
  const setting = smoothstep(0, 0.6, l), rising = smoothstep(0.45, 0.85, l);
  sky.style.setProperty("--top", mix(a[1], b[1]));
  sky.style.setProperty("--bottom", mix(a[2], b[2]));
  sky.style.setProperty("--stars", smoothstep(0.55, 0.95, l));
  sky.style.setProperty("--body", `${d}px`);
  sky.style.setProperty("--sun-color", rgb([1, 0.97 - 0.35 * setting, 0.88 - 0.6 * setting]));
  sky.style.setProperty("--sun-glow", `rgba(255,190,100,${0.35 + 0.4 * setting})`);
  const sun = $("sun"), moon = $("moon");
  sun.style.transform = `translate(${w * (flyout ? 0.8 : 0.76) - d / 2}px, ${rest + h * 0.75 * setting - d / 2}px)`;
  sun.style.opacity = 1 - smoothstep(0.5, 0.7, l);
  const md = d * 0.85;
  moon.style.transform = `translate(${w * (flyout ? 0.7 : 0.22) - md / 2}px, ${rest + h * 0.8 * (1 - rising) - md / 2}px)`;
  moon.style.opacity = rising;
}
function drawStars() {
  const c = $("stars"), r = devicePixelRatio || 1;
  c.width = c.clientWidth * r; c.height = c.clientHeight * r;
  const ctx = c.getContext("2d");
  ctx.fillStyle = "rgba(255,255,255,0.85)";
  let seed = 7n;
  for (let i = 0; i < 70; i++) {
    seed = (seed * 6364136223846793005n + 1442695040888963407n) & 0xffffffffffffffffn;
    const x = Number((seed >> 40n) & 0xffffn) / 65535 * c.width;
    const y = Number((seed >> 20n) & 0xffffn) / 65535 * c.height * 0.7;
    const s = (0.5 + Number((seed >> 8n) & 0xffn) / 255 * 1.1) * r;
    ctx.beginPath(); ctx.arc(x, y, s / 2, 0, Math.PI * 2); ctx.fill();
  }
}
let skyFrom = 0, skyTo = 0, skyStart = 0, skyRaf = 0;
function easeSky(to, instant) {
  if (instant) { cancelAnimationFrame(skyRaf); skyRaf = 0; skyLevel = to; drawSky(to); return; }
  if (Math.abs(to - skyTo) < 1e-4 && skyRaf) return;
  skyFrom = skyLevel; skyTo = to; skyStart = performance.now();
  cancelAnimationFrame(skyRaf);
  const step = (now) => {
    const p = Math.min(1, (now - skyStart) / 1000);
    skyLevel = skyFrom + (skyTo - skyFrom) * smoothstep(0, 1, p);
    drawSky(skyLevel);
    skyRaf = p < 1 ? requestAnimationFrame(step) : 0;
  };
  skyRaf = requestAnimationFrame(step);
}

// Dials
function dial(el, get, set, detents) {
  let pending = null, raf = 0;
  const send = (v) => { pending = v; if (!raf) raf = requestAnimationFrame(() => { raf = 0; set(pending, false); }); };
  const at = (e) => {
    const r = el.getBoundingClientRect();
    let x = clamp((e.clientX - r.left - 12) / (r.width - 24));
    const snap = detents.find((d) => Math.abs(d - x) < 0.012);
    return snap ?? x;
  };
  el.addEventListener("pointerdown", (e) => { dragging = true; el.setPointerCapture(e.pointerId); send(at(e)); });
  el.addEventListener("pointermove", (e) => { if (el.hasPointerCapture(e.pointerId)) send(at(e)); });
  el.addEventListener("pointerup", () => { dragging = false; });
  el.addEventListener("keydown", (e) => {
    const step = { ArrowRight: 0.05, ArrowUp: 0.05, ArrowLeft: -0.05, ArrowDown: -0.05 }[e.key];
    if (step) { e.preventDefault(); set(clamp(get() + step), true); }
  });
}

// Render
function render() {
  if (!S) return;
  const level = S.enabled ? S.level : 0;
  $("title").textContent = S.enabled ? `${S.kelvin.toLocaleString()}K` : "Off";
  $("subtitle").textContent = !S.enabled ? "Your screen is untouched"
    : S.brightness < 0.995 ? `${S.preset} · ${Math.round(S.brightness * 100)}% brightness` : S.preset;
  $("power").checked = $("power-flyout").checked = S.enabled;
  $("follow").checked = $("follow2").checked = S.followSun;
  $("autostart").checked = S.autostart;
  $("next").textContent = `${S.nextWarms ? "Warms" : "Cools"} at ${timeFmt.format(S.nextAt)}`;
  $("place").textContent = S.placeName;
  $("today").textContent = `Sunrise ${timeFmt.format(S.sunrise)} · Sunset ${timeFmt.format(S.sunset)}`;
  $("about").textContent = "github.com/michael-berardi/goodnight";
  $("version").textContent = `Version ${S.version}`;
  $("autoupdate").checked = S.autoUpdate;
  $("update").textContent = S.update ? `Install ${S.update}` : "Check now";
  $("update-banner").hidden = !S.update;
  $("update-banner").textContent = S.update ? `Good Night ${S.update} is ready · Install` : "";
  $("error").hidden = !S.error;
  $("error").textContent = S.error || "";

  const w = $("warmth");
  w.style.setProperty("--x", S.level);
  w.style.setProperty("--knob", tint(S.level));
  w.setAttribute("aria-valuenow", Math.round(S.level * 100));
  const b = $("brightness"), white = tint(Math.min(level, 0.3));
  b.style.setProperty("--x", S.brightness);
  b.style.setProperty("--track", `linear-gradient(90deg, #000, ${white})`);
  b.style.setProperty("--knob", white);
  b.setAttribute("aria-valuenow", Math.round(S.brightness * 100));

  document.querySelectorAll(".preset").forEach((p, i) => {
    p.classList.toggle("on", S.enabled && Math.abs(S.level - PRESETS[i].level) < 0.01);
  });
  easeSky(level, dragging);
}

function build() {
  $("warmth").style.setProperty("--track", `linear-gradient(90deg, ${Array.from({ length: 9 }, (_, i) => tint(i / 8)).join(",")})`);
  const box = $("presets");
  PRESETS.forEach((p) => {
    const b = document.createElement("button");
    b.className = "preset";
    b.title = `${p.name} · ${Math.round(kelvin(p.level) / 50) * 50}K`;
    b.style.setProperty("--c", tint(p.level));
    b.style.setProperty("--on-ink", p.level > 0.6 ? "#fff" : "rgba(0,0,0,0.8)");
    b.innerHTML = `<span class="dot"><svg viewBox="0 0 24 24">${p.icon}</svg></span>${p.name}`;
    b.onclick = () => invoke("set_level", { level: p.level, animated: true });
    box.appendChild(b);
  });
  dial($("warmth"), () => S.level, (v, animated) => { S.level = v; S.enabled = true; render(); invoke("set_level", { level: v, animated }); }, PRESETS.map((p) => p.level));
  dial($("brightness"), () => S.brightness, (v, animated) => { S.brightness = v; S.enabled = true; render(); invoke("set_brightness", { value: v, animated }); }, [1]);
  const power = (e) => invoke("set_enabled", { on: e.target.checked });
  $("power").onchange = power;
  $("power-flyout").onchange = power;
  $("follow").onchange = $("follow2").onchange = (e) => invoke("set_follow_sun", { on: e.target.checked });
  $("autostart").onchange = (e) => invoke("set_autostart", { on: e.target.checked }).catch((err) => { S.error = String(err); render(); });
  $("autoupdate").onchange = (e) => invoke("set_auto_update", { on: e.target.checked });
  const install = () => { $("update").textContent = "Installing…"; invoke("install_update").catch((err) => { S.error = String(err); render(); }); };
  $("update-banner").onclick = install;
  $("update").onclick = () => {
    if (S.update) return install();
    $("update").textContent = "Checking…";
    invoke("check_update")
      .then((v) => { $("update").textContent = v ? `Install ${v}` : "Up to date"; })
      .catch((err) => { $("update").textContent = "Check now"; S.error = String(err); render(); });
  };
  $("gear").onclick = () => { $("sheet").hidden = !$("sheet").hidden; };
  $("done").onclick = () => { $("sheet").hidden = true; };
  $("minimize").onclick = () => win.minimize();
  $("close").onclick = () => win.close();
  $("open").onclick = () => invoke("open_main");
  $("quit").onclick = () => invoke("quit");
  document.addEventListener("keydown", (e) => { if (e.key === "Escape") view === "flyout" ? win.close() : ($("sheet").hidden = true); });
  drawStars();
  if (new URLSearchParams(location.search).has("sheet")) $("sheet").hidden = false;
}

build();
listen("state", (e) => { if (!dragging) { S = e.payload; render(); } });
invoke("state").then((s) => {
  S = s; skyLevel = s.enabled ? s.level : 0; drawSky(skyLevel); render();
  // The flyout is as tall as its content.
  if (view === "flyout") {
    const card = $("card");
    win.setSize(new window.__TAURI__.dpi.LogicalSize(320, Math.ceil(card.offsetTop + card.offsetHeight)));
  }
});
