"use strict";
// Pure presentation rules for the update UI. No network, no platform APIs: the signed feed
// and the install are handled by tauri-plugin-updater in the shell.
(function (root, factory) {
  const api = factory();
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.UpdatesUI = api;
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  const notice = (app, version, current) => `${app} ${version} is available — you have ${current}.`;

  // Release notes arrive as short Markdown. They are shown as text (never HTML): headings lose
  // their marks, list items become bullets, and only the first lines are kept.
  function excerpt(notes) {
    const lines = String(notes || "").split(/\r?\n/)
      .map((l) => l.trim().replace(/^#+\s*/, "").replace(/^[-*]\s+/, "• "))
      .filter(Boolean);
    const short = lines.slice(0, 8).join("\n");
    return short.length > 600 ? `${short.slice(0, 599).trimEnd()}…` : short;
  }

  // Stable x.y.z only: prereleases and unparseable versions are never offered.
  function parseVersion(s) {
    const p = String(s || "").trim().split(".");
    if (p.length !== 3 || p.some((x) => !/^\d+$/.test(x))) return null;
    return p.map(Number);
  }
  function isNewer(candidate, current) {
    const a = parseVersion(candidate), b = parseVersion(current);
    if (!a || !b) return false;
    for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] > b[i];
    return false;
  }

  // The release the prompt shows, if the shell offered a newer one.
  function shownUpdate(state) {
    const u = state && state.update;
    if (!u || !isNewer(u.version, state.version)) return null;
    return { ...u, notice: u.notice || notice("Good Night", u.version, state.version) };
  }

  const bannerText = (update) => `Good Night ${update.version} is available`;

  // "Version 0.3.0" on the left of the Check Now row; a failed check says so, the cause is the tooltip.
  const versionLabel = (state) => `Version ${state.version}${state.updateError ? " · last check failed" : ""}`;

  // An absent preference means the opt-in is off.
  const autoChecked = (state) => !!(state && state.installUpdatesAutomatically);

  return { notice, excerpt, parseVersion, isNewer, shownUpdate, bannerText, versionLabel, autoChecked };
});
