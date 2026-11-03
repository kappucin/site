"use strict";

const REDUCE_MOTION = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

function parseUnix(v) {
  const s = String(v ?? "").trim();
  if (!/^\d+$/.test(s)) return null;
  const n = parseInt(s, 10);
  return isFinite(n) ? n : null;
}

function pad2(x) {
  return String(x).padStart(2, "0");
}

function fmtDate(v) {
  const n = parseUnix(v);
  if (n === null) return String(v ?? "");
  const d = new Date(n * 1000);
  return `${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

function fmtDateShort(v) {
  const n = parseUnix(v);
  if (n === null) return String(v ?? "");
  const d = new Date(n * 1000);
  return `${d.getDate()} ${MONTHS[d.getMonth()]}`;
}

function applyDates(root) {
  (root || document).querySelectorAll("[data-date]").forEach((el) => {
    const v = el.getAttribute("data-date");
    el.textContent = el.hasAttribute("data-date-short") ? fmtDateShort(v) : fmtDate(v);
  });
}

if (window.htmx) {
  htmx.config.selfRequestsOnly = true;
  htmx.config.allowEval = false;
}

applyTheme(getThemeMode());

const mainTopbar = document.getElementById("mainTopbar");
const mainContainer = document.getElementById("mainContainer");

function updateTopbarShadow() {
  mainTopbar.classList.toggle("scrolled", window.scrollY > 4);
}

function syncScrollLock() {
  const root = document.documentElement;
  root.classList.remove("no-scroll");
  const totalContentHeight = mainContainer.getBoundingClientRect().bottom + window.scrollY;
  if (totalContentHeight <= window.innerHeight + 1) {
    window.scrollTo(0, 0);
    root.classList.add("no-scroll");
  }
  updateTopbarShadow();
}

window.addEventListener("scroll", updateTopbarShadow, { passive: true });
window.addEventListener("resize", syncScrollLock, { passive: true });
if (window.ResizeObserver) {
  new ResizeObserver(syncScrollLock).observe(mainContainer);
}


function samplePolarShape(fn, segments = 36) {
  const pts = [];
  for (let i = 0; i < segments; i++) {
    const theta = (i * 2 * Math.PI) / segments - Math.PI / 2;
    pts.push(fn(theta));
  }
  let d = "";
  for (let i = 0; i < segments; i++) {
    const p0 = pts[(i - 1 + segments) % segments];
    const p1 = pts[i];
    const p2 = pts[(i + 1) % segments];
    const mid1 = [(p0[0] + p1[0]) / 2, (p0[1] + p1[1]) / 2];
    const mid2 = [(p1[0] + p2[0]) / 2, (p1[1] + p2[1]) / 2];
    if (i === 0) d += `M ${mid1[0].toFixed(2)} ${mid1[1].toFixed(2)} `;
    d += `Q ${p1[0].toFixed(2)} ${p1[1].toFixed(2)} ${mid2[0].toFixed(2)} ${mid2[1].toFixed(2)} `;
  }
  return d + "Z";
}

const MD3E_ALL_SHAPES = [
  samplePolarShape((t) => {
    const r = 40 - 7.5 * Math.cos(4 * t);
    return [50 + r * Math.cos(t), 50 + r * Math.sin(t)];
  }),
  samplePolarShape((t) => {
    const a = 48, b = 31, rot = -0.68, x0 = a * Math.cos(t), y0 = b * Math.sin(t);
    return [50 + x0 * Math.cos(rot) - y0 * Math.sin(rot), 50 + x0 * Math.sin(rot) + y0 * Math.cos(rot)];
  }),
  samplePolarShape((t) => {
    const r = 42.5 + 4.8 * Math.cos(12 * t);
    return [50 + r * Math.cos(t), 50 + r * Math.sin(t)];
  }),
  samplePolarShape((t) => {
    const r = 41 + 4 * Math.sin(2 * t - 0.5);
    return [50 + r * Math.cos(t) + 5 * Math.sin(t), 50 + r * Math.sin(t)];
  }),
  samplePolarShape((t) => {
    const r = 41 + 6 * Math.cos(8 * t);
    return [50 + r * Math.cos(t), 50 + r * Math.sin(t)];
  }),
  samplePolarShape((t) => {
    const r = 41 + 6.5 * Math.cos(4 * t);
    return [50 + r * Math.cos(t), 50 + r * Math.sin(t)];
  }),
  samplePolarShape((t) => {
    const r = 42 + 5 * Math.cos(2 * t) - 2.5 * Math.cos(6 * t);
    return [50 + r * Math.cos(t), 50 + r * Math.sin(t)];
  }),
  samplePolarShape((t) => {
    const x = 50 + 42 * Math.cos(t);
    let y = 50 + 44 * Math.sin(t);
    if (Math.sin(t) > 0.35) y = 50 + 44 * (0.35 + (Math.sin(t) - 0.35) * 0.45);
    return [x, y];
  }),
];

let currentShapeIdx = 0;
const heroPathEl = document.getElementById("heroMorphPath");

function updateHeroMorph() {
  if (document.hidden) return;
  heroPathEl.setAttribute("d", MD3E_ALL_SHAPES[currentShapeIdx]);
  currentShapeIdx = (currentShapeIdx + 1) % MD3E_ALL_SHAPES.length;
}

let squeezeTimer = null;
const nameWidgetTile = document.getElementById("nameWidgetTile");
function triggerNameSqueeze() {
  nameWidgetTile.classList.add("squeezing");
  scrambleNickArrows();
  clearTimeout(squeezeTimer);
  squeezeTimer = setTimeout(() => nameWidgetTile.classList.remove("squeezing"), 210);
}

const ARROW_CHARS = ["<", ">", "\u00ab", "\u00bb", "\u2039", "\u203a"];
function initGeneratedPlaceholders() {
  document.querySelectorAll(".nick-arrows").forEach((el) => {
    if (el.children.length) return;
    for (let i = 0; i < 28; i++) el.appendChild(document.createElement("span"));
  });
  const ruler = document.querySelector(".bday-ruler");
  if (!ruler || ruler.children.length) return;
  for (let i = 0; i < 5; i++) {
    const tick = document.createElement("i");
    if (i === 3) tick.className = "active-tick";
    ruler.appendChild(tick);
  }
}

initGeneratedPlaceholders();

const nickArrowSpans = document.querySelectorAll(".nick-arrows span");
let arrowTimer = null;

function randomArrowChar() {
  return ARROW_CHARS[Math.floor(Math.random() * ARROW_CHARS.length)];
}

function randomizeNickArrows() {
  nickArrowSpans.forEach((a) => (a.textContent = randomArrowChar()));
}

function scrambleNickArrows() {
  if (REDUCE_MOTION) return;
  clearInterval(arrowTimer);
  let ticks = 0;
  arrowTimer = setInterval(() => {
    ticks++;
    nickArrowSpans.forEach((a) => {
      if (Math.random() < 0.6) a.textContent = randomArrowChar();
    });
    if (ticks >= 15) {
      clearInterval(arrowTimer);
      nickArrowSpans.forEach((a) => (a.textContent = randomArrowChar()));
    }
  }, 65);
}

randomizeNickArrows();

const POW_WORKER_SRC = `
self.onmessage = async (e) => {
  const { prefix, n } = e.data;
  const enc = new TextEncoder();
  const pfx = enc.encode(prefix);
  let nonce = 0;
  const BATCH = 2000;
  for (;;) {
    const ps = [];
    for (let i = 0; i < BATCH; i++) {
      const ns = String(nonce + i);
      const data = new Uint8Array(pfx.length + ns.length);
      data.set(pfx);
      data.set(enc.encode(ns), pfx.length);
      ps.push(crypto.subtle.digest('SHA-256', data));
    }
    const hs = await Promise.all(ps);
    for (let i = 0; i < hs.length; i++) {
      const d = new Uint8Array(hs[i]);
      let bits = 0, done = false;
      for (let k = 0; k < 32 && !done; k++) {
        const b = d[k];
        if (b === 0) { bits += 8; continue; }
        for (let j = 7; j >= 0; j--) { if (b & (1 << j)) { done = true; break; } bits++; }
      }
      if (bits >= n) { self.postMessage(String(nonce + i)); return; }
    }
    nonce += BATCH;
  }
};
`;

let powPromise = null;
let powReadyAt = 0;

function fetchCaptcha() {
  const slot = document.getElementById("captchaSlot");
  if (!slot) return Promise.resolve();
  powPromise = htmx
    .ajax("GET", "/fragments/captcha", { target: "#captchaSlot", swap: "innerHTML" })
    .then(() => {
      const prefixInput = slot.querySelector('[name="pow_prefix"]');
      if (!prefixInput) return;
      const prefix = prefixInput.value;
      const n = parseInt(slot.querySelector('[name="pow_n"]').value, 10);
      if (!prefix || !n) return;
      const worker = new Worker(
        URL.createObjectURL(new Blob([POW_WORKER_SRC], { type: "text/javascript" }))
      );
      return new Promise((resolve) => {
        worker.onmessage = (e) => {
          slot.querySelector('[name="pow_nonce"]').value = e.data;
          worker.terminate();
          resolve();
        };
        worker.postMessage({ prefix, n });
      });
    })
    .then(() => {
      powReadyAt = Date.now();
    })
    .catch(() => {
      powPromise = null;
    });
  return powPromise;
}

function haptic(pattern) {
  try {
    if (navigator.vibrate) navigator.vibrate(pattern);
  } catch (e) {}
}

const TAB_PATHS = { home: "/", blog: "/blog", comments: "/comments" };

function pathToTab(p) {
  if (p === "/blog" || p.startsWith("/blog/")) return "blog";
  if (p === "/comments") return "comments";
  if (/^\/post\/[^/]+/.test(p)) return "blog";
  return "home";
}

let restoringRoute = false;

function switchTab(tabId, push = true) {
  document.querySelectorAll(".tab-view").forEach((el) => el.classList.remove("active"));
  document.getElementById(`tab-${tabId}`).classList.add("active");
  document.querySelectorAll(".nav-btn").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.tab === tabId);
  });
  syncScrollLock();
  if (push && !restoringRoute) {
    const url = TAB_PATHS[tabId] || "/";
    if (location.pathname !== url) history.pushState({ tab: tabId }, "", url);
  }
}

window.addEventListener("popstate", () => {
  const p = location.pathname;
  restoringRoute = true;
  const unstick = setTimeout(() => {
    restoringRoute = false;
  }, 3000);
  const consume = () => {
    clearTimeout(unstick);
    restoringRoute = false;
  };
  if (/^\/post\/([^/]+)/.test(p)) {
    const id = decodeURIComponent(p.match(/^\/post\/([^/]+)/)[1]);
    switchTab("blog", false);
    if (window.htmx) {
      htmx.ajax("GET", `/fragments/post/${encodeURIComponent(id)}`, {
        target: "#blogViewSlot",
        swap: "innerHTML",
      });
    } else consume();
    return;
  }
  if (pathToTab(p) === "blog") {
    if (document.getElementById("blogDetailView") && window.htmx) {
      htmx.ajax("GET", "/fragments/blog", { target: "#blogViewSlot", swap: "innerHTML" });
    } else consume();
    switchTab("blog", false);
    return;
  }
  consume();
  switchTab(pathToTab(p), false);
});

function openModal(id) {
  document.getElementById(id).classList.add("open");
  if (id === "newCommentModal") fetchCaptcha();
}

function closeModal(id) {
  document.getElementById(id).classList.remove("open");
}

function closePostDetail() {
  if (!window.htmx) return;
  htmx.ajax("GET", "/fragments/blog", {
    target: "#blogViewSlot",
    swap: "innerHTML",
  });
}

function getThemeMode() {
  try {
    const t = localStorage.getItem("kppcnTheme");
    return t === "light" || t === "dark" || t === "system" ? t : "system";
  } catch (e) {
    return "system";
  }
}

function resolveTheme(mode) {
  if (mode !== "system") return mode;
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function applyTheme(mode) {
  document.documentElement.setAttribute("data-theme", resolveTheme(mode));
}

function selectThemeOption(mode) {
  document.querySelectorAll("#themeSegmentedList .md3e-seg-item").forEach((el) => {
    el.classList.toggle("selected", el.dataset.themeOpt === mode);
  });
  try {
    if (mode === "system") localStorage.removeItem("kppcnTheme");
    else localStorage.setItem("kppcnTheme", mode);
  } catch (e) {}
  applyTheme(mode);
  haptic(8);
}

function syncThemeSegment() {
  const mode = getThemeMode();
  document.querySelectorAll("#themeSegmentedList .md3e-seg-item").forEach((el) => {
    el.classList.toggle("selected", el.dataset.themeOpt === mode);
  });
}

syncThemeSegment();

window.matchMedia("(prefers-color-scheme: light)").addEventListener("change", () => {
  if (getThemeMode() === "system") applyTheme("system");
});

let sortOrder = "new";
let hideProfanity = true;

function commentParams(extra) {
  const q = document.getElementById("commentSearchInput")?.value || "";
  return Object.assign(
    { sort: sortOrder, profanity: hideProfanity ? "hide" : "show", q },
    extra || {}
  );
}

function refreshComments(page = 1) {
  if (!window.htmx) return;
  htmx.ajax("GET", "/fragments/comments", {
    target: "#commentsFeedContainer",
    swap: "innerHTML",
    values: commentParams({ page }),
  });
}

function currentPageNum() {
  const txt = document.getElementById("pageIndicator")?.textContent || "";
  const n = parseInt(txt, 10);
  return isFinite(n) && n > 0 ? n : 1;
}

function handlePageNav(btn) {
  const page = parseInt(btn.dataset.page, 10);
  if (!isFinite(page) || page < 1 || !window.htmx) return;
  htmx
    .ajax("GET", "/fragments/comments", {
      target: "#commentsFeedContainer",
      swap: "innerHTML",
      values: commentParams({ page }),
    })
    .then(() => {
      window.scrollTo({ top: 0, behavior: "smooth" });
    });
}

function setSortOrder(order) {
  sortOrder = order === "oldest" ? "old" : "new";
  document.getElementById("sortNewest").classList.toggle("selected", order === "newest");
  document.getElementById("sortOldest").classList.toggle("selected", order === "oldest");
  refreshComments();
}

function toggleProfanity(el) {
  el.classList.toggle("selected");
  hideProfanity = el.classList.contains("selected");
  refreshComments();
}

const VOTE_RE = /^\/fragments\/comments\/[^/]+\/vote/;
const DELETE_RE = /^\/fragments\/comments\/[^/]+$/;
let voteSnapshot = null;

async function handleDelete(btn) {
  const id = btn.dataset.commentId;
  if (!id || !window.htmx) return;
  if (!window.confirm("delete this comment?")) return;
  await htmx.ajax("DELETE", `/fragments/comments/${encodeURIComponent(id)}`, {
    target: "#commentsFeedContainer",
    swap: "innerHTML",
    values: commentParams({ page: currentPageNum() }),
  });
}

function voteDelta(v) {
  return v === "up" ? 1 : v === "down" ? -1 : 0;
}

function applyVoteState(capsule, state, score) {
  capsule.dataset.vote = state;
  capsule.dataset.score = String(score);
  capsule.classList.remove("state-up", "state-down");
  if (state === "up") capsule.classList.add("state-up");
  if (state === "down") capsule.classList.add("state-down");
  capsule.querySelectorAll(".expanded-score, .vote-score-center").forEach((el) => {
    el.textContent = String(score);
  });
}

function applyOptimisticVote(capsule, dir) {
  const old = capsule.dataset.vote || "none";
  const nv = old === dir ? "none" : dir;
  const score = (parseInt(capsule.dataset.score, 10) || 0) + voteDelta(nv) - voteDelta(old);
  applyVoteState(capsule, nv, score);
}

document.body.addEventListener("htmx:beforeRequest", (e) => {
  const cfg = e.detail ? e.detail.requestConfig : null;
  if (!cfg || cfg.verb !== "post" || !VOTE_RE.test(cfg.path)) return;
  const capsule = cfg.elt && cfg.elt.closest ? cfg.elt.closest(".vote-capsule") : null;
  if (!capsule) return;
  haptic(8);
  const id = capsule.dataset.voteCapsule;
  const affected = id
    ? Array.from(document.querySelectorAll(`[data-vote-capsule="${CSS.escape(id)}"]`))
    : [capsule];
  const dir = cfg.path.indexOf("dir=up") !== -1 ? "up" : "down";
  voteSnapshot = {
    dir,
    items: affected.map((c) => ({
      el: c,
      html: c.innerHTML,
      vote: c.dataset.vote || "none",
      score: c.dataset.score || "0",
    })),
  };
  affected.forEach((c) => applyOptimisticVote(c, dir));
});

document.body.addEventListener("htmx:beforeSwap", (e) => {
  const cfg = e.detail ? e.detail.requestConfig : null;
  if (!cfg || cfg.verb !== "post" || !VOTE_RE.test(cfg.path)) return;
  if (!e.detail.serverResponse || !e.detail.xhr || e.detail.xhr.status !== 200) return;
  const fresh = new DOMParser().parseFromString(e.detail.serverResponse, "text/html").querySelector(".vote-capsule");
  const current = cfg.elt && cfg.elt.closest ? cfg.elt.closest(".vote-capsule") : null;
  if (!fresh || !current) return;
  if (fresh.dataset.vote === current.dataset.vote && fresh.dataset.score === current.dataset.score) {
    e.detail.shouldSwap = false;
  }
});

document.body.addEventListener("htmx:responseError", () => {
  showToast("error");
  haptic([10, 60, 10]);
});

document.body.addEventListener("htmx:sendError", () => {
  showToast("error");
  haptic([10, 60, 10]);
});

document.body.addEventListener("htmx:afterRequest", (e) => {
  const cfg = e.detail ? e.detail.requestConfig : null;
  if (!cfg) return;
  if (cfg.verb === "delete" && DELETE_RE.test(cfg.path) && e.detail.successful) {
    showToast("deleted");
    haptic(15);
  }
  if (cfg.verb === "post" && VOTE_RE.test(cfg.path)) {
    if (e.detail.successful === false && voteSnapshot) {
      voteSnapshot.items.forEach((s) => {
        if (!s.el.isConnected) return;
        s.el.innerHTML = s.html;
        applyVoteState(s.el, s.vote, parseInt(s.score, 10) || 0);
        if (window.htmx) htmx.process(s.el);
      });
    }
    voteSnapshot = null;
  }
});

document.body.addEventListener("htmx:afterSwap", (e) => {
  const cfg = e.detail && e.detail.requestConfig ? e.detail.requestConfig : null;
  if (cfg && cfg.target && cfg.target.id === "blogViewSlot") {
    const m = (cfg.path || "").match(/^\/fragments\/post\/([^/?]+)/);
    if (restoringRoute) {
      restoringRoute = false;
    } else {
      const url = m ? `/post/${decodeURIComponent(m[1])}` : "/blog";
      if (location.pathname !== url) history.pushState({}, "", url);
    }
    switchTab("blog", false);
  }
  applyDates(document);
  syncScrollLock();
});

const nameInput = document.getElementById("newCommentAuthor");
const textInput = document.getElementById("newCommentText");
const charCounter = document.getElementById("charCounter");
const COMMENT_MAX_LEN = 4000;

function updateCharCount() {
  charCounter.textContent = `${textInput.value.length} / ${COMMENT_MAX_LEN}`;
}

function saveDraft() {
  try {
    localStorage.setItem("kppcnDraft", textInput.value);
  } catch (e) {}
}

let draftSaveTimer = null;
textInput.addEventListener("input", () => {
  updateCharCount();
  clearTimeout(draftSaveTimer);
  draftSaveTimer = setTimeout(saveDraft, 500);
});

nameInput.addEventListener("input", () => {
  try {
    localStorage.setItem("kppcnName", nameInput.value);
  } catch (e) {}
});

function applyFormat(wrapper) {
  const start = textInput.selectionStart;
  const end = textInput.selectionEnd;
  const selected = textInput.value.substring(start, end) || "text";
  if (wrapper === "> ") {
    textInput.setRangeText(wrapper + selected, start, end, "end");
  } else {
    textInput.setRangeText(`${wrapper}${selected}${wrapper}`, start, end, "end");
  }
  textInput.focus();
  updateCharCount();
}

function toggleEmojiPicker() {
  document.getElementById("emojiPopover").classList.toggle("open");
}

function insertAtCursor(s) {
  const a = textInput.selectionStart;
  const b = textInput.selectionEnd;
  const v = textInput.value;
  textInput.value = v.slice(0, a) + s + v.slice(b);
  textInput.selectionStart = textInput.selectionEnd = a + s.length;
  textInput.focus();
}

document.getElementById("emojiPopover").addEventListener("click", (e) => {
  const item = e.target.closest(".emoji-item");
  if (!item || !item.dataset.emojiCode) return;
  insertAtCursor(`:${item.dataset.emojiCode}:`);
  updateCharCount();
  document.getElementById("emojiPopover").classList.remove("open");
});

const sendBtn = document.getElementById("composerSend");
sendBtn.addEventListener("click", async () => {
  const t = textInput.value.trim();
  if (!t || sendBtn.classList.contains("sending")) return;
  const n = nameInput.value.trim() || "anon";
  try {
    localStorage.setItem("kppcnName", n);
  } catch (e) {}
  sendBtn.classList.add("sending");
  try {
    if (!powPromise || Date.now() - powReadyAt > 240000) fetchCaptcha();
    await powPromise;
    const slot = document.getElementById("captchaSlot");
    await htmx.ajax("POST", "/fragments/comments", {
      target: "#commentsFeedContainer",
      swap: "innerHTML",
      values: commentParams({
        name: n,
        text: t,
        captcha_id: slot.querySelector('[name="captcha_id"]')?.value || "",
        pow_nonce: slot.querySelector('[name="pow_nonce"]')?.value || "",
        website: slot.querySelector('[name="website"]')?.value || "",
      }),
    });
  } catch (e) {
    showToast("error");
    fetchCaptcha();
  } finally {
    sendBtn.classList.remove("sending");
  }
});

function showToast(msg) {
  const container = document.getElementById("toastContainer");
  if (!container) return;
  const visible = Array.from(container.querySelectorAll(".toast:not(.hide)"));
  if (visible.some((t) => t.textContent === msg)) return;
  if (visible.length >= 3) visible[0].remove();
  const el = document.createElement("div");
  el.className = "toast";
  el.textContent = msg;
  container.appendChild(el);
  requestAnimationFrame(() => el.classList.add("show"));
  setTimeout(() => {
    el.classList.add("hide");
    setTimeout(() => el.remove(), 400);
  }, 3000);
}

document.body.addEventListener("kppcn:sent", () => {
  showToast("published");
  haptic([15, 40, 15]);
  textInput.value = "";
  try {
    localStorage.removeItem("kppcnDraft");
  } catch (e) {}
  updateCharCount();
  closeModal("newCommentModal");
});

document.addEventListener("keydown", (e) => {
  const active = document.activeElement;
  const isTyping =
    active?.tagName === "INPUT" || active?.tagName === "TEXTAREA" || active?.isContentEditable;

  if (e.key === "Escape") {
    const openModals = document.querySelectorAll(".modal-backdrop.open");
    if (openModals.length > 0) {
      openModals.forEach((m) => m.classList.remove("open"));
      return;
    }
    if (isTyping) {
      active.blur();
      return;
    }
    if (document.getElementById("blogDetailView")) {
      closePostDetail();
    }
    return;
  }

  if (isTyping) return;
  if (e.key === "1") switchTab("home");
  if (e.key === "2") switchTab("blog");
  if (e.key === "3") switchTab("comments");
  if (e.key === "/") {
    e.preventDefault();
    switchTab("comments");
    document.getElementById("commentSearchInput")?.focus();
  }
  if (e.key === "n") {
    e.preventDefault();
    openModal("newCommentModal");
    setTimeout(() => document.getElementById("newCommentText")?.focus(), 80);
  }
});

document.addEventListener("keydown", (e) => {
  if (e.key !== "Enter" && e.key !== " ") return;
  const el = e.target;
  if (el.matches && el.matches('[role="button"][hx-get]')) {
    e.preventDefault();
    el.click();
  }
});

document.addEventListener("click", (e) => {
  const el = e.target;
  const nav = el.closest ? el.closest(".nav-btn[data-tab]") : null;
  if (nav) {
    haptic(8);
    switchTab(nav.dataset.tab);
    return;
  }
  const gotoBtn = el.closest ? el.closest("[data-goto-tab]") : null;
  if (gotoBtn) {
    haptic(8);
    switchTab(gotoBtn.dataset.gotoTab);
    return;
  }
  const closer = el.closest ? el.closest("[data-close]") : null;
  if (closer) {
    closeModal(closer.dataset.close);
    return;
  }
  const del = el.closest ? el.closest(".comment-delete-btn") : null;
  if (del) {
    handleDelete(del);
    return;
  }
  const pageBtn = el.closest ? el.closest(".comments-pager-btn") : null;
  if (pageBtn) {
    handlePageNav(pageBtn);
    return;
  }
  const themeOpt = el.closest ? el.closest("#themeSegmentedList .md3e-seg-item") : null;
  if (themeOpt) {
    selectThemeOption(themeOpt.dataset.themeOpt);
    return;
  }
  if (el.closest && el.closest("#sortNewest")) {
    setSortOrder("newest");
    return;
  }
  if (el.closest && el.closest("#sortOldest")) {
    setSortOrder("oldest");
    return;
  }
  const prof = el.closest ? el.closest("#profanityRow") : null;
  if (prof) {
    toggleProfanity(prof);
    return;
  }
  const fmt = el.closest ? el.closest("[data-format]") : null;
  if (fmt) {
    applyFormat(fmt.dataset.format);
    return;
  }
  if (el.closest && el.closest("#emojiToggleBtn")) {
    toggleEmojiPicker();
    return;
  }
  if (el.closest && el.closest("#backToBlog")) {
    closePostDetail();
    return;
  }
  if (el.closest && el.closest("#avatarTile")) {
    updateHeroMorph();
    return;
  }
  if (el.closest && el.closest("#nameWidgetTile")) {
    triggerNameSqueeze();
    return;
  }
  if (el.closest && el.closest("#settingsBtn")) {
    openModal("themeModal");
    return;
  }
  if (el.closest && el.closest("#filtersBtn")) {
    openModal("filterModal");
    return;
  }
  if (el.closest && el.closest("#composerTriggerBtn")) {
    openModal("newCommentModal");
    return;
  }
  const searchBox = el.closest ? el.closest("#commentsSearchBox") : null;
  if (searchBox) {
    document.getElementById("commentSearchInput")?.focus();
    return;
  }
});

const commentSearchInput = document.getElementById("commentSearchInput");
if (commentSearchInput) {
  let searchTimer = null;
  commentSearchInput.addEventListener("input", () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(refreshComments, 200);
  });
}

const CONSOLE_ART = "aaa:1;bbb:2;abb:2;aab:1;889:1;455:1;334:1;124:1;026:1;048:1;16a:1;17c:1;049:1;024:1;246:1;bcc:2;ccc:3\n9aa:2;999:1;aab:2;9ab:1;99a:1;889:1;556:1;235:1;036:1;259:1;258:1;479:1;024:1;014:1;247:1;bcc:3;ccc:2\nbbb:1;899:1;aaa:1;99a:1;789:1;899:1;89a:1;99a:1;678:1;467:1;036:1;037:1;159:1;148:1;026:1;015:1;247:1;bcc:1;ccc:1;bcc:1;ccc:2\nabb:1;ccc:1;aab:1;a98:1;776:1;788:1;899:1;788:2;467:1;136:1;036:1;148:1;158:1;026:1;014:1;247:1;aa9:2;aaa:1;ccc:2\naaa:1;abb:1;bbb:1;aaa:1;985:1;987:1;888:1;975:1;778:1;246:1;025:2;137:1;036:1;014:2;346:1;998:1;cba:1;aa9:1;787:1;cdc:1\n999:1;9aa:1;aaa:2;863:1;a73:1;b97:1;975:1;986:1;765:1;654:1;875:1;876:1;666:1;335:1;014:1;235:1;665:1;cb7:1;a98:1;998:1;ddc:1\n999:3;a98:1;973:1;c96:1;b98:1;976:1;764:1;963:1;ca6:1;c96:1;b95:1;a84:1;973:1;864:1;445:1;654:1;a98:1;a97:1;bba:1;ddd:1\n9aa:1;999:2;a86:1;b95:1;db9:1;dba:1;a98:1;ba8:1;a74:1;b85:1;b95:1;b84:1;a84:2;973:1;963:1;974:1;a97:1;887:1;333:1;000:1\n999:1;aaa:1;ba9:1;ca6:1;ec9:1;edb:2;ecb:1;a85:2;a74:1;a73:2;b95:1;b84:1;a84:1;a73:1;963:1;863:1;211:1;222:1;010:1\n999:2;553:1;ca8:1;542:1;db9:2;b95:1;a84:1;974:2;ca6:1;c95:2;a83:1;a73:1;b84:2;962:1;652:1;000:2\n899:2;987:1;db8:1;754:1;874:1;862:2;752:2;b85:1;b84:1;da6:1;c95:1;b84:1;a73:2;b84:2;751:1;431:1;322:1\n899:1;999:1;b97:1;a85:1;b96:1;863:1;864:1;753:1;641:1;a84:1;b95:1;da6:1;b84:1;c95:1;a84:1;a74:1;962:2;b84:1;852:1;641:1;865:1\n899:1;999:1;654:1;b97:1;986:1;865:1;643:1;420:1;974:1;b84:1;da6:1;b84:2;973:1;963:1;b84:1;a84:1;963:1;a84:1;a73:1;641:1;765:1\n888:1;999:2;876:1;764:1;542:2;975:1;a84:2;a74:2;973:1;863:1;862:1;a74:1;b84:1;a73:2;851:1;641:1;865:1\n888:2;999:1;765:1;763:1;531:1;652:1;753:1;963:2;863:1;641:1;531:1;a84:1;a74:1;a73:2;973:2;751:1;531:1;865:1\n888:3;987:1;763:1;431:1;541:1;752:1;641:1;642:1;742:1;642:1;863:1;a74:2;642:1;863:2;752:1;531:1;653:1;765:1\n777:1;887:1;888:2;765:1;753:1;752:1;642:1;530:2;642:1;864:1;863:1;852:1;964:1;642:3;320:1;653:1;754:1;765:1\n777:2;888:3;764:1;753:1;642:1;753:1;853:1;a85:1;753:1;863:1;974:1;975:1;853:1;b86:1;642:1;653:1;876:1;764:1;765:1\n666:1;777:1;888:1;887:1;888:1;987:1;c97:1;b97:1;c97:1;b96:1;b86:1;a86:1;c97:1;864:2;b86:1;543:1;332:1;986:1;876:1;654:1;765:1\n555:1;776:1;787:1;887:1;a98:1;db9:1;b96:1;863:1;864:2;863:1;a85:1;753:2;864:1;653:1;664:1;654:1;986:1;876:1;664:1;775:1\n554:1;665:1;777:1;b97:1;da9:1;ca8:1;975:1;642:1;531:1;532:2;542:2;543:1;654:1;765:2;654:1;976:1;875:1;764:1;765:1\n554:1;765:1;c98:1;ca8:1;b97:1;864:1;642:1;654:1;443:1;543:1;443:1;442:1;332:1;331:1;332:1;443:1;553:1;554:1;875:2;654:1;765:1";

function initConsoleArt() {
  const rows = CONSOLE_ART.split("\n");
  let fmt = "";
  const args = [];
  rows.forEach((row, ri) => {
    for (const run of row.split(";")) {
      const p = run.split(":");
      const rgb = [0, 1, 2]
        .map((i) => parseInt(p[0][i], 16) * 17)
        .join(",");
      fmt += "%c" + "  ".repeat(Number(p[1]));
      args.push(`background:rgb(${rgb})`);
    }
    if (ri < rows.length - 1) {
      fmt += "%c\n";
      args.push("");
    }
  });
  console.log(fmt, ...args);
}

initConsoleArt();

window.addEventListener("load", () => {
  syncThemeSegment();

  try {
    const n = localStorage.getItem("kppcnName");
    if (n) nameInput.value = n;
    const d = localStorage.getItem("kppcnDraft");
    if (d) textInput.value = d;
  } catch (e) {}
  updateCharCount();
  applyDates(document);
  syncScrollLock();
});

updateHeroMorph();
if (!REDUCE_MOTION) setInterval(updateHeroMorph, 2700);
