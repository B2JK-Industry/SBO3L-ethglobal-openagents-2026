// ZeroGUploader runtime — companion to
// `src/components/ZeroGUploader.astro`. Lives as a static asset so it
// loads under Vercel's CSP (`script-src 'self' 'wasm-unsafe-eval'`,
// no `'unsafe-inline'`). The component element exposes its constants
// via `data-*` attrs; the runtime auto-binds every
// `[data-zerog-uploader]` on the page at DOMContentLoaded.

function initOne(root) {
  const cfg = {
    storagescanToolUrl: root.dataset.storagescanToolUrl,
    storagescanFileUrl: root.dataset.storagescanFileUrl,
    indexerProbeUrl: root.dataset.indexerProbeUrl,
    probeTimeoutMs: Number(root.dataset.probeTimeoutMs) || 5000,
    maxFileBytes: Number(root.dataset.maxFileBytes) || 1024 * 1024,
  };

  const $ = (sel) => root.querySelector(sel);
  const drop = $(".zg-drop");
  const fileInput = drop.querySelector('input[type="file"]');
  const promptEl = $(".zg-drop-prompt");
  const loadedEl = $(".zg-drop-loaded");
  const fileNameEl = $(".zg-file-name");
  const fileMetaEl = $(".zg-file-meta");
  const clearBtn = $(".zg-clear");
  const errEl = $(".zg-error");
  const uploadBtn = $(".zg-upload");
  const statusEl = $(".zg-status");
  const fallbackEl = $(".zg-fallback");
  const manualHashInput = $(".zg-manual-hash");
  const manualSubmitBtn = $(".zg-manual-submit");
  const successEl = $(".zg-success");
  const successSrcEl = $(".zg-success-source");
  const hashValueEl = $(".zg-hash-value");
  const copyBtn = $(".zg-copy");
  const permalinkEl = $(".zg-permalink");

  let currentFile = null;

  function setError(msg) {
    if (msg) {
      errEl.textContent = msg;
      errEl.hidden = false;
    } else {
      errEl.textContent = "";
      errEl.hidden = true;
    }
  }
  function setStatus(s) {
    statusEl.textContent = s;
  }
  function setState(s) {
    root.dataset.state = s;
  }
  function bytesHuman(n) {
    if (n < 1024) return n + " B";
    if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KB";
    return (n / (1024 * 1024)).toFixed(2) + " MB";
  }
  function isHex32(s) {
    // 0G Storage rootHash is 32 bytes (64 hex chars), 0x-prefixed.
    return /^0x[0-9a-fA-F]{64}$/.test(s.trim());
  }
  function showSuccess(rootHash, source) {
    successEl.hidden = false;
    const norm = rootHash.trim().toLowerCase();
    hashValueEl.textContent = norm;
    const url = cfg.storagescanFileUrl + "/" + norm;
    permalinkEl.textContent = url;
    permalinkEl.href = url;
    successSrcEl.textContent =
      source === "manual"
        ? "Source: manually pasted from 0G storage tool."
        : "Source: 0G SDK auto-upload.";
    setState("success");
  }

  async function pickFile(file) {
    setError(null);
    successEl.hidden = true;
    fallbackEl.hidden = true;
    if (!file) return;
    if (file.size > cfg.maxFileBytes) {
      setError("File too large (" + bytesHuman(file.size) + "). 0G testnet practical cap is 1 MB.");
      return;
    }
    let text;
    try {
      text = await file.text();
    } catch (e) {
      setError("Could not read file: " + (e && e.message ? e.message : String(e)));
      return;
    }
    if (!text || !text.trim()) {
      setError("File is empty.");
      return;
    }
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch (e) {
      setError("Not valid JSON: " + (e && e.message ? e.message : String(e)));
      return;
    }
    if (!parsed || typeof parsed !== "object") {
      setError("JSON root must be an object (got " + (parsed === null ? "null" : typeof parsed) + ").");
      return;
    }
    if (typeof parsed.schema !== "string" || !parsed.schema) {
      setError(
        "Missing top-level `schema` field — this doesn't look like a SBO3L Passport capsule (expected schema like `sbo3l.passport_capsule.v1`)."
      );
      return;
    }
    currentFile = file;
    promptEl.hidden = true;
    loadedEl.hidden = false;
    fileNameEl.textContent = file.name;
    fileMetaEl.textContent = "(" + bytesHuman(file.size) + ", schema=" + parsed.schema + ")";
    uploadBtn.disabled = false;
    setStatus("ready");
    setState("ready");
  }

  function clearFile() {
    currentFile = null;
    fileInput.value = "";
    promptEl.hidden = false;
    loadedEl.hidden = true;
    fileNameEl.textContent = "";
    fileMetaEl.textContent = "";
    uploadBtn.disabled = true;
    setError(null);
    setStatus("idle");
    setState("idle");
    successEl.hidden = true;
    fallbackEl.hidden = true;
  }

  drop.addEventListener("click", () => {
    if (!currentFile) fileInput.click();
  });
  drop.addEventListener("keydown", (e) => {
    if ((e.key === "Enter" || e.key === " ") && !currentFile) {
      e.preventDefault();
      fileInput.click();
    }
  });
  drop.addEventListener("dragover", (e) => {
    e.preventDefault();
    drop.classList.add("zg-drag");
  });
  drop.addEventListener("dragleave", () => {
    drop.classList.remove("zg-drag");
  });
  drop.addEventListener("drop", (e) => {
    e.preventDefault();
    drop.classList.remove("zg-drag");
    const f = e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0];
    if (f) void pickFile(f);
  });
  fileInput.addEventListener("change", (e) => {
    const f = e.target.files && e.target.files[0];
    if (f) void pickFile(f);
  });
  clearBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    clearFile();
  });

  // Probe: 5s AbortController against the 0G indexer. CORS may block the
  // response read, but we only care whether the request resolves vs. times
  // out. `mode: "no-cors"` returns an opaque response on success.
  async function probeIndexer() {
    const ac = new AbortController();
    const timer = setTimeout(() => ac.abort("timeout"), cfg.probeTimeoutMs);
    try {
      await fetch(cfg.indexerProbeUrl, {
        method: "GET",
        mode: "no-cors",
        signal: ac.signal,
        cache: "no-store",
      });
      clearTimeout(timer);
      return { ok: true };
    } catch (e) {
      clearTimeout(timer);
      const reason = ac.signal.aborted
        ? "timeout after " + cfg.probeTimeoutMs + "ms"
        : (e && e.message ? e.message : "network error");
      return { ok: false, reason };
    }
  }

  async function handleUpload() {
    if (!currentFile) return;
    setError(null);
    uploadBtn.disabled = true;
    setStatus("probing 0G indexer…");
    const probe = await probeIndexer();
    if (probe.ok) {
      setStatus("indexer reachable — handing off to manual web tool");
    } else {
      setStatus("indexer unreachable (" + probe.reason + ") — falling back");
    }
    try {
      window.open(cfg.storagescanToolUrl, "_blank", "noopener,noreferrer");
    } catch (_) {
      // Popup blockers are non-fatal; the in-page link still works.
    }
    fallbackEl.hidden = false;
    setState("fallback");
    uploadBtn.disabled = false;
  }

  uploadBtn.addEventListener("click", () => void handleUpload());

  manualSubmitBtn.addEventListener("click", () => {
    const v = manualHashInput.value.trim();
    if (!v) {
      setError("Paste the rootHash from the 0G storage tool first.");
      return;
    }
    if (!isHex32(v)) {
      setError("rootHash must be 0x-prefixed 32-byte hex (66 chars total). Got " + v.length + " chars.");
      return;
    }
    setError(null);
    showSuccess(v, "manual");
    setStatus("manual rootHash accepted");
  });

  manualHashInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      manualSubmitBtn.click();
    }
  });

  copyBtn.addEventListener("click", async () => {
    const v = hashValueEl.textContent || "";
    if (!v) return;
    try {
      await navigator.clipboard.writeText(v);
      copyBtn.classList.add("zg-copied");
      const orig = copyBtn.textContent;
      copyBtn.textContent = "Copied";
      setTimeout(() => {
        copyBtn.classList.remove("zg-copied");
        copyBtn.textContent = orig;
      }, 1500);
    } catch (_) {
      // Fallback for browsers without clipboard API — select the hash.
      const range = document.createRange();
      range.selectNode(hashValueEl);
      const sel = window.getSelection();
      if (sel) {
        sel.removeAllRanges();
        sel.addRange(range);
      }
    }
  });
}

function bindAll() {
  document.querySelectorAll("[data-zerog-uploader]").forEach((root) => {
    if (root.dataset.zerogBound === "1") return;
    root.dataset.zerogBound = "1";
    initOne(root);
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bindAll);
} else {
  bindAll();
}
