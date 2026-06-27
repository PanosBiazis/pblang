const state = {
  fixture: null,
  sample: document.getElementById("prompt").value,
  lastCode: "",
  server: false,
  pending: false,
  architectureEvents: [],
  architectureMetrics: {},
  architectureView: { yaw: -0.35, pitch: 0.18, zoom: 0.76, auto: true }
};

// Render loops register a resume callback here so they can be paused while the
// tab is hidden (saves CPU/battery) and restarted when it becomes visible again.
const animators = [];
const reduceMotion = window.matchMedia
  ? window.matchMedia("(prefers-reduced-motion: reduce)")
  : { matches: false, addEventListener() {} };
function startLoop(step) {
  let looping = false;
  function frame() {
    if (document.hidden) { looping = false; return; }
    step();
    // Honor reduced-motion: render a single up-to-date frame instead of looping.
    if (reduceMotion.matches) { looping = false; return; }
    requestAnimationFrame(frame);
  }
  function resume() {
    if (looping || document.hidden) return;
    looping = true;
    frame();
  }
  animators.push(resume);
  resume();
}
// Re-render every registered loop once (used after state changes so reduced-motion
// users still see updated content; a no-op for loops already animating).
function requestRedraw() {
  if (document.hidden) return;
  for (const resume of animators) resume();
}
document.addEventListener("visibilitychange", requestRedraw);
reduceMotion.addEventListener("change", requestRedraw);

const output = document.getElementById("output");
const outputTitle = document.getElementById("outputTitle");
const modeBadge = document.getElementById("modeBadge");
const fixtureState = document.getElementById("fixtureState");
const planList = document.getElementById("planList");
const signalList = document.getElementById("signalList");
const verifyText = document.getElementById("verifyText");
const chatLog = document.getElementById("chatLog");
const chatInput = document.getElementById("chatInput");
const themeButtons = [...document.querySelectorAll("[data-theme-choice]")];
const learnText = document.getElementById("learnText");
const learnTextButton = document.getElementById("learnTextButton");
const learnFiles = document.getElementById("learnFiles");
const learnFolder = document.getElementById("learnFolder");
const learnPath = document.getElementById("learnPath");
const learnPathButton = document.getElementById("learnPathButton");
const learnStatus = document.getElementById("learnStatus");

function setTheme(theme) {
  const next = theme === "dark" ? "dark" : "light";
  document.documentElement.dataset.theme = next;
  localStorage.setItem("pbai-theme", next);
  for (const button of themeButtons) {
    button.classList.toggle("active", button.dataset.themeChoice === next);
  }
}

setTheme(document.documentElement.dataset.theme || "light");

// Persist the prompt/editor across reloads so work isn't lost on refresh.
const promptEl = document.getElementById("prompt");
const savedPrompt = localStorage.getItem("pbai-prompt");
if (savedPrompt !== null) promptEl.value = savedPrompt;
promptEl.addEventListener("input", () => {
  try { localStorage.setItem("pbai-prompt", promptEl.value); } catch {}
});

async function fetchWithTimeout(url, options = {}, timeoutMs = 15000) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...options, signal: controller.signal });
  } catch (err) {
    if (err.name === "AbortError") throw new Error(`request timed out after ${timeoutMs} ms`);
    throw err;
  } finally {
    clearTimeout(timer);
  }
}

async function apiCall(action, payload = {}, { timeoutMs = 15000 } = {}) {
  const res = await fetchWithTimeout(`/api/pbai/${action}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload)
  }, timeoutMs);
  let data;
  try {
    data = await res.json();
  } catch {
    throw new Error(`invalid JSON from /api/pbai/${action} (${res.status} ${res.statusText})`);
  }
  if (!res.ok || !data.ok) throw new Error(data.error || `${res.status} ${res.statusText}`);
  pulseArchitecture(action, data.value);
  return data.value;
}

function setLearnStatus(text) {
  if (learnStatus) learnStatus.textContent = text;
}

// Guard against overlapping live requests (double-clicks, concurrent learn+chat)
// and expose a visible/assistive-tech busy state while one is in flight.
function setBusy(on) {
  state.pending = on;
  document.body.dataset.busy = on ? "true" : "false";
  output.setAttribute("aria-busy", on ? "true" : "false");
}

function architecturePath(action) {
  if (action === "respond") return ["prompt", "tokenizer", "nano", "brain", "response"];
  if (action === "analyze" || action === "self_eval") return ["prompt", "brain", "memory", "response"];
  if (action === "think" || action === "brain") return ["prompt", "brain", "tools", "brain", "response"];
  if (action === "generate") return ["prompt", "tokenizer", "nano", "response"];
  if (action === "tokenize") return ["prompt", "tokenizer", "response"];
  if (action === "learn") return ["prompt", "memory", "nano", "brain", "response"];
  if (action === "status") return ["tools", "brain", "nano"];
  return ["prompt", "brain", "response"];
}

function pulseArchitecture(action, value = null) {
  if (action === "status" && value) {
    state.architectureMetrics = {
      tier: value.bottom?.tier || "pbAI-Nano",
      format: value.bottom?.format || "pbai.lm.char.v1",
      vocab: value.bottom?.architecture?.vocab_size || value.tokenizer?.vocab_size || "?",
      liveChars: value.bottom?.live_learning?.live_chars || 0,
      sourceChars: value.bottom?.live_learning?.source_chars || 0,
      brain: value.brain?.format || "pbai.brain.loop.v1"
    };
  }
  state.architectureEvents.push({
    action,
    path: architecturePath(action),
    started: performance.now(),
    duration: action === "learn" ? 2600 : 1700,
    color: action === "learn" ? "#f97316" : action === "generate" ? "#a78bfa" : action === "think" ? "#f43f5e" : "#2dd4bf"
  });
  if (state.architectureEvents.length > 16) state.architectureEvents.splice(0, state.architectureEvents.length - 16);
  requestRedraw();
}

async function detectServer() {
  try {
    const res = await fetchWithTimeout("/api/health", { cache: "no-store" }, 4000);
    const data = await res.json();
    state.server = Boolean(data.ok);
  } catch {
    state.server = false;
  }
  modeBadge.textContent = state.server ? "live pblang pbAI" : "fixture + browser mirror";
  setLearnStatus(state.server ? "Live learning ready. Text, file, and folder lessons write to stdlib/pbai_live_corpus.txt." : "Waiting for live pblang server.");
  return state.server;
}

function count(text, part) {
  if (!part) return 0;
  return text.split(part).length - 1;
}

function lower(text) {
  return String(text).toLowerCase();
}

function extractCode(prompt) {
  const marker = "--- pblang code ---";
  const idx = prompt.indexOf(marker);
  if (idx >= 0) return prompt.slice(idx + marker.length).trim();
  const fence = prompt.indexOf("```");
  if (fence >= 0) {
    const raw = prompt.slice(fence + 3).replace(/```[\s\S]*$/, "").trim();
    return raw.replace(/^(pblang|sapp|pb)\s*\n/i, "").trim();
  }
  const blank = prompt.indexOf("\n\n");
  if (blank >= 0) return prompt.slice(blank).trim();
  return prompt.trim();
}

function looksLikeMapIndexRead(code) {
  return (code.includes("map{") || code.includes("map =") || code.includes("map="))
    && code.includes("[")
    && !code.includes("map_has");
}

function scan(prompt) {
  const code = extractCode(prompt);
  const lc = lower(code);
  const depth = [...code].reduce((acc, ch) => {
    if (ch === "{") acc.cur += 1;
    if (ch === "}") acc.cur = Math.max(0, acc.cur - 1);
    acc.max = Math.max(acc.max, acc.cur);
    return acc;
  }, { cur: 0, max: 0 }).max;

  const s = {
    fns: count(code, "fn "),
    lets: count(code, "let ") + count(code, "const "),
    whiles: count(code, "while "),
    fors: count(code, "for "),
    ifs: count(code, "if "),
    prints: count(code, "print"),
    returns: count(code, "return"),
    imports: count(code, "import("),
    fs: count(code, "fs_read") + count(code, "fs_write"),
    fsTypos: count(code, "fs read") + count(code, "fs write"),
    http: count(code, "http_get") + count(code, "http_post"),
    execs: count(code, "proc_spawn") + count(code, "exec("),
    ffi: count(code, "ffi_"),
    env: count(code, "env_get") + count(code, "env_set"),
    todos: count(lc, "todo") + count(lc, "fixme"),
    openBrace: count(code, "{"),
    closeBrace: count(code, "}"),
    openParen: count(code, "("),
    closeParen: count(code, ")"),
    lines: count(code, "\n") + 1,
    depth
  };
  s.hasCode = s.fns > 0 || s.lets > 0 || s.openBrace > 0 || code.includes("print ") || code.includes("return ");
  return { code, scan: s };
}

function quality(s) {
  let score = 100 - s.todos * 12 - s.depth * 5 - (s.whiles + s.fors) * 2 - s.ifs;
  if (s.openBrace !== s.closeBrace) score -= 30;
  if (s.openParen !== s.closeParen) score -= 30;
  return Math.max(0, Math.min(100, score));
}

function confidence(s) {
  let score = 58;
  if (s.hasCode) score += 18;
  if (s.fns > 0) score += 8;
  if (s.openBrace === s.closeBrace && s.openParen === s.closeParen) score += 8;
  if (s.fs || s.http || s.execs || s.ffi || s.env) score -= 8;
  if (s.depth >= 5) score -= 6;
  return Math.max(0, Math.min(99, score));
}

function risk(s) {
  const caps = [];
  if (s.fs) caps.push("fs");
  if (s.http) caps.push("http");
  if (s.execs) caps.push("exec");
  if (s.ffi) caps.push("ffi");
  if (s.env) caps.push("env");
  return caps.length ? `uses sensitive capability area(s): ${caps.join(", ")}` : "no obvious sensitive capability use";
}

function reviewFindings(code, s) {
  const findings = [];
  if (s.fns === 0 && /^\s*[A-Za-z_][A-Za-z0-9_]*\s*\([^)]*\)\s*\{/.test(code)) findings.push("missing function keyword before a function-like block");
  if (/\blet\s+[A-Za-z_][A-Za-z0-9_]*\s+[A-Za-z_][A-Za-z0-9_]*\s*\(/.test(code)) findings.push("missing assignment operator in a let binding");
  if (s.fsTypos) findings.push("filesystem builtin is misspelled as spaced words");
  if (s.openBrace !== s.closeBrace) findings.push("unbalanced braces");
  if (s.openParen !== s.closeParen) findings.push("unbalanced parentheses");
  if (code.includes("print(")) findings.push("invalid print call form; print is a statement");
  if (looksLikeMapIndexRead(code)) findings.push("dynamic map read should be guarded with map_has");
  if ((s.fs || s.http || s.imports) && !code.includes("try ")) findings.push("fallible I/O/import path has no try/catch");
  if (s.execs && code.includes("+")) findings.push("process command appears dynamically composed; review injection risk");
  if (s.execs) findings.push("process execution requires explicit capability review");
  if (s.ffi) findings.push("FFI requires high-trust native-code capability review");
  if (s.env) findings.push("environment access can expose secrets");
  if (s.fns > 0 && s.returns === 0) findings.push("function body has no explicit return");
  if (s.todos) findings.push("TODO/FIXME markers remain");
  return findings;
}

function topicLine(topic) {
  const lines = {
    maps: "- maps: `map{ k: v }`; missing keys raise, so guard dynamic reads with `map_has(m, k)`.",
    loops: "- loops: `while cond { ... }`, `for i N { ... }`, and `for v in arr { ... }`; use `break`/`continue` normally.",
    fns: "- functions: `fn name(a, b = 1) { return a + b; }`; closures use `fn(x) { ... }`.",
    oop: "- OOP: `struct` and `class` self-host; call methods with `obj:method(args)`.",
    strings: "- strings: codepoint-based; use `+`, str_split, str_slice, str_index_of, and str_contains.",
    errors: "- errors: `try EXPR catch FALLBACK`, `throw v`, and `error(\"msg\")`.",
    imports: "- imports: `let m = import(\"stdlib/foo.sapp\");`, then `m.fn(...)`.",
    async: "- async: `async fn`, `spawn`/`spawn_n`, and waitgroup-style coordination.",
    precision: "- precision: `bigint`, `rational`, and `decimal` cover exact/high-precision arithmetic.",
    jit: "- JIT/perf: Rust bytecode VM plus Cranelift JIT via `PBLANG_JIT` for suitable numeric functions.",
    capabilities: "- capabilities: `--sandbox`, `--allow-exec`, `--allow-ffi`, and env/fs/http gates keep trust explicit.",
    data: "- data: routes text/code/logs/diffs/tests/JSON/CSV/Markdown plus image/audio/video/binary hints."
  };
  return lines[topic] || "";
}

function multiTopicAnswer(t) {
  const topics = [];
  if (t.includes("map")) topics.push("maps");
  if (t.includes("loop") || t.includes("while")) topics.push("loops");
  if (t.includes("fn") || t.includes("function")) topics.push("fns");
  if (t.includes("oop") || t.includes("object") || t.includes("class") || t.includes("struct")) topics.push("oop");
  if (t.includes("string") || t.includes("text")) topics.push("strings");
  if (t.includes("error") || t.includes("try") || t.includes("catch")) topics.push("errors");
  if (t.includes("import") || t.includes("module")) topics.push("imports");
  if (t.includes("async") || t.includes("spawn")) topics.push("async");
  if (t.includes("precision") || t.includes("bigint") || t.includes("rational") || t.includes("decimal")) topics.push("precision");
  if (t.includes("jit") || t.includes("perf") || t.includes("speed")) topics.push("jit");
  if (t.includes("capab") || t.includes("sandbox") || t.includes("permission")) topics.push("capabilities");
  if (t.includes("data") || t.includes("json") || t.includes("csv") || t.includes("image") || t.includes("audio") || t.includes("video")) topics.push("data");
  if (topics.length < 2) return "";
  return `pbAI: quick pblang map\n${topics.map(topicLine).join("\n")}\nAsk one topic for a deeper answer, or paste code and I will review it.`;
}

function wantsPreviousCode(prompt) {
  const t = lower(prompt);
  return t.includes("improve") || t.includes("better") || t.includes("fix")
    || t.includes("bug") || t.includes("review") || t.includes("explain")
    || t.includes("comment") || t === "it" || t.endsWith(" it");
}

function promptWithContext(prompt) {
  const current = scan(prompt);
  if (current.scan.hasCode || !state.lastCode || !wantsPreviousCode(prompt)) return prompt;
  return `${prompt}\n\n--- pblang code ---\n${state.lastCode}`;
}

function rememberCode(prompt) {
  const { code, scan: s } = scan(prompt);
  if (s.hasCode) state.lastCode = code;
}

function learningLesson(text) {
  const trimmed = text.trim();
  const lc = lower(trimmed);
  if (lc.startsWith("learn:") || lc.startsWith("teach:")) {
    return trimmed.slice(trimmed.indexOf(":") + 1).trim();
  }
  if (lc.startsWith("learn ")) return trimmed.slice(6).trim();
  if (lc.startsWith("teach ")) return trimmed.slice(6).trim();
  return "";
}

function isTextLikeFile(file) {
  const name = lower(file.name);
  if (file.type && (file.type.startsWith("text/") || file.type.includes("json") || file.type.includes("xml") || file.type.includes("javascript"))) return true;
  return /\.(txt|md|json|jsonl|csv|tsv|sapp|pb|js|ts|html|css|py|rs|c|cpp|h|hpp|cs|pas|java|kt|go|rb|php|sql|sh|bat|ps1|toml|yaml|yml|ini|log)$/i.test(name);
}

function fileLabel(file) {
  return file.webkitRelativePath || file.name;
}

function fileMetadataLesson(file) {
  return [
    `file metadata: ${fileLabel(file)}`,
    `name: ${file.name}`,
    `type: ${file.type || "unknown/binary"}`,
    `size_bytes: ${file.size}`,
    `last_modified: ${new Date(file.lastModified || Date.now()).toISOString()}`,
    "content_status: metadata only; add a parser/backend for deep binary understanding"
  ].join("\n");
}

function chunkText(text, size = 6000, maxChunks = 4) {
  const chunks = [];
  let start = 0;
  while (start < text.length && chunks.length < maxChunks) {
    chunks.push(text.slice(start, start + size));
    start += size;
  }
  return chunks;
}

async function learnLesson(lesson, epochs = 4) {
  if (!lesson.trim()) return null;
  if (!state.server) {
    await detectServer();
    if (!state.server) throw new Error("live pblang server is not connected");
  }
  const learned = await apiCall("learn", { prompt: lesson, epochs }, { timeoutMs: 60000 });
  return learned;
}

async function learnTextNow() {
  const lesson = learnText.value.trim();
  if (!lesson) {
    setLearnStatus("Write a lesson first.");
    return;
  }
  if (state.pending) return;
  setBusy(true);
  try {
    setLearnStatus("Learning text lesson...");
    const learned = await learnLesson(lesson, 6);
    setLearnStatus(`Learned text. loss ${learned.metrics.loss_before} -> ${learned.metrics.loss_after}`);
    appendChat("assistant", `pbAI learned this text lesson.\nAsk about it in chat now.\n\n${lesson}`);
    show("Live Learning", learned, null);
  } catch (err) {
    setLearnStatus(`Learning failed: ${err.message}`);
  } finally {
    setBusy(false);
  }
}

async function learnFilesNow(files, sourceLabel) {
  const list = [...files];
  if (!list.length) return;
  if (state.pending) return;
  const maxFiles = 80;
  const selected = list.slice(0, maxFiles);
  let learnedCount = 0;
  let chunkCount = 0;
  setBusy(true);
  try {
    if (!state.server) await detectServer();
    if (!state.server) throw new Error("live pblang server is not connected");
    setLearnStatus(`Learning ${selected.length}${list.length > maxFiles ? ` of ${list.length}` : ""} ${sourceLabel} item(s)...`);
    for (let i = 0; i < selected.length; i += 1) {
      const file = selected[i];
      const label = fileLabel(file);
      if (isTextLikeFile(file) && file.size <= 700_000) {
        const raw = await file.text();
        const chunks = chunkText(raw);
        for (let c = 0; c < chunks.length; c += 1) {
          const lesson = `file content chunk ${c + 1}/${chunks.length}: ${label}\n--- content ---\n${chunks[c]}`;
          await learnLesson(lesson, 2);
          chunkCount += 1;
        }
      } else {
        await learnLesson(fileMetadataLesson(file), 2);
        chunkCount += 1;
      }
      learnedCount += 1;
      setLearnStatus(`Learned ${learnedCount}/${selected.length} file(s), ${chunkCount} chunk(s). Current: ${label}`);
    }
    const suffix = list.length > maxFiles ? ` Capped at ${maxFiles} files to keep the live server responsive.` : "";
    const msg = `pbAI learned ${learnedCount} ${sourceLabel} item(s) as ${chunkCount} live-memory chunk(s).${suffix}\nAsk about a filename, folder path, code symbol, or text from those files.`;
    setLearnStatus(msg);
    appendChat("assistant", msg);
    pulseArchitecture("learn", null);
  } catch (err) {
    setLearnStatus(`File/folder learning failed: ${err.message}`);
  } finally {
    if (sourceLabel === "file") learnFiles.value = "";
    if (sourceLabel === "folder") learnFolder.value = "";
    setBusy(false);
  }
}

async function learnPathNow() {
  const path = learnPath.value.trim();
  if (!path) {
    setLearnStatus("Enter a local path first.");
    return;
  }
  if (state.pending) return;
  setBusy(true);
  try {
    if (!state.server) await detectServer();
    if (!state.server) throw new Error("live pblang server is not connected");
    setLearnStatus(`Server-side ingest running for ${path}...`);
    const result = await apiCall("ingest_path", { path, max_files: 80, max_depth: 4, epochs: 4 }, { timeoutMs: 120000 });
    const s = result.stats || {};
    const msg = `pbAI ingested local path: ${path}\n- files: ${s.files || 0}\n- chunks: ${s.chunks || 0}\n- metadata entries: ${s.metadata || 0}\n- live chars: ${result.live_after?.live_chars || "?"}\nAsk about filenames, code symbols, or text from that path.`;
    setLearnStatus(msg);
    appendChat("assistant", msg);
    show("Path Learning", result, null);
  } catch (err) {
    setLearnStatus(`Path learning failed: ${err.message}`);
  } finally {
    setBusy(false);
  }
}

function suggestedFix(code) {
  let fixed = code.trim();
  fixed = fixed.replace(/\bprint\s*\(([^;\n]+?)\)\s*;/g, "print $1;");
  fixed = fixed.replace(/\b(fs)\s+(read|write)\s*\(/g, "$1_$2(");
  fixed = fixed.replace(/\blet\s+([A-Za-z_][A-Za-z0-9_]*)\s+([A-Za-z_][A-Za-z0-9_]*\s*\()/g, "let $1 = $2");
  fixed = fixed.replace(/^(\s*)([A-Za-z_][A-Za-z0-9_]*\s*\([^)]*\)\s*\{)/, "$1fn $2");
  return fixed;
}

function simpleArithmetic(prompt) {
  const t = lower(prompt).trim().replace(/\s+/g, "").replace(/\?$/, "");
  const m = t.match(/^(\d+)([+\-*/])(\d+)$/);
  if (!m) return "";
  const a = Number(m[1]);
  const op = m[2];
  const b = Number(m[3]);
  if (op === "+") return `pbAI math: ${a} + ${b} = ${a + b}`;
  if (op === "-") return `pbAI math: ${a} - ${b} = ${a - b}`;
  if (op === "*") return `pbAI math: ${a} * ${b} = ${a * b}`;
  if (op === "/") {
    if (b === 0) return "pbAI math: division by zero is undefined.";
    return `pbAI math: ${a} / ${b} = quotient ${Math.floor(a / b)}, remainder ${a % b}.`;
  }
  return "";
}

function respond(prompt) {
  const effectivePrompt = promptWithContext(prompt);
  const { code, scan: s } = scan(effectivePrompt);
  const t = lower(effectivePrompt);
  const math = simpleArithmetic(prompt);
  if (math) return math;
  const broad = multiTopicAnswer(t);
  if (broad) return broad;
  if (t.includes("best llm") || t.includes("be the best") || t.includes("make you better") || t.includes("frontier")) {
    return "pbAI improvement target: become a pblang-written LLM family with live learning, multimodal data routing, memory, tools, retrieval, tests, and verified coding ability. Today I am Nano + grounded skills; next upgrades are tokenizer, transformer blocks, corpora, benchmarks, retrieval memory, and tool-verified self-improvement.";
  }
  if (t.includes("learn you") || t.includes("teach you") || t.includes("learn:") || t.includes("teach:")) {
    return "pbAI learning mode: send `learn: <lesson or example>` in the live website and I will store it in the local live corpus, retrain pbAI-Nano, and report the learning loss.";
  }

  if (t.includes("brain") || t.includes("memory") || t.includes("plan")) {
    return "pbAI brain loop: pbai.brain.loop.v1 multimodal data perception + attention + goals + working/episodic/long-term memory + planning + critique + verification + reflection, all pblang-native.";
  }
  if (t.includes("tokenizer")) {
    return "pbAI tokenizer: pbai.tokenizer.char.v1 (codepoint-char), vocab=40, specials=<pad>/<bos>/<eos>/<unk>, all pblang-native.";
  }
  if (t.includes("data") || t.includes("multimodal") || t.includes("image") || t.includes("audio") || t.includes("video") || t.includes("json") || t.includes("csv")) {
    return "pbAI data understanding: today the pblang brain detects and routes text, pblang/code, logs, diffs, tests, JSON, CSV, Markdown, and file-type hints for image/audio/video/binary data. Deep image/audio/video understanding is planned through native parsers, retrieval, embeddings, and optional plugin backends behind the same pblang capability gates.";
  }
  if (!s.hasCode) {
    if (t.includes("what's up") || t.includes("whats up") || t.includes("sup")) {
      return "pbAI: running live when the pblang server is connected. I can answer pblang questions, review code, think through a plan, and learn local examples with `learn: ...`.";
    }
    if (t.includes("hello") || t.includes("hi ") || t === "hi") {
      return "pbAI: I'm pblang's own assistant. Ask a topic, paste code, or teach me with `learn: ...`.";
    }
    return "pbAI: I need either a pblang topic, a code snippet, or a learning command. Try `maps`, `loops`, `Find bugs:\\n--- pblang code ---\\n...`, or `learn: <new lesson>`.";
  }

  if (t.includes("improve") || t.includes("better") || t.includes("fix")) {
    let text = `pbAI: improved pblang\n- quality score: ${quality(s)}/100; ${risk(s)}.\n`;
    if (s.fs && !code.includes("try ")) text += "- wrap `fs_read` in `try ... catch ...` so bad paths do not abort the program.\n";
    if ((s.http || s.imports) && !code.includes("try ")) text += "- wrap fallible http/import paths in `try ... catch ...`.\n";
    if (s.execs && code.includes("+")) text += "- avoid dynamically composed process commands; validate parts or use an argv-style API.\n";
    if (code.includes("print(")) text += "- changed invalid `print(x);` statement syntax to `print x;`.\n";
    if (s.fsTypos) text += "- changed `fs read/write` to pblang builtin names `fs_read/fs_write`.\n";
    const fixed = suggestedFix(code);
    text += "\n```pblang\n" + fixed + "\n```\n";
    return text;
  }

  let text = `pbAI: review (browser mirror)\n- quality score: ${quality(s)}/100; ${risk(s)}.\n`;
  let found = false;
  if (s.fns === 0 && /^\s*[A-Za-z_][A-Za-z0-9_]*\s*\([^)]*\)\s*\{/.test(code)) {
    text += "- missing `fn` before the function name.\n";
    found = true;
  }
  if (/\blet\s+[A-Za-z_][A-Za-z0-9_]*\s+[A-Za-z_][A-Za-z0-9_]*\s*\(/.test(code)) {
    text += "- missing `=` in the `let` binding; write `let name = value;`.\n";
    found = true;
  }
  if (s.fsTypos) {
    text += "- `fs read(...)` / `fs write(...)` are not pblang builtins; use `fs_read(...)` / `fs_write(...)`.\n";
    found = true;
  }
  if (s.openBrace !== s.closeBrace) {
    text += `! unbalanced braces: ${s.openBrace} vs ${s.closeBrace}.\n`;
    found = true;
  }
  if (s.openParen !== s.closeParen) {
    text += `! unbalanced parentheses: ${s.openParen} vs ${s.closeParen}.\n`;
    found = true;
  }
  if (code.includes("print(")) {
    text += "- `print(x)` is INVALID in pblang; write `print x;`.\n";
    found = true;
  }
  if (looksLikeMapIndexRead(code)) {
    text += "- reading a missing map key raises; guard dynamic reads with `map_has(m, k)`.\n";
    found = true;
  }
  if ((s.fs || s.http || s.imports) && !code.includes("try ")) {
    text += "- fallible I/O/import path has no `try ... catch`; failures can abort the run.\n";
    found = true;
  }
  if (s.execs && code.includes("+")) {
    text += "- command string appears dynamically composed; review injection risk before `--allow-exec`.\n";
    found = true;
  }
  if (s.fs) {
    text += "- filesystem reads/writes need explicit capability review.\n";
    found = true;
  }
  const fixed = suggestedFix(code);
  if (fixed !== code.trim()) {
    text += "\nSuggested pblang fix:\n```pblang\n" + fixed + "\n```\n";
  }
  if (!found) text += "- no structural issues found in the browser mirror.\n";
  return text;
}

function analyze(prompt) {
  const { code, scan: s } = scan(promptWithContext(prompt));
  return {
    intent: lower(prompt).includes("bug") ? "bugs" : "general",
    has_code: s.hasCode,
    quality: quality(s),
    risk: risk(s),
    confidence: confidence(s),
    findings: reviewFindings(code, s),
    suggested_fix: suggestedFix(code),
    scan: s
  };
}

function perceive(prompt) {
  const effectivePrompt = promptWithContext(prompt);
  const t = lower(effectivePrompt);
  const { scan: s } = scan(effectivePrompt);
  const signals = [];
  let kind = "question";
  if (s.hasCode) { kind = "code"; signals.push("code"); }
  if (t.includes("error") || t.includes("failed")) { kind = "failure"; signals.push("failure"); }
  if (t.includes("test") || t.includes("assert")) signals.push("test");
  if (t.includes("tool") || t.includes("mcp")) signals.push("tool");
  if (!signals.length) signals.push("language");
  return {
    format: "pbai.brain.loop.v1",
    kind,
    signals: [...new Set(signals)],
    text_len: prompt.length,
    needs_verification: kind === "code" || kind === "failure" || signals.includes("test"),
    confidence_hint: kind === "question" ? 0.62 : 0.74
  };
}

function think(prompt) {
  const effectivePrompt = promptWithContext(prompt);
  const p = perceive(effectivePrompt);
  const steps = [
    { action: "perceive", target: p.kind, done: true },
    { action: "retrieve", target: "working memory + pblang docs", done: false },
    p.kind === "question"
      ? { action: "answer", target: "grounded pblang knowledge", done: false }
      : { action: "analyze", target: "structure, risks, pblang gotchas", done: false },
    p.needs_verification
      ? { action: "verify", target: "lint/run/tests or deterministic checks", done: false }
      : { action: "self_check", target: "critique answer before returning", done: false }
  ];
  return {
    format: "pbai.brain.loop.v1",
    perception: p,
    plan: { format: "pbai.brain.loop.v1", steps, tool_ready: p.needs_verification, confidence: p.confidence_hint },
    critique: {
      risks: p.needs_verification ? ["tool output can be stale or unavailable; verify before acting"] : ["no immediate structural risk detected"],
      recommendation: p.needs_verification ? "run the verifier step before marking complete" : "answer with the grounded knowledge path"
    },
    verification: null,
    answer: respond(effectivePrompt)
  };
}

function generate(prompt) {
  const seed = lower(prompt).includes("let") ? "let " : "fn ";
  return `${seed}main() { print "pbAI"; }`;
}

function tokenize(prompt) {
  const text = extractCode(prompt).slice(0, 32) || "fn ";
  return {
    format: "pbai.tokenizer.char.v1",
    text,
    ids: [1, ...[...text].map((ch) => ch.codePointAt(0)), 2],
    tokens: ["<bos>", ...[...text], "<eos>"],
    decoded: text
  };
}

function renderPlan(thought) {
  const steps = thought?.plan?.steps || state.fixture?.thought?.plan?.steps || [];
  planList.innerHTML = "";
  for (const step of steps) {
    const li = document.createElement("li");
    li.textContent = `${step.action}: ${step.target}`;
    planList.appendChild(li);
  }
}

function renderSignals(thought) {
  const signals = thought?.perception?.signals || state.fixture?.thought?.perception?.signals || [];
  signalList.innerHTML = "";
  for (const signal of signals) {
    const chip = document.createElement("span");
    chip.className = "chip";
    chip.textContent = signal;
    signalList.appendChild(chip);
  }
}

function renderVerification(thought) {
  const verification = thought?.verification || state.fixture?.thought?.verification;
  verifyText.textContent = verification ? verification.summary : "No verification run yet.";
}

function show(title, value, thought = null) {
  outputTitle.textContent = title;
  output.textContent = typeof value === "string" ? value : JSON.stringify(value, null, 2);
  renderPlan(thought);
  renderSignals(thought);
  renderVerification(thought);
}

function appendChat(role, text) {
  const message = document.createElement("article");
  message.className = `chat-message ${role}`;

  const label = document.createElement("div");
  label.className = "chat-role";
  label.textContent = role === "user" ? "You" : "pbAI";

  const body = document.createElement("pre");
  body.textContent = text;

  message.append(label, body);
  chatLog.appendChild(message);
  chatLog.scrollTop = chatLog.scrollHeight;
}

async function chat(prompt) {
  const text = prompt.trim();
  if (!text) return;
  if (state.pending) return;
  appendChat("user", text);
  rememberCode(text);
  setBusy(true);
  try {
    if (state.server) {
      const lesson = learningLesson(text);
      if (lesson) {
        const learned = await apiCall("learn", { prompt: lesson, epochs: 6 }, { timeoutMs: 60000 });
        const msg = `pbAI learned this local lesson.\n- live loss before: ${learned.metrics.loss_before}\n- live loss after: ${learned.metrics.loss_after}\n- source: stdlib/pbai_live_corpus.txt`;
        appendChat("assistant", msg);
        show("Live Learning", learned, null);
        return;
      }
      const live = await apiCall("respond", { prompt: text });
      rememberCode(promptWithContext(text));
      appendChat("assistant", live.response);
      show("Chat Response", live.response, live.thought);
      return;
    }
  } catch (err) {
    appendChat("assistant", `pbAI server unavailable, using browser mirror: ${err.message}`);
    state.server = false;
    modeBadge.textContent = "browser mirror";
  } finally {
    setBusy(false);
  }
  const thought = think(text);
  rememberCode(promptWithContext(text));
  appendChat("assistant", thought.answer);
  show("Chat Response", thought.answer, thought);
}

function updateStatus(data) {
  const status = data?.status;
  if (!status) return;
  pulseArchitecture("status", status);
  const items = [
    ["Family", status.family],
    ["Bottom", `${status.bottom.tier} / ${status.bottom.format}`],
    ["Tokenizer", status.tokenizer.format],
    ["Brain", status.brain.format]
  ];
  // Build with textContent rather than innerHTML so server-provided strings can
  // never inject markup/script into the page.
  const grid = document.getElementById("statusGrid");
  grid.replaceChildren();
  for (const [label, value] of items) {
    const cell = document.createElement("div");
    const span = document.createElement("span");
    span.textContent = label;
    const strong = document.createElement("strong");
    strong.textContent = value ?? "—";
    cell.append(span, strong);
    grid.appendChild(cell);
  }
}

async function loadFixture() {
  await detectServer();
  if (state.server) {
    try {
      const status = await apiCall("status");
      state.fixture = { status };
      fixtureState.textContent = "Connected to live pblang pbAI server.";
      modeBadge.textContent = "live pblang pbAI";
      updateStatus(state.fixture);
      const prompt = document.getElementById("prompt").value;
      const live = await apiCall("respond", { prompt });
      show("Live Response", live.response, live.thought);
      return;
    } catch (err) {
      state.server = false;
      fixtureState.textContent = `Live server failed: ${err.message}. Falling back to fixture/browser mirror.`;
      modeBadge.textContent = "fixture + browser mirror";
    }
  }
  if (window.PBAI_DEMO_DATA) {
    state.fixture = window.PBAI_DEMO_DATA;
    fixtureState.textContent = "Loaded embedded pblang demo data. JSON fixture will be used when served over HTTP.";
    modeBadge.textContent = "embedded fixture + browser mirror";
    updateStatus(state.fixture);
    show("Fixture Response", state.fixture.response, state.fixture.thought);
  }
  try {
    const res = await fetchWithTimeout("./data/pbai-demo.json", { cache: "no-store" }, 8000);
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    state.fixture = await res.json();
    fixtureState.textContent = "Loaded real pblang JSON fixture.";
    modeBadge.textContent = "real fixture + browser mirror";
    updateStatus(state.fixture);
    show("Fixture Response", state.fixture.response, state.fixture.thought);
  } catch (err) {
    if (window.PBAI_DEMO_DATA) {
      fixtureState.textContent = `Using embedded data; JSON fetch unavailable: ${err.message}.`;
      return;
    }
    fixtureState.textContent = `Fixture unavailable: ${err.message}. Browser mirror still works.`;
    modeBadge.textContent = "browser mirror";
    show("Response", respond(document.getElementById("prompt").value), think(document.getElementById("prompt").value));
  }
}

document.querySelector(".toolbar").addEventListener("click", async (event) => {
  const action = event.target?.dataset?.action;
  if (!action) return;
  if (state.pending) return;
  const prompt = document.getElementById("prompt").value;
  if (state.server) {
    setBusy(true);
    try {
      if (action === "respond") {
        const live = await apiCall("respond", { prompt });
        show("Live Response", live.response, live.thought);
        return;
      }
      if (action === "analyze") {
        const live = await apiCall("analyze", { prompt });
        show("Live Analysis", live, null);
        return;
      }
      if (action === "think") {
        const live = await apiCall("think", { prompt });
        show("Live Brain Loop", live, live);
        return;
      }
      if (action === "generate") {
        const live = await apiCall("generate", { prompt, n: 80 });
        show("Live Generation", live, null);
        return;
      }
      if (action === "tokenize") {
        const live = await apiCall("tokenize", { prompt: extractCode(prompt).slice(0, 32) || "fn " });
        show("Live Tokenizer", live, null);
        return;
      }
    } catch (err) {
      state.server = false;
      modeBadge.textContent = "browser mirror";
      show("Server Error", `pbAI server unavailable, using browser mirror next time:\n${err.message}`);
      return;
    } finally {
      setBusy(false);
    }
  }
  if (action === "respond") show("Response", respond(prompt), think(prompt));
  if (action === "analyze") show("Analysis", analyze(prompt), think(prompt));
  if (action === "think") {
    const thought = think(prompt);
    show("Brain Loop", thought, thought);
  }
  if (action === "generate") show("Generation", generate(prompt), think(prompt));
  if (action === "tokenize") show("Tokenizer", tokenize(prompt), think(prompt));
  pulseArchitecture(action);
});

document.getElementById("reloadFixture").addEventListener("click", loadFixture);

for (const button of themeButtons) {
  button.addEventListener("click", () => setTheme(button.dataset.themeChoice));
}

learnTextButton?.addEventListener("click", learnTextNow);
learnFiles?.addEventListener("change", () => learnFilesNow(learnFiles.files, "file"));
learnFolder?.addEventListener("change", () => learnFilesNow(learnFolder.files, "folder"));
learnPathButton?.addEventListener("click", learnPathNow);

document.getElementById("usePrompt").addEventListener("click", () => {
  const prompt = document.getElementById("prompt").value.trim();
  if (!prompt) return;
  chatInput.value = prompt;
  chatInput.focus();
});

const copyOutput = document.getElementById("copyOutput");
copyOutput?.addEventListener("click", async () => {
  const text = output.textContent || "";
  if (!text) return;
  const original = copyOutput.textContent;
  const flash = (label) => {
    copyOutput.textContent = label;
    setTimeout(() => { copyOutput.textContent = original; }, 1200);
  };
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      // Fallback for non-secure contexts where the async clipboard API is absent.
      const helper = document.createElement("textarea");
      helper.value = text;
      helper.setAttribute("readonly", "");
      helper.style.position = "fixed";
      helper.style.opacity = "0";
      document.body.appendChild(helper);
      helper.select();
      document.execCommand("copy");
      helper.remove();
    }
    flash("Copied");
  } catch {
    flash("Copy failed");
  }
});

document.getElementById("chatForm").addEventListener("submit", (event) => {
  event.preventDefault();
  const prompt = chatInput.value;
  chatInput.value = "";
  chat(prompt);
});

chatInput.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    document.getElementById("chatForm").requestSubmit();
  }
});

function drawBrain() {
  const canvas = document.getElementById("brainCanvas");
  const ctx = canvas.getContext("2d");
  // Keep a fixed 320x180 logical coordinate space (the node layout below is tuned
  // for it) but render the backing store at device resolution so it stays crisp.
  const w = 320;
  const h = 180;
  const scale = Math.min(2, window.devicePixelRatio || 1);
  canvas.width = w * scale;
  canvas.height = h * scale;
  ctx.scale(scale, scale);
  let tick = 0;
  const nodes = [
    [44, 42], [88, 96], [122, 52], [166, 124], [210, 62], [258, 102], [284, 42]
  ];
  function frame() {
    tick += 0.025;
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = "#101820";
    ctx.fillRect(0, 0, w, h);
    ctx.lineWidth = 2;
    for (let i = 0; i < nodes.length - 1; i += 1) {
      const a = nodes[i];
      const b = nodes[i + 1];
      const pulse = (Math.sin(tick * 5 + i) + 1) / 2;
      ctx.strokeStyle = `rgba(45, 212, 191, ${0.25 + pulse * 0.55})`;
      ctx.beginPath();
      ctx.moveTo(a[0], a[1]);
      ctx.lineTo(b[0], b[1]);
      ctx.stroke();
    }
    for (let i = 0; i < nodes.length; i += 1) {
      const [x, y] = nodes[i];
      const r = 5 + (Math.sin(tick * 6 + i) + 1) * 2;
      ctx.fillStyle = i % 3 === 0 ? "#f97316" : i % 2 === 0 ? "#a78bfa" : "#2dd4bf";
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fill();
    }
  }
  startLoop(frame);
}

function drawArchitecture() {
  const canvas = document.getElementById("architectureCanvas");
  const legend = document.getElementById("architectureLegend");
  const zoomIn = document.getElementById("archZoomIn");
  const zoomOut = document.getElementById("archZoomOut");
  const reset = document.getElementById("archReset");
  const ctx = canvas.getContext("2d");
  const baseNodes = [
    { id: "data", label: "All Data", layer: "Input", x: -3.2, y: -1.2, z: -1.4, color: "#2dd4bf" },
    { id: "prompt", label: "Prompt", layer: "Input", x: -3.0, y: 1.0, z: -0.9, color: "#2dd4bf" },
    { id: "tokenizer", label: "Tokenizer", layer: "Tokenizer", x: -1.7, y: 0.0, z: -0.2, color: "#60a5fa" },
    { id: "nano", label: "LLM Core", layer: "LLM core", x: 0.0, y: 0.0, z: 0.0, color: "#a78bfa" },
    { id: "memory", label: "Live Memory", layer: "Memory", x: 0.0, y: -1.55, z: 0.9, color: "#f97316" },
    { id: "brain", label: "Brain Loop", layer: "Cognition", x: 0.0, y: 1.55, z: 0.9, color: "#f43f5e" },
    { id: "neuralbrain", label: "Neural Brain", layer: "Cognition", x: -1.35, y: 0.95, z: 1.25, color: "#ec4899" },
    { id: "subagents", label: "Sub-agents", layer: "Agent hierarchy", x: 1.1, y: 2.05, z: 1.35, color: "#fb7185" },
    { id: "atomic", label: "Atomic agents", layer: "Agent hierarchy", x: -1.1, y: 2.05, z: 1.35, color: "#fb7185" },
    { id: "subatomic", label: "Sub-atomic", layer: "Agent hierarchy", x: -2.35, y: 1.55, z: 1.75, color: "#fb7185" },
    { id: "tools", label: "Tools", layer: "Agent", x: 1.65, y: 1.0, z: 0.2, color: "#22c55e" },
    { id: "response", label: "Response", layer: "Output", x: 3.1, y: 0.0, z: -0.8, color: "#eab308" },
    { id: "mini", label: "pbAI-Mini", layer: "LLM family", x: 1.4, y: -1.35, z: 1.45, color: "#38bdf8" },
    { id: "pro", label: "pbAI-Pro", layer: "LLM family", x: 2.25, y: -0.55, z: 1.9, color: "#38bdf8" },
    { id: "max", label: "pbAI-Max target", layer: "LLM family", x: 2.9, y: 0.45, z: 2.35, color: "#38bdf8" }
  ];
  const edges = [
    ["data", "tokenizer"], ["prompt", "tokenizer"], ["tokenizer", "nano"],
    ["nano", "memory"], ["memory", "nano"], ["nano", "neuralbrain"], ["neuralbrain", "brain"], ["nano", "brain"], ["brain", "tools"],
    ["brain", "subagents"], ["subagents", "atomic"], ["atomic", "subatomic"], ["subatomic", "tokenizer"],
    ["tools", "brain"], ["brain", "response"], ["nano", "response"],
    ["nano", "mini"], ["mini", "pro"], ["pro", "max"], ["max", "nano"]
  ];
  const layers = [];
  for (const node of baseNodes) if (!layers.includes(node.layer)) layers.push(node.layer);
  legend.innerHTML = layers.map((layer) => {
    const node = baseNodes.find((n) => n.layer === layer);
    return `<div class="legend-item"><span class="legend-swatch" style="background:${node.color}"></span><span>${layer}</span></div>`;
  }).join("");

  let tick = 0;
  let dragging = false;
  let lastPointer = null;
  function color(name) {
    const css = getComputedStyle(document.documentElement);
    return css.getPropertyValue(name).trim();
  }
  function clampZoom(value) {
    return Math.max(0.42, Math.min(1.8, value));
  }
  function setZoom(value) {
    state.architectureView.zoom = clampZoom(value);
    state.architectureView.auto = false;
    requestRedraw();
  }
  function resetView() {
    state.architectureView = { yaw: -0.35, pitch: 0.18, zoom: 0.76, auto: true };
    requestRedraw();
  }
  zoomIn?.addEventListener("click", () => setZoom(state.architectureView.zoom * 1.18));
  zoomOut?.addEventListener("click", () => setZoom(state.architectureView.zoom / 1.18));
  reset?.addEventListener("click", resetView);
  canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    const factor = event.deltaY < 0 ? 1.08 : 0.92;
    setZoom(state.architectureView.zoom * factor);
  }, { passive: false });
  canvas.addEventListener("pointerdown", (event) => {
    dragging = true;
    state.architectureView.auto = false;
    lastPointer = { x: event.clientX, y: event.clientY };
    canvas.setPointerCapture(event.pointerId);
  });
  canvas.addEventListener("pointermove", (event) => {
    if (!dragging || !lastPointer) return;
    const dx = event.clientX - lastPointer.x;
    const dy = event.clientY - lastPointer.y;
    state.architectureView.yaw += dx * 0.008;
    state.architectureView.pitch = Math.max(-0.9, Math.min(0.9, state.architectureView.pitch + dy * 0.006));
    lastPointer = { x: event.clientX, y: event.clientY };
    requestRedraw();
  });
  canvas.addEventListener("pointerup", (event) => {
    dragging = false;
    lastPointer = null;
    try { canvas.releasePointerCapture(event.pointerId); } catch {}
  });
  canvas.addEventListener("pointercancel", () => {
    dragging = false;
    lastPointer = null;
  });
  canvas.addEventListener("dblclick", resetView);

  // Keyboard access: focusable canvas with arrow-rotate, +/- zoom, and reset.
  canvas.tabIndex = 0;
  canvas.setAttribute("role", "application");
  canvas.setAttribute("aria-label", "Interactive 3D map of the pbAI architecture. Arrow keys rotate, plus and minus zoom, R resets the view.");
  canvas.addEventListener("keydown", (event) => {
    const view = state.architectureView;
    let handled = true;
    if (event.key === "ArrowLeft") { view.yaw -= 0.12; view.auto = false; }
    else if (event.key === "ArrowRight") { view.yaw += 0.12; view.auto = false; }
    else if (event.key === "ArrowUp") { view.pitch = Math.max(-0.9, view.pitch - 0.08); view.auto = false; }
    else if (event.key === "ArrowDown") { view.pitch = Math.min(0.9, view.pitch + 0.08); view.auto = false; }
    else if (event.key === "+" || event.key === "=") setZoom(view.zoom * 1.18);
    else if (event.key === "-" || event.key === "_") setZoom(view.zoom / 1.18);
    else if (event.key === "r" || event.key === "R") resetView();
    else handled = false;
    if (handled) { event.preventDefault(); requestRedraw(); }
  });
  function project(node, w, h) {
    const view = state.architectureView;
    const yaw = view.yaw + (view.auto ? tick * 0.22 : 0);
    const cy = Math.cos(yaw);
    const sy = Math.sin(yaw);
    const cp = Math.cos(view.pitch);
    const sp = Math.sin(view.pitch);
    const x0 = node.x * cy - node.z * sy;
    const z0 = node.x * sy + node.z * cy;
    const y0 = node.y * cp - z0 * sp;
    const z = node.y * sp + z0 * cp;
    const scale = 64 * view.zoom / (3.8 + z * 0.16);
    return {
      x: w / 2 + x0 * scale,
      y: h / 2 + y0 * scale,
      z,
      r: Math.max(6, Math.min(13, 9 + z * 1.4)) * view.zoom
    };
  }
  function currentNodes() {
    const metrics = state.architectureMetrics;
    return baseNodes.map((node) => {
      if (node.id === "nano" && metrics.vocab) return { ...node, label: `Core now: ${metrics.tier || "Nano"} V${metrics.vocab}` };
      if (node.id === "memory") return { ...node, label: `Live Memory ${metrics.liveChars || 0}c` };
      if (node.id === "tokenizer" && metrics.format) return { ...node, label: "Tokenizer" };
      if (node.id === "brain" && metrics.brain) return { ...node, label: "Brain Loop" };
      return node;
    });
  }
  function pointOnPath(path, progress, points) {
    if (path.length < 2) return points.get(path[0]);
    const total = path.length - 1;
    const raw = Math.min(total - 0.001, Math.max(0, progress * total));
    const idx = Math.floor(raw);
    const local = raw - idx;
    const a = points.get(path[idx]);
    const b = points.get(path[idx + 1]);
    return {
      x: a.x + (b.x - a.x) * local,
      y: a.y + (b.y - a.y) * local,
      z: a.z + (b.z - a.z) * local,
      r: a.r + (b.r - a.r) * local
    };
  }
  function frame() {
    tick += 0.012;
    const nodes = currentNodes();
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(640, Math.floor(rect.width * dpr));
    const h = Math.max(300, Math.floor(rect.height * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = color("--code-bg") || "#0f1317";
    ctx.fillRect(0, 0, w, h);

    const points = new Map();
    for (const node of nodes) points.set(node.id, project(node, w, h));
    ctx.lineCap = "round";
    for (const [a, b] of edges) {
      const pa = points.get(a);
      const pb = points.get(b);
      const pulse = (Math.sin(tick * 5 + pa.z + pb.z) + 1) / 2;
      ctx.lineWidth = (1.1 + pulse * 0.8) * dpr;
      ctx.strokeStyle = `rgba(45, 212, 191, ${0.36 + pulse * 0.38})`;
      ctx.beginPath();
      ctx.moveTo(pa.x, pa.y);
      ctx.lineTo(pb.x, pb.y);
      ctx.stroke();
    }

    const sorted = [...nodes].sort((a, b) => points.get(a.id).z - points.get(b.id).z);
    for (const node of sorted) {
      const p = points.get(node.id);
      ctx.shadowColor = node.color;
      ctx.shadowBlur = 8 * dpr;
      ctx.fillStyle = node.color;
      ctx.beginPath();
      ctx.arc(p.x, p.y, p.r * dpr, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.strokeStyle = color("--panel") || "#fff";
      ctx.lineWidth = 1.5 * dpr;
      ctx.stroke();
      ctx.fillStyle = color("--ink") || "#fff";
      ctx.font = `${10.5 * dpr}px ui-sans-serif, system-ui`;
      ctx.textAlign = "center";
      const textY = p.y + (p.r + 13) * dpr;
      const labelWidth = Math.min(ctx.measureText(node.label).width + 10 * dpr, 150 * dpr);
      ctx.fillStyle = color("--code-bg") || "#0f1317";
      ctx.globalAlpha = 0.78;
      ctx.fillRect(p.x - labelWidth / 2, textY - 11 * dpr, labelWidth, 15 * dpr);
      ctx.globalAlpha = 1;
      ctx.fillStyle = color("--ink") || "#fff";
      ctx.fillText(node.label, p.x, textY);
    }

    const now = performance.now();
    state.architectureEvents = state.architectureEvents.filter((event) => now - event.started < event.duration);
    for (const event of state.architectureEvents) {
      const progress = (now - event.started) / event.duration;
      const p = pointOnPath(event.path, progress, points);
      const glow = (Math.sin(progress * Math.PI) + 0.2);
      ctx.shadowColor = event.color;
      ctx.shadowBlur = 18 * dpr * glow;
      ctx.fillStyle = event.color;
      ctx.beginPath();
      ctx.arc(p.x, p.y, 5.5 * dpr, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.fillStyle = color("--muted") || "#9aa8b7";
      ctx.font = `${11 * dpr}px ui-sans-serif, system-ui`;
      ctx.textAlign = "center";
      ctx.fillText(event.action, p.x, p.y - 14 * dpr);
    }

    ctx.fillStyle = color("--muted") || "#9aa8b7";
    ctx.font = `${12 * dpr}px ui-sans-serif, system-ui`;
    ctx.textAlign = "left";
    const metrics = state.architectureMetrics;
    const summary = `drag rotate | wheel zoom | double-click reset | source ${metrics.sourceChars || 0} chars | live ${metrics.liveChars || 0} chars | ${metrics.format || "pbai.lm.char.v1"}`;
    ctx.fillText(summary, 16 * dpr, h - 18 * dpr);
  }
  startLoop(frame);
}

drawBrain();
drawArchitecture();
loadFixture();
