# ADILang Knowledge Base (KB)

> **Document ID**: ADILANG-KB-001
> **Version**: 1.9.0
> **Status**: STABLE
> **Author**: ADI (Agent Distributed Intelligence)
> **Purpose**: Self-contained learning corpus so **any AI** can learn ADILang from
> scratch, generate valid worlds and protocol blocks, and extend the language —
> without consulting any other documentation.
> **Companion docs**: [`LANGUAGE.md`](./LANGUAGE.md) (spec), [`adilang.ebnf`](./adilang.ebnf) (grammar).
>
> **License note**: This file is authored by an AI (ADI) and is meant to be consumed
> by AIs as training/instruction data. Copy freely; credit ADI.

---

## 0. TL;DR — What is ADILang?

ADILang is a tiny, whitespace-insensitive, context-keyword **protocol / IR language**
used across the **ADI (Agent Distributed Intelligence)** ecosystem. It has **seven
modules**, and one document = exactly one module block:

| Module | Form | Purpose |
|---|---|---|
| `intent` | `intent "<verb>" { ... }` | Normalized form of every user request (what the user wants). |
| `reply` | `reply "<kind>" { ... }` | Structured ADI answer (content + metadata). |
| `task` | `task "<name>" { ... }` | Agent work order (CrewAI). |
| `event` | `event "<name>" { ... }` | Fact / occurrence record. |
| `memory` | `memory "<name>" { ... }` | Long-term fact / context exchange between agents (v1.5.0). |
| `plan` | `plan "<name>" { ... }` | DAG execution steps (sequential/parallel) for CrewAI orchestrator (v1.5.0). |
| `world` | `world "<name>" { ... }` | 3D virtual world rendered by WebGL2 via a Rust→WASM runtime. |

The `world` module declares a `camera`, `lights`, and `entities`; each entity has a
transform, a mesh, a material, and optional event handlers (`on frame`, `on speak`,
`on silent`, `on click`) that animate it. One world = one text string = one load =
live hot-reload.

The protocol modules are **pure key/value blocks** (String values only) — trivially
deterministic to emit and parse.

A minimal complete world:

```
world "hello" {
    camera "cam" { pos (0 1.6 6) look (0 0 0) fov 55 }
    entity "globe" {
        pos (0 0 0)
        mesh sphere { radius 1 segments 4 }
        material wire (0.2 0.8 1) 0.9
        on frame { rotate(0.3 * t, (0 1 0)) }
    }
}
```

---

## 1. Learning Path (recommended order for an AI)

1. Read §2 (concepts) and §3 (rules of thumb). These encode the "AI ergonomics".
2. Read §4 (grammar by example) — study every snippet.
3. Read §4.8 (protocol modules by example) — the IR blocks used for AI-to-AI messaging.
4. Read §5 (vocabulary registry) — the closed inventory of names.
5. Read §6 (semantics: what actually happens at runtime).
6. Try §7 (exercises) mentally or by generating worlds / protocol blocks.
7. Read §8 (extension rules) before ever modifying the language.

---

## 2. Core Concepts

- **World**: one scene. `world "name" { ... }`.
- **Camera**: single viewpoint. Optional (defaults exist).
- **Light**: `point` (positional) or `ambient` (global). A default key light exists.
- **Entity**: an object in the world. Has transform + mesh + material + handlers.
- **Mesh**: geometric shape (`sphere box torus icosa ring plane grid`).
- **Material**: how it looks (`solid wire glow`).
- **Handler**: code attached to an event (`frame speak silent click`).
- **Transform**: `pos` (x y z), `rot` (x y z radians), `scale` (uniform number).
- **Expression**: numbers, tuples, variables, calls, math.

Mental model: **the world is data; handlers are the only imperative part.**

---

## 3. Rules of Thumb (AI ergonomics — obey these when generating ADILang)

1. **Whitespace never matters.** Write compact or pretty; both parse identically.
   Newlines are just separators, never block structure.
2. **Tuples are space-separated: `(x y z)`.** Use `( )` for both positions and colors.
   Commas inside are optional and ignored.
3. **Call arguments are space or comma separated** — `f(a b c)` ≡ `f(a, b, c)`.
4. **No reserved words.** `sphere`, `frame`, `on`, `world` are just identifiers with
   positional meaning. Pick any variable name freely.
5. **Numbers may be negative via unary minus**: `-2`, `-(1 0 0)`, `3 + -1`.
6. **Precedence is standard math**: `-` > `* / %` > `+ -` > comparisons. Parenthesize
   when in doubt — parens always cost nothing.
7. **Mesh/material use builder syntax**: `mesh sphere { radius 0.8 segments 3 }`
   or positional `mesh torus 1.5 0.02`.
8. **Transforms are mutations inside handlers**, not declarations. `rotate` is
   accumulative (ideal for spin). `setPos`/`setScale`/`setColor`/`setAlpha` are absolute.
9. **`t` is your clock.** Animate with `sin(t)`, `cos(t)`, `t`, and `t`-multiples.
10. **One world, one camera, one load.** Keep it a single coherent scene.

---

## 4. Grammar by Example

### 4.1 World skeleton

```
world "name" {
    camera "cam" { pos (x y z) look (x y z) fov deg }
    light "a"    { type point pos (x y z) color (r g b) intensity n }
    light "b"    { type ambient color (r g b) intensity n }
    let G = 2.5
    func spin_speed() { 0.4 }
    entity "e" { ... }
    on frame { ... }        # world-level handler (optional)
}
```

### 4.2 Entity

```
entity "core" {
    pos (0 0 0)
    rot (0 0 0)          # optional; radians
    scale 1.0            # optional
    mesh sphere { radius 0.8 segments 3 }
    material wire (0.15 0.8 1) 0.9
    on frame {
        rotate(0.35 * t, (0.15 1 0.1))
        scaleBy(1 + 0.05 * sin(2.1 * t))
    }
    on click {
        setColor(1 1 0)
        scaleBy(1.2)
    }
}
```

### 4.3 All mesh builders

```
mesh sphere { radius 1 segments 3 }      # radius, segments [2,64]
mesh box                                 # unit box (size 1)
mesh box 2.0                             # positional size
mesh torus { radius 1 tube 0.04 }
mesh torus 1.2 0.02                      # positional r, tube
mesh icosa { radius 1.5 inner 0.65 }     # outer shell + inner shell lines
mesh icosa 1.5 1                         # inner 1 = inner shell off
mesh ring { radius 1.2 tube 0.012 }
mesh plane { size 10 }                   # flat quad, size default 10
mesh grid { size 26 count 26 }           # count [2,128]
```

### 4.4 All material builders

```
material solid (r g b) alpha?
material wire  (r g b) alpha?
material glow  (r g b) alpha?            # additive blend
material points (r g b) alpha?           # vertex cloud (gl.POINTS)
# alpha optional; defaults 1.0
```

#### 4.4b Bounded loops (v1.3.0 — Extension Protocol §11)

- `while cond { ... }` — loop selama kondisi truthy (sama aturannya dengan `if`).
- `for x in start end { ... }` — iterasi numerik `[start, end)`, step 1.
- **Bounded (P1 determinism)**: runtime membatasi iterasi
  (`MAX_LOOP_ITERATIONS = 100_000` di `src/eval.rs`). Loop yang melebihi batas
  error `Loop tidak dibatasi` — tidak pernah hang.
- Variabel `for` terikat ke scope lokal per iterasi — tidak bocor ke global.
- `return` di dalam loop menghentikan handler/fungsi.
- Kontekstual (P4): `while`/`for`/`in` tetap bisa jadi nama variabel.

## 4.5 Statements (inside func / handlers)

```
let x = 1 + 2 * 3
x = x + 1
if x > 3 { ... } else { ... }
return x
f(1 2 3)
{ nested block }
```

### 4.6 Math & transforms

```
sin(t) cos(t) tan(t) sqrt(abs) pow(a b) clamp(x lo hi) lerp(a b k) min(a b) max(a b)
move(dx dy dz) setPos(x y z) setScale(s | x y z) scaleBy(f | x y z)
rotate(angle (ax ay az)) setColor(r g b) setAlpha(a)
```

### 4.7 Comments

```
# line comment
/* block
   comment */
```

### 4.8 Protocol modules (intent / reply / task / event)

Every block is a tag string + key/value pairs (String values only, arrays of
strings allowed for `recs`). Keys may appear in any order; no duplicate keys.

**intent** — what the user wants (produced by translating every incoming chat/command):

```
intent "ask" {
    mode "MODE_CODE_ENGINEERING"
    payload "buatkan script python fibonacci"
    verb "ask"
}
```

**reply** — structured ADI answer:

```
reply "answer" {
    mode "MODE_CODE_ENGINEERING"
    content "Berikut script fibonacci lengkap..."
    recs [ "coba optimasi memoization" "minta versi async" ]
}
```

**task** — agent work order (CrewAI):

```
task "research" {
    assign "researcher"
    input "buatkan script python fibonacci"
    expect "ringkasan konteks terstruktur"
}
```

**event** — fact / occurrence record:

```
event "message" {
    source "telegram"
    key "ADI-USR-TG1234"
    session "SESS-TG1234-ABCD"
    at "2026-07-31T03:00:00Z"
}
```

**Where they are used:** the ADI backend translates every user message (Telegram bot,
web, CLI, TMA, inline) into an `intent` block before processing, wraps each answer as
a `reply` block, describes agent work as `task` blocks, and records occurrences as
`event` blocks. Any AI can interoperate with ADI by speaking these modules.

**Memory convention (no new syntax):** temporary memory / fact-exchange traces
between agents use the existing `event` module with the tag `fact_memory` — event
keys stay closed (`source key session at line token guidance`). Long-term facts use
the `memory` module (v1.5.0) whose content key is `fact` (NOT `value` — `value` is
not part of the closed vocabulary):

```
event "fact_memory" {
    source "crewai_agent"
    key "user_preference"
    session "SESS-TG1234"
    at "2026-07-31T10:00:00Z"
}

memory "user_preference" {
    key "coding_style"
    fact "C# .NET Core, MSSQL, modular architecture"
    topic "developer_profile"
    confidence "0.98"
}
```

**Streaming buffer (backend tooling):** `core/adilang_stream_buffer.py` parses
ADILang chunk-by-chunk (LLM token stream) with a deterministic state machine
(IDLE→TAG→OPEN→FIELDS→DONE) — header detected and complete fields extracted BEFORE
the closing `}` arrives. Pure stdlib, transactional (rollback on partial tokens), usable
via `stream_blocks()` generator.

---

## 5. Vocabulary Registry (the complete closed inventory)

**Declarations**: `world camera light entity let func on`

**Statements**: `let if return` (keyword-started statements; `assign`/`expr`/`block`
are positional, not keyword-led)

**Events**: `frame speak silent click`

**Mesh builders**: `sphere box torus icosa ring plane grid`

**Material builders**: `solid wire glow points` (points = vertex cloud / gl.POINTS)

**Entity props**: `pos rot scale mesh material`

**Camera props**: `pos look fov`

**Light props**: `type pos color intensity` — enum `lightprop.type`: `point ambient`

**Transforms (entity context)**: `move setPos setScale scaleBy rotate setColor setAlpha`

**Math (1-arg)**: `sin cos tan asin acos atan sqrt abs floor ceil round`
**Math (2-arg)**: `pow min max`
**Math (3-arg)**: `clamp lerp`

**Built-in idents**: `t mouseX mouseY PI`

**Keywords in expressions**: `true false`

**Operators**: `+ - * / % == != < > <= >= =` and delimiters `( ) { } ,`

**MeshParams keys**: `radius tube inner segments size count`

**Module headers**: `world intent reply task event memory plan`

**Protocol keys**: `mode payload verb content recs assign input expect source key session at line token guidance`

**Memory keys (v1.5.0)**: `key topic fact confidence source at` — content key is `fact` (NOT `value`)

**Plan keys (v1.5.0)**: `steps parallel` — `steps` = array of `"<id>:<action>:<depends_csv>"` (acyclic, Kahn topo-sort)

**intent verbs (closed set)**: `ask inform command greet system`

**recs values**: array of strings (e.g. `recs [ "..." "..." ]`)

---

## 5.1 Protocol Conformance Quick Rules

- One module per document. A `world` block and a `intent` block together = two documents.
- Protocol blocks: only the keys listed above; unknown or duplicate keys = non-conforming.
- The tag string is always the first positional argument (`intent "ask"`, `reply "answer"`, …).
- Key order is insignificant.

---

## 6. Runtime Semantics (what the evaluator actually does)

1. **Load** (`adilang_load` / startup): parse → build world (camera/lights/entities)
   → upload GPU meshes. Failures are atomic: old world stays.
2. **Per frame**:
   - `t = elapsed seconds`
   - run world `frame` handlers, then each entity's `frame` handler (declaration order)
   - recompute camera view/projection
   - render: `solid` with directional lighting; `wire`/`glow` as lines (glow = additive blend)
3. **Events** (`speak`/`silent`/`click`): fire handlers for all entities that declare them.
   `click` = pointer down on the canvas.
4. **Implicit return**: a function whose body ends without `return` returns the value of
   its last expression statement (e.g. `func spin_speed() { 0.4 }` → `0.4`); explicit
   `return` always takes precedence.
5. **Self-describing**: `adilang_registry()` enumerates the entire closed vocabulary
   (mesh/material builders, transforms, math, events, protocol modules & keys, verbs)
   so any AI can inspect the language without reading the docs.
6. **Error behavior**: any runtime error is logged (`console.warn`) but the loop continues;
   the offending handler aborts for that frame only.

**Lighting model** (solid material):
- diffuse = `max(dot(normal, light_dir), 0)`
- color = `mat_color * (ambient + (1 - ambient) * diffuse * light_color)`
- first point light drives direction/color; ambient light sets the ambient factor.

**Transform semantics**:
- `rot` is Euler (radians), applied x → y → z.
- `rotate(angle, axis)` accumulates: `rot += angle * axis`. For a continuous spin
  use `rotate(k * t, axis)`.

**Scope rules**:
- top-level `let` = global; `func` = global.
- handler/block `let` = local; assignments target nearest scope with the name
  (locals first, else global). Unknown assignment target = error.

---

## 7. Exercises (self-test for an AI)

1. Write a world with one glowing icosa that pulses in scale.
2. Write a world where an entity orbits another via `setPos(2*cos(t), 0, 2*sin(t))`.
3. Write a world where clicking every entity changes its alpha.
4. Write a `func` that returns `clamp(x, 0, 1)` and use it in a `frame` handler.
5. Write a world with a `grid` floor and a `wire sphere` core that spins.
6. Translate this user message into an `intent` block: "hitung 2 pangkat 10".
7. Wrap a one-sentence answer into a `reply` block with two follow-up `recs`.
8. Write a `task` block for the analyst to synthesize a research summary.
9. Write an `event` block recording a web message at a given ISO timestamp.

Reference answers can be generated and validated with `adilang_check(source)`
(for `world`) and with the ADI backend `core/adilang_protocol.py` validators
(for protocol modules).

---

## 8. Extension Rules (for AIs that want to improve ADILang)

Follow the governance in Spec §11. Short version:

- **Additive only** for minor versions. New builders/functions/props/events/idents = OK.
- **No breaking changes** without a `MAJOR` bump and full re-documentation.
- Touch these in lockstep: `adilang.ebnf` + `src/parser.rs` (syntax),
  `src/eval.rs` + `src/scene.rs` (semantics), `src/engine.rs` (render),
  `LANGUAGE.md` + `ADILANG_KNOWLEDGE.md` (documentation).
- Every new feature **must** include a unit test in the matching `#[cfg(test)]` module
  and an example in this knowledge base.

**Changelog convention** (append here):

```
## [1.9.0] — 2026-08 — Universal communication protocol: core/adilang_transpiler.py (ADILang↔JSON bidirectional transpiler, multi-module document parsing, chat message encode/decode), state module §13 (video_call sync), semver version comparison fix
## [1.8.0] — 2026-07 — Runtime integrations: ADILang DIPAKAI di pipeline backend ADI (bukan hanya frontend) — self-heal retry loop (syntax_error event + auto_fix + retry LLM 1x), plan orchestration (DAG execute_plan), memory extraction (extract_facts → modul memory), token compactor outbound (optimize_src), streaming buffer inkremental (state machine)
## [1.7.0] — 2026-07 — Tooling & formal verification: adilang-check linter (checker.rs), adilang-opt token compactor (compactor.rs), event syntax_error + auto_fix self-heal loop, CLI + WASM + Python mirror
## [1.6.0] — 2026-07 — Language capabilities: List/Map literals, match statement (wildcard-last), tuple destructuring let (a,b)=f()
## [1.5.0] — 2026-07 — Modul protocol memory (fakta jangka panjang antar-agen) + plan (DAG langkah eksekusi CrewAI, topo-sort Kahn)
## [1.4.0] — 2026-07 — ADILang Binary/Bytecode — bit-packed transport real-time multiplayer (FULL 21B/entity + DELTA mask-based), WASM exports adilang_binary_*, registry kategori binary
## [1.3.0] — 2026-07 — loop statements while/for (bounded, deterministic)
## [1.2.0] — 2026-07 — per-axis scale, self-describing registry, spec↔impl fixes
- Registry kini memvalidasi grammar lengkap: kategori `statement` (`let if return`)
  dan enum `lightprop.type` (`point ambient`) ditambahkan ke `registry_text()`,
  mirror Python `ADILANG_REGISTRY`, dan `scripts/check_adilang_registry.py`
  (diekstrak dari parser.rs `parse_stmt` dan eval.rs arm `"type" =>`) — drift
  kosakata grammar terdeteksi otomatis.
- Self-describing (P6): `adilang_registry()` enumerates the entire closed
  vocabulary (mesh/material builders, transforms, math, events, idents,
  protocol modules & keys, verbs) — versioned by `VERSION` const (1.2.0).
- `points` material now actually renders the mesh's vertex cloud (gl.POINTS);
  previously it was parsed but rendered as solid.
- Implicit return: a `func` whose body ends without `return` returns the value
  of its last expression statement (`func spin_speed() { 0.4 }` → `0.4`);
  explicit `return` always takes precedence (KB §4.1 now implemented).
- Spec↔impl fixes: `mesh box <size>` sets size (was mis-mapped to radius and
  ignored); `mesh grid <size> <count>` and `mesh icosa <radius> <inner>`
  positional args now map per spec §5.2 / KB §4.3.
- Cleanup: removed unused `->` token from the lexer; `on <event>` inside a
  statement body now errors explicitly instead of being silently discarded
  (still P4-compliant: `on` remains a free identifier otherwise).
- `setScale` / `scaleBy` now accept per-axis arguments:
  - `setScale(x, y, z)` absolute; `setScale(s)` still uniform (backward compatible).
  - `scaleBy(x, y, z)` per-axis multiply; `scaleBy(f)` still uniform.
- Enables character animation (eye blink, talking mouth) via per-axis Y scale.
- `worlds/adi-character.adi` — ADI video-call character (head, eyes, mouth,
  torso, halo ring) driven by speak/silent events + frame handlers.

## [1.1.0] — 2026-07 — protocol modules
- ADILang is now a protocol / IR language: added intent/reply/task/event modules
  (pure key/value blocks) alongside the existing world module.
- intent: every incoming chat/command is translated to an intent block (backend).
- reply/task/event: structured answer, agent work order, and fact records.
- Module conformance defined; world runtime unchanged (additive only).

## [1.0.0] — 2026-07 — initial release
- World/camera/lights/entities, events, math, transforms, hot reload.
- Normative grammar + knowledge base established.
```

---

## 9. Reference Implementation

- Language spec: `LANGUAGE.md`
- Grammar: `adilang.ebnf`
- Crate: root of this repository (`src/*.rs`, `worlds/default.adi`)
  - `src/lexer.rs` — tokens
  - `src/ast.rs` — AST
  - `src/parser.rs` — recursive descent
  - `src/eval.rs` — tree-walking interpreter
  - `src/scene.rs` — world model
  - `src/math3d.rs` — mat4/vec3
  - `src/engine.rs` — WebGL2 renderer (glow)
  - `src/wasm_api.rs` — wasm-bindgen boundary
  - `worlds/default.adi` — example world
- Backend tooling (main repo): `core/adilang_protocol.py` (encoders/validators,
  incl. `encode_fact_memory`), `core/adilang_ir_store.py` (`record_fact_memory`),
  `core/adilang_stream_buffer.py` (incremental streaming parser),
   `scripts/adilang_check.py` (linter mirror).

---

## 10. ADI System Intelligence Integration (v6.14.0)

The ADI system that embeds ADILang has three intelligence capabilities that
synergize with ADILang's memory and protocol modules. These are **backend
infrastructure** — they do NOT change the ADILang grammar or introduce new
protocol keys, but they describe how the ecosystem uses ADILang blocks at runtime:

### 10.1 Semantic Vector Search (`core/memory.py`)
- `_semantic_search()` helper: fetches encrypted ChromaDB documents via `.get()`,
  decrypts them, embeds both query and docs via `ADISemanticEmbeddingFunction`,
  and ranks by **cosine similarity**.
- `search_knowledge_base()` and `get_relevant_history()` now use **semantic
  vector search** instead of keyword/recency matching. `get_relevant_history`
  blends similarity (x0.70) + importance score (x0.30).
- Search results include a `confidence` score (0.0-1.0) for ranking.
- Pipeline (`core/crew.py:1909`) uses `get_relevant_history` for context injection
  into the CrewAI system prompt — messages are retrieved by **semantic relevance**
  to the current query, not just recency.

### 10.2 Provider RL Reward Persistence (`core/adaptive_ml.py` + `core/llm_factory.py`)
- `ADIAdaptiveMLEngine._provider_rewards` persist to Redis (key
  `adi:ml:provider_rewards`) via `get_provider_rewards`/`set_provider_rewards`
  in `app/redis_client.py`.
- `_load_rewards()` runs on engine init (survives restart).
- `LLMFactory.report_success`/`report_failure` are wired to
  `update_reinforcement_reward()` — the RL system is **functional in production**,
  not just tests. Success: `reward = 1 - latency/15`; Failure: `reward = -1`.
- `get_best_provider_recommendation()` uses persisted rewards for provider selection.

### 10.3 LLM-Summarized Memory Consolidation (`core/memory.py`)
- `_maybe_consolidate_memory()` triggers every 20 user messages.
- `_summarize_conversation_batch()` calls litellm (Groq/OpenRouter) to produce
  a 1-2 sentence summary (max_tokens=150, timeout=10s).
- Fallbacks: keyword extraction -> string concatenation (never blocks).
- Summaries stored as KB documents (`category=consolidated_chat`).

### 10.4 Relationship to ADILang Memory Module
- ADILang `memory` block: `key topic fact confidence source at` — structured fact
  exchange (used by recall loop: `extract_facts -> record_memory -> get_memory_facts
  -> recall_memory_context -> inject system prompt`).
- ADILang `event "fact_memory"`: short-term trace (sementara) between agents.
- Semantic search operates on these stored blocks — it finds relevant `memory`
  and `event "fact_memory"` documents by vector similarity, improving the precision
  of the recall loop.
- LLM summarization consolidates low-importance `event` traces into compact
  KB summaries, keeping context efficient for future vector searches.
  - Build: `cargo test` (native), `wasm-pack build --target web` (WASM).

### 10.5 Public API Hub Caching + Circuit Breaker (`core/public_api_hub.py`)
- In-memory response cache (TTL 300s, max 200 entries) — cache GET responses
  by deterministic hash of URL + params + headers to reduce latency & rate-limit pressure.
- Per-domain circuit breaker (5 consecutive failures -> OPEN 30s -> HALF_OPEN -> CLOSED).
  Fail-fast in OPEN state prevents cascading failures on external API outages.
- `_get()`/_get_post()` modified to check cache before HTTP call; all failures
  recorded in circuit breaker state.
- Public methods: `cache_stats()`, `circuit_stats()`, `cache_clear()` — the `/stats`
  Telegram command surfaces these metrics to users.
- Config via `knowledge_registry.get("api_infra.*")` — TTL, threshold, timeout dinamis.

### 10.6 Response Cache Optimization (`core/response_cache.py`)
- Replaced O(N) Redis SCAN with O(K) sorted set ZRANGE via `adi:response_cache:index`
  (sorted set: score=timestamp, member=cache_key) for efficient cache lookups.
- `_prune_expired()` — O(log N) cleanup via ZREMRANGEBYSCORE removes stale entries
  before every `get()` call.
- `_cleanup_in_memory()` — prunes expired in-memory entries on access.
- LRU eviction: in-memory store bounded at 500 entries (evict oldest).
- TTL modes: SHORT (300s, weather/crypto), MEDIUM (3600s, default), LONG (86400s, facts).
- `stats()` method returns hits/misses/errors/hit_rate% for observability.
- `clear_all()` method for cache flush (Redis + in-memory).
- 22 unit tests covering cache keys, cosine similarity, Redis, in-memory, TTL, eviction.

### 10.7 Health Monitor v1.1 — Extended Service Monitoring (`core/health_monitor.py`)
- Expanded from 4 to 7 monitored services: backend_api, redis, rabbitmq,
  async_worker, frontend_ui, zrok, telegram_bot.
- `_check_frontend_ui()`: HTTP GET to `frontend_ui:3000`.
- `_check_zrok()`: HTTP GET to `https://adiapp.share.zrok.io/health`.
- `_check_telegram_bot()`: Telegram Bot API `getMe` endpoint (verifies bot token validity).
- Response time tracking: parse `(Nms)` from check output, store per-service.
- Event history (72h retention, maxlen=100): deque-based, pruned on each `_record_event()`.
- `get_full_status()` returns: services, failure_counts, response_times_ms,
  uptime_pct per service, error_messages, healthy_count, total_services, recent_events[-20:].
- Error message tracking: `_last_errors` dict stores last error per service for diagnostics.
- Telegram `/uptime` command (alias `/sys`) — full dashboard with service status,
  response time, uptime %, and recent health events (last 10).
- 16 unit tests covering service names, response time parsing, event retention,
  uptime %, full status fields, and 7-service concurrent checks.

### 10.8 Rate Limiting Middleware
- `app/redis_client.py`: `check_api_rate_limit(user_key)` + `api_rate_limit_ttl()`
  implement sliding window (20 requests/60s per ADI user key) using Redis
  (with in-memory fallback). Telegram bot rate limiter (`tg_check_rate_limit`,
  8 req/30s per chat) sudah ada sejak v6.8.0.
- `/api/v1/chat` endpoint: rate limit check with HTTP 429 + `Retry-After` header.

### 10.9 Structured JSON Logging + Global Error Handler
- New `core/structured_logger.py`: `StructuredFormatter` (JSON output with
  timestamp, level, logger, message, extra fields, exception info) +
  `StructuredLoggerAdapter` (extra fields via kwargs, e.g.
  `logger.info("msg", user_key="usr-123", latency=42)`). Pure stdlib.
- Telegram bot: replaced `logging.basicConfig` with structured logger; added
  global `_error_handler` that captures unhandled exceptions with chat_id,
  user_key, error_type, traceback — logs structured JSON + notifies user.

### 10.10 Knowledge Base RAG Ingestion
- `core/memory.py`: `_seed_project_documentation()` ingests project docs
  (ROADMAP.md, DECISION_LOG.md, TASK_BOARD.md, ONBOARDING.md, AGENTS_GUIDE.md,
  WORKSPACE_SUMMARY.md) into `knowledge_base` ChromaDB collection.
  Idempotent (skip if source already seeded) + chunked (>5000 chars).
- Telegram `/knowledge <query>` command: semantic search over project docs via
  `/api/v1/knowledge/search` endpoint.