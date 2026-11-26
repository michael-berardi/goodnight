"use strict";
// Run with: node --test windows/ui/tests/updates.test.js
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const ui = require("../updates.js");

const html = fs.readFileSync(path.join(__dirname, "..", "index.html"), "utf8");
const appJs = fs.readFileSync(path.join(__dirname, "..", "app.js"), "utf8");

test("the prompt names both versions", () => {
  assert.equal(ui.notice("Good Night", "0.4.0", "0.3.0"), "Good Night 0.4.0 is available — you have 0.3.0.");
  const state = { version: "0.3.0", update: { version: "0.4.0", notes: "" } };
  assert.equal(ui.shownUpdate(state).notice, "Good Night 0.4.0 is available — you have 0.3.0.");
  assert.equal(ui.bannerText(state.update), "Good Night 0.4.0 is available");
});

test("only a strictly newer stable release is shown", () => {
  assert.equal(ui.isNewer("0.4.0", "0.3.0"), true);
  assert.equal(ui.isNewer("0.3.1", "0.3.0"), true);
  assert.equal(ui.isNewer("0.10.0", "0.9.0"), true);
  assert.equal(ui.isNewer("0.3.0", "0.3.0"), false);
  assert.equal(ui.isNewer("0.2.9", "0.3.0"), false);
  assert.equal(ui.isNewer("0.4.0-beta.1", "0.3.0"), false);
  assert.equal(ui.isNewer("v0.4.0", "0.3.0"), false);
  assert.equal(ui.isNewer("garbage", "0.3.0"), false);
  assert.equal(ui.shownUpdate({ version: "0.3.0", update: null }), null);
  assert.equal(ui.shownUpdate({ version: "0.3.0", update: { version: "0.3.0" } }), null);
});

test("release notes are short plain text", () => {
  assert.equal(ui.excerpt(null), "");
  assert.equal(ui.excerpt("### Added\n\n- One.\n* Two."), "Added\n• One.\n• Two.");
  const many = Array.from({ length: 20 }, (_, i) => `- line ${i}`).join("\n");
  assert.equal(ui.excerpt(many).split("\n").length, 8);
  const long = ui.excerpt("x".repeat(2000));
  assert.equal(long.length, 600);
  assert.ok(long.endsWith("…"));
  // Notes are set with textContent, never parsed as HTML.
  assert.match(appJs, /\$\("update-notes"\)\.textContent = /);
  assert.doesNotMatch(appJs, /update-notes"\)\.innerHTML/);
});

test("automatic install is off unless chosen, and needs automatic checks", () => {
  assert.equal(ui.autoChecked(null), false);
  assert.equal(ui.autoChecked({}), false);
  assert.equal(ui.autoChecked({ installUpdatesAutomatically: true }), true);
  assert.doesNotMatch(html, /id="(update-auto|installauto)"[^>]*checked/);
  assert.match(appJs, /\$\("installauto"\)\.disabled = !S\.autoUpdate;/);
});

test("a failed check says so next to the version, with the cause as the tooltip", () => {
  assert.equal(ui.versionLabel({ version: "0.3.0" }), "Version 0.3.0");
  assert.equal(ui.versionLabel({ version: "0.3.0", updateError: "timed out" }), "Version 0.3.0 · last check failed");
  assert.match(appJs, /\$\("version"\)\.title = S\.updateError \|\| "";/);
});

test("Settings › Updates: one toggle per row, then version left and Check Now right on one row", () => {
  const group = html.match(/<div class="group">\s*<div class="row"><span>Check for updates automatically[\s\S]*?<\/div>\s*<\/div>/)[0];
  const rows = group.match(/<div class="row">.*<\/div>/g);
  assert.equal(rows.length, 3);
  assert.match(rows[0], /Check for updates automatically<\/span><label class="switch"><input type="checkbox" id="autoupdate">/);
  assert.match(rows[1], /Install updates automatically<\/span><label class="switch"><input type="checkbox" id="installauto">/);
  assert.equal(rows[2], '<div class="row"><span class="value" id="version"></span><button class="link" id="update">Check Now</button></div>');
});

test("the prompt: notes, opt-in checkbox, then actions on one trailing row", () => {
  const prompt = html.match(/<section class="update-prompt"[\s\S]*?<\/section>/)[0];
  const order = ["update-title", "update-notes", "update-auto", "update-skip", "update-later", "update-now"].map((id) => prompt.indexOf(`id="${id}"`));
  assert.ok(order.every((i, k) => i > 0 && (k === 0 || i > order[k - 1])), String(order));
  assert.match(prompt, /<button class="link" id="update-skip">Skip This Version<\/button>/);
  assert.match(prompt, /<button class="link" id="update-later">Remind Me Later<\/button>/);
  assert.match(prompt, /<button class="primary" id="update-now">Install Update<\/button>/);
  assert.match(prompt, /Automatically download and install updates in the future/);
  // The window shows the prompt; the tray flyout shows only the banner, whose Review opens the window.
  assert.match(appJs, /\$\("update-prompt"\)\.hidden = !update \|\| view === "flyout"/);
  assert.match(appJs, /\$\("review-update"\)\.onclick = \(\) => invoke\("open_main"\)/);
});
