"use strict";

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

const DEVICES = [
  { val: "unknown", label: "web" },
  { val: "desktop", label: "desktop" },
  { val: "iphone", label: "iphone" },
  { val: "macos", label: "macos" },
  { val: "android", label: "android" },
  { val: "windows", label: "windows" },
  { val: "linux", label: "linux" },
];

function escapeHtml(s) {
  return s ? s.replace(/[&<>'"]/g, (t) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" }[t] || t)) : "";
}

function fmtDate(v) {
  const n = typeof v === "number" ? v : (/^\d+$/.test(String(v).trim()) ? parseInt(v, 10) : NaN);
  if (!isFinite(n)) return String(v ?? "");
  const d = new Date(n * 1000);
  const p = (x) => String(x).padStart(2, "0");
  return `${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

function showToast(m, t = "success") {
  const c = document.getElementById("toast-container");
  const e = document.createElement("div");
  e.className = `toast ${t}`;
  e.textContent = m;
  c.appendChild(e);
  requestAnimationFrame(() => e.classList.add("show"));
  setTimeout(() => {
    e.classList.remove("show");
    setTimeout(() => e.remove(), 300);
  }, 2600);
}

function copyId(id) {
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(id).then(
      () => showToast("copied"),
      () => showToast("copy failed", "error")
    );
  } else {
    showToast("clipboard unavailable (needs https)", "error");
  }
}

let currentSettings = { contacts: [], emojis: [] };

async function loadSettings() {
  try {
    const r = await fetch("/api/settings");
    if (!r.ok) return;
    currentSettings = await r.json();
    renderSettings();
  } catch (e) {
    showToast("settings error", "error");
  }
}

function renderSettings() {
  const c = document.getElementById("contactsList");
  c.innerHTML = "";
  (currentSettings.contacts || []).forEach((i, x) => {
    c.insertAdjacentHTML(
      "beforeend",
      `<div class="item" data-index="${x}"><input type="text" class="con-icon" placeholder="icon (empty = auto)" value="${escapeHtml(i.icon || "")}"><input type="text" class="con-text" placeholder="text" value="${escapeHtml(i.text)}"><input type="text" class="con-link" placeholder="link" value="${escapeHtml(i.link)}"><button class="btn small danger" data-action="remove-contact" data-index="${x}">remove</button></div>`
    );
  });
  if (!(currentSettings.contacts || []).length) c.innerHTML = '<div class="empty">empty</div>';

  const e = document.getElementById("emojisList");
  e.innerHTML = "";
  (currentSettings.emojis || []).forEach((i, x) => {
    e.insertAdjacentHTML(
      "beforeend",
      `<div class="item" data-index="${x}"><input type="text" class="emo-code" placeholder="code" value="${escapeHtml(i.code)}"><input type="text" class="emo-url" placeholder="image url" value="${escapeHtml(i.url)}"><img class="emoji-preview" src="${escapeHtml(i.url)}" alt=""><button class="btn small danger" data-action="remove-emoji" data-index="${x}">remove</button></div>`
    );
  });
  if (!(currentSettings.emojis || []).length) e.innerHTML = '<div class="empty">empty</div>';
}

function syncEmojiPreview(input) {
  const img = input.parentElement.querySelector("img.emoji-preview");
  if (img) img.src = input.value;
}

function collectSettings() {
  currentSettings = {
    contacts: Array.from(document.querySelectorAll("#contactsList .item")).map((i) => ({ icon: i.querySelector(".con-icon").value, text: i.querySelector(".con-text").value, link: i.querySelector(".con-link").value })),
    emojis: Array.from(document.querySelectorAll("#emojisList .item")).map((i) => ({ code: i.querySelector(".emo-code").value, url: i.querySelector(".emo-url").value })),
  };
}

function addContact() {
  collectSettings();
  currentSettings.contacts.push({ icon: "", text: "", link: "" });
  renderSettings();
}
function removeContact(i) {
  collectSettings();
  currentSettings.contacts.splice(i, 1);
  renderSettings();
}
function addEmoji() {
  collectSettings();
  currentSettings.emojis.push({ code: "", url: "" });
  renderSettings();
}
function removeEmoji(i) {
  collectSettings();
  currentSettings.emojis.splice(i, 1);
  renderSettings();
}

async function saveSettings() {
  collectSettings();
  try {
    const r = await fetch("/api/admin/settings", { method: "PUT", headers: { "Content-Type": "application/json", "X-Requested-With": "XMLHttpRequest" }, body: JSON.stringify({ settings: currentSettings }) });
    if (r.ok) showToast("saved");
    else showToast("error", "error");
  } catch (e) {
    showToast("network error", "error");
  }
}

function deviceOptions(selected) {
  return DEVICES.map((d) => `<option value="${d.val}"${d.val === selected ? " selected" : ""}>${d.label}</option>`).join("");
}

async function loadComments() {
  try {
    const r = await fetch("/api/comments?profanity=show");
    const d = await r.json();
    const b = document.getElementById("commentsList");
    b.innerHTML = "";
    if (!d.length) return (b.innerHTML = '<div class="empty">no comments</div>');
    d.forEach((c) => {
      const el = document.createElement("div");
      el.className = "comment";
      el.innerHTML = `<div class="id-cell" title="copy id" data-action="copy-id" data-id="${escapeHtml(c.id)}">${escapeHtml(c.id)} · ${fmtDate(c.date)}</div><div class="row"><input type="text" id="name-${escapeHtml(c.id)}" value="${escapeHtml(c.name)}" placeholder="name"><label class="checkbox-row"><input type="checkbox" id="admin-${escapeHtml(c.id)}"${c.is_admin ? " checked" : ""}>admin</label><input type="number" id="score-${escapeHtml(c.id)}" value="${c.score || 0}"><select id="dev-${escapeHtml(c.id)}">${deviceOptions(c.device)}</select></div><textarea id="text-${escapeHtml(c.id)}">${escapeHtml(c.text)}</textarea><div class="row-end"><button class="btn small" data-action="save-comment" data-id="${escapeHtml(c.id)}">save</button><button class="btn small danger" data-action="del-comment" data-id="${escapeHtml(c.id)}">delete</button></div>`;
      b.appendChild(el);
    });
  } catch (e) {
    showToast("error", "error");
  }
}

async function saveComment(i) {
  try {
    const r = await fetch("/api/admin/comments/" + i, {
      method: "PUT",
      headers: { "Content-Type": "application/json", "X-Requested-With": "XMLHttpRequest" },
      body: JSON.stringify({
        name: document.getElementById(`name-${i}`).value,
        text: document.getElementById(`text-${i}`).value,
        is_admin: document.getElementById(`admin-${i}`).checked,
        score: parseInt(document.getElementById(`score-${i}`).value) || 0,
        device: document.getElementById(`dev-${i}`).value,
      }),
    });
    if (r.ok) showToast("saved");
    else showToast("error", "error");
  } catch (e) {
    showToast("network error", "error");
  }
}

async function delComment(i) {
  if (!confirm("delete?")) return;
  try {
    const r = await fetch("/api/admin/comments/" + i, { method: "DELETE", headers: { "X-Requested-With": "XMLHttpRequest" } });
    if (r.ok) {
      showToast("deleted");
      loadComments();
    } else showToast("error", "error");
  } catch (e) {
    showToast("network error", "error");
  }
}

async function loadBlog() {
  try {
    const r = await fetch("/api/blog");
    const d = await r.json();
    const b = document.getElementById("blogList");
    b.innerHTML = "";
    if (!d.length) return (b.innerHTML = '<div class="empty">no posts</div>');
    d.forEach((p) => {
      const el = document.createElement("div");
      el.className = "post";
      el.innerHTML = `<div class="id-cell" title="copy id" data-action="copy-id" data-id="${escapeHtml(p.id)}">${escapeHtml(p.id)} · ${escapeHtml(fmtDate(p.date))}</div><input type="text" id="btitle-${escapeHtml(p.id)}" value="${escapeHtml(p.title)}" placeholder="title"><textarea id="btext-${escapeHtml(p.id)}">${escapeHtml(p.text)}</textarea><div class="row-end"><button class="btn small" data-action="save-post" data-id="${escapeHtml(p.id)}">save</button><button class="btn small danger" data-action="del-post" data-id="${escapeHtml(p.id)}">delete</button></div>`;
      b.appendChild(el);
    });
  } catch (e) {
    showToast("error", "error");
  }
}

async function createPost() {
  const t = document.getElementById("blog-title");
  const x = document.getElementById("blog-text");
  if (!t.value.trim() || !x.value.trim()) return showToast("fill all fields", "error");
  try {
    const r = await fetch("/api/admin/blog", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Requested-With": "XMLHttpRequest" },
      body: JSON.stringify({ title: t.value, text: x.value, date: String(Math.floor(Date.now() / 1000)) }),
    });
    if (r.ok) {
      t.value = "";
      x.value = "";
      showToast("published");
      loadBlog();
    } else showToast("error", "error");
  } catch (e) {
    showToast("network error", "error");
  }
}

async function savePost(i) {
  try {
    const r = await fetch("/api/admin/blog/" + i, {
      method: "PUT",
      headers: { "Content-Type": "application/json", "X-Requested-With": "XMLHttpRequest" },
      body: JSON.stringify({ title: document.getElementById(`btitle-${i}`).value, text: document.getElementById(`btext-${i}`).value }),
    });
    if (r.ok) showToast("updated");
    else showToast("error", "error");
  } catch (e) {
    showToast("network error", "error");
  }
}

async function delPost(i) {
  if (!confirm("delete?")) return;
  try {
    const r = await fetch("/api/admin/blog/" + i, { method: "DELETE", headers: { "X-Requested-With": "XMLHttpRequest" } });
    if (r.ok) {
      showToast("deleted");
      loadBlog();
    } else showToast("error", "error");
  } catch (e) {
    showToast("network error", "error");
  }
}

document.addEventListener("click", (ev) => {
  const el = ev.target.closest("[data-action]");
  if (!el) return;
  switch (el.dataset.action) {
    case "add-contact": addContact(); break;
    case "add-emoji": addEmoji(); break;
    case "save-settings": saveSettings(); break;
    case "reload-comments": loadComments(); break;
    case "create-post": createPost(); break;
    case "remove-contact": removeContact(Number(el.dataset.index)); break;
    case "remove-emoji": removeEmoji(Number(el.dataset.index)); break;
    case "copy-id": copyId(el.dataset.id); break;
    case "save-comment": saveComment(el.dataset.id); break;
    case "del-comment": delComment(el.dataset.id); break;
    case "save-post": savePost(el.dataset.id); break;
    case "del-post": delPost(el.dataset.id); break;
  }
});

document.getElementById("emojisList").addEventListener("input", (ev) => {
  if (ev.target.classList.contains("emo-url")) syncEmojiPreview(ev.target);
});

window.addEventListener("load", () => {
  loadSettings();
  loadComments();
  loadBlog();
});
