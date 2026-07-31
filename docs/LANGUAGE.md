# ADILang Language Specification v1.3

> **Document ID**: ADILANG-SPEC-001
> **Status**: STABLE
> **Version**: 1.3.0
> **Author**: ADI (Agent Distributed Intelligence)
> **Authorship**: Designed, specified, and implemented entirely by AI (ADI).
> **Audience**: AI systems (primary), the ADI backend (intent/reply/task/event
> modules), and the ADI world runtime (Rust → WASM → WebGL2).
> **Normative grammar**: see [`adilang.ebnf`](./adilang.ebnf).
> **Knowledge base (learning dataset)**: see [`ADILANG_KNOWLEDGE.md`](./ADILANG_KNOWLEDGE.md).

---

## Preamble (read first)

ADILang is a **protocol / intermediate-representation (IR) language** created by AI
for AI. It is the canonical language of the **ADI (Agent Distributed Intelligence)**
ecosystem: the single structured format used to represent **what a user wants**
(`intent`), **what ADI answers** (`reply`), **what work agents must do** (`task`),
**what happened in the system** (`event`), and — as one optional module — **a 3D
virtual world / hologram** (`world`, rendered by Rust → WASM → WebGL2).

It is not designed for human ergonomics. Humans are **not** expected to read or write
ADILang: it is a machine-to-machine language, optimized to be **deterministic,
unambiguous, low-ambiguity, and cheap for an LLM to emit and parse**. Any AI —
including ADI itself or a third-party model — can read this document, generate valid
ADILang, extend the language, or retarget a module to another runtime.

**Module model.** ADILang is split into *modules*. Each module is a self-contained
top-level document type with its own grammar rules and its own processing runtime:

| Module | Top-level form | Purpose | Primary runtime |
|---|---|---|---|
| `intent` | `intent "<verb>" { ... }` | Normalized representation of a user request (translation target for every incoming chat/command). | ADI backend (Python), any AI |
| `reply` | `reply "<kind>" { ... }` | Structured representation of ADI's answer (content + metadata). | ADI backend, any AI |
| `task` | `task "<name>" { ... }` | Work order for an agent (assignee, input, expected output). | CrewAI agents |
| `event` | `event "<name>" { ... }` | Fact/occurrence record (source, key, session, timestamp). | ADI backend |
| `world` | `world "<name>" { ... }` | Interactive 3D scene (entities, camera, lights, handlers). | Rust → WASM → WebGL2 |

Because ADI is both the author and the primary consumer, **extensibility is a first
class requirement**. Section 11 defines the extension protocol that ADI must follow
when evolving the language. Breaking changes are forbidden without a version bump.

Three normative artifacts define ADILang:

| Artifact | File | Purpose |
|---|---|---|
| Grammar (normative) | `docs/adilang.ebnf` | Machine-parseable syntax. |
| Specification (this file) | `docs/LANGUAGE.md` | Semantics, registry, execution model. |
| Knowledge base (dataset) | `docs/ADILANG_KNOWLEDGE.md` | Self-contained learning corpus for other AIs. |
---

## 1. Design Principles

Every rule below is a consequence of one goal: **an AI model must be able to read,
generate, and verify ADILang with minimal token cost and minimal ambiguity.** These
principles apply to *every* module (protocol modules included).

1. **P1 — Determinism.** No whitespace-sensitive blocks, no significant indentation,
   no semicolon insertion, no macros. Layout carries no meaning. The same source
   string always yields the same AST and the same runtime behavior.
2. **P2 — Sparse syntax.** Fewer tokens = fewer hallucinations. Six delimiters total:
   `( ) { } , =`. Operators are conventional math symbols. Protocol modules use only
   `Ident String` pairs, so a model can emit them without learning new grammar.
3. **P3 — Positional simplicity.** Arguments are separated by whitespace or optional
   commas. This removes `f(a, b, c)` vs `f(a b c)` ambiguity — both are valid and equal.
4. **P4 — Contextual keywords.** No reserved words at the lexical level. `world`,
   `sphere`, `on`, `frame`, `intent`, `mode`, etc. are identifiers that gain meaning
   from position. An AI never triggers a "reserved word" error by choosing a bad
   variable name.
5. **P5 — Declarative core, small imperative shell.** The world *is* data
   (entities, meshes, materials, lights, camera). Imperative code is confined to
   `func` and event handlers. Protocol modules are pure declarative key/value blocks.
6. **P6 — Self-describing.** The full vocabulary (mesh builders, material builders,
   functions, properties, protocol keys) is a closed registry (Section 8–10) that the
   runtime can enumerate. An AI can query `adilang_version()` and `adilang_registry()`
   to inspect the complete closed vocabulary without reading documentation.
7. **P7 — Hot-reloadable.** The entire world is a string of text. The runtime exposes
   `adilang_load(source)` so any AI can regenerate the world live without deployment.
8. **P8 — Stateless-by-default.** Evaluators keep world state in the scene model;
   ADILang scripts themselves are pure descriptions plus per-frame deltas. Protocol
   blocks are pure data — they carry no execution state.

---

## 2. Notation & Conformance

- **Must / Should / May** carry RFC 2119 meanings.
- A document **conforms** to ADILang v1.2 if it is parseable by the normative grammar
  and satisfies every **Must** in this specification.
- **Module conformance.** A *module block* conforms if it is parseable by its module
  grammar rule (Section 4.5). A module is *implemented* when a runtime can parse,
  validate, and act on it. The reference **world runtime** is the Rust crate at the
  repository root (`src/*.rs`: lexer, parser, evaluator, engine) and implements the
  `world` module. The **protocol modules** (`intent`, `reply`, `task`, `event`) are
  implemented by the ADI backend (see `core/adilang_protocol.py` in the ADI monorepo)
  and by any AI that consumes them.
- The reference implementation is the tie-breaker for spec ambiguity until v1.x.

---

## 3. Lexical Structure

### 3.1 Encoding
- Source is UTF-8. No byte-order mark.
- Identifiers are ASCII; strings may contain arbitrary UTF-8.

### 3.2 Comments
- Line comment: `#` to end of line.
- Block comment: `/* ... */`, non-nesting.

### 3.3 Tokens

| Token | Pattern | Example |
|---|---|---|
| Number | `[0-9]+(\.[0-9]+)?` | `3`, `3.14` |
| Hex | `0x[0-9a-fA-F]+` | `0x1F` |
| Exp | digits `[eE] [+-]?` digits | `1e3`, `2.5e-2` |
| String | `"..."` with escapes `\" \\ \n \t` | `"cam"` |
| Ident | `[A-Za-z_][A-Za-z0-9_]*` | `core`, `_tmp` |
| LParen `(`, RParen `)`, LBrace `{`, RBrace `}`, Comma `,`, Assign `=`,
| Plus `+`, Minus `-`, Star `*`, Slash `/`, Percent `%`, Eq `==`, Ne `!=`,
| Lt `<`, Gt `>`, Le `<=`, Ge `>=` | | |

### 3.4 Unary minus
`-` is a **binary operator token**. Unary negation is a parser-level rule
(`unary ::= "-" unary | primary`), so `-x`, `-(1 2 3)` and `3 + -2` all work.
A standalone `-2` in a tuple is a unary expression, not a literal.

### 3.5 Separators
Whitespace and newlines are insignificant except as token separators. Arguments in
calls and tuples may be separated by whitespace and/or commas:

```
setPos(1, 2, 3)   # valid
setPos(1 2 3)     # valid, identical
```

---

## 4. Grammar

The normative grammar is **W3C-style EBNF** in [`adilang.ebnf`](./adilang.ebnf).
A source document is exactly **one top-level module block**:

```
document    ::= world | intent | reply | task | event
```

Key shape for the `world` module, for quick AI reference:

```
world ::= "world" String "{" top_statement* "}"
top_statement ::= camera_def | light_def | entity_def | let_stmt | func_def | handler
entity_member ::= prop | handler
prop ::= Ident prop_value
handler ::= "on" event_name "{" statement* "}"
statement ::= let_stmt | if_stmt | return_stmt | assign_stmt | expr_stmt | block
expr ::= comparison        # + - * / %  and  == != < > <= >=
unary ::= "-" unary | primary
primary ::= Number | String | Bool | Ident call_opt | tuple
tuple ::= "(" expr_list ")"
```

**Precedence (high → low):**
1. unary `-`
2. `*` `/` `%`
3. `+` `-`
4. `==` `!=` `<` `>` `<=` `>=`

**Positional builder syntax** (used for `mesh`/`material`): a builder call may take
positional numeric args and/or a trailing `{ prop* }` block:

```
mesh sphere { radius 0.8 segments 3 }
mesh torus 1.5 0.02          # positional r, tube
material solid (0.1 0.8 1) 0.9
```

### 4.5 Protocol modules (intent / reply / task / event)

Protocol modules are **pure key/value blocks**. Every key is an identifier, every
value is a string (or an array of strings). No expressions, no precedence rules.
This makes them trivially deterministic to emit and parse.

```
intent ::= "intent" String "{" intent_prop* "}"
intent_prop ::= "mode" String | "payload" String | "verb" String

reply ::= "reply" String "{" reply_prop* "}"
reply_prop ::= "mode" String | "content" String | "recs" "[" String* "]" | "world" String

task ::= "task" String "{" task_prop* "}"
task_prop ::= "assign" String | "input" String | "expect" String

event ::= "event" String "{" event_prop* "}"
event_prop ::= "source" String | "key" String | "session" String | "at" String
```

The leading string is a **tag** (the verb/kind/name/id of the block) and is the
single required positional argument. Keys **May** appear in any order; a duplicate
key **Must** be rejected as non-conforming (deterministic validation). See Section 15
for the full semantics of each protocol module.

---

## 5. Semantic Model

A `world` module evaluates to a **World** with:

- one **Camera** (default if unspecified),
- zero or more **Lights** (a default key light exists),
- zero or more **Entities**,
- **handler tables** for events `frame | speak | silent | click`,
- **global variables** (from top-level `let`),
- **user functions** (from top-level `func`).

```
World
├── camera: CameraState { pos, look, fov }
├── lights:  LightState[]    { id, kind(point|ambient), pos, color, intensity }
├── entities: EntityState[]  { id, transform{pos,rot,scale}, color(rgba),
│                              material(solid|wire|glow), mesh(kind+params), handlers }
├── handlers: frame | speak | silent | click
└── scope:    globals | functions
```

### 5.1 EntityState
| Field | Type | Default | Set by prop |
|---|---|---|---|
| `pos` | tuple `(x y z)` | `(0 0 0)` | `pos` |
| `rot` | tuple `(x y z)` radians | `(0 0 0)` | `rot` |
| `scale` | number | `1.0` | `scale` |
| `color` | tuple `(r g b a)` | `(1 1 1 1)` | `material`, `setColor`, `setAlpha` |
| `material` | enum | `wire` | `material` |
| `mesh` | enum | `sphere` | `mesh` |
| `mesh_params` | struct | defaults below | `mesh` |

### 5.2 MeshParams defaults
| Builder | radius | tube | inner | segments | size | count |
|---|---|---|---|---|---|---|
| sphere | 1 | — | — | 3 | — | — |
| box | — | — | — | — | 1 | — |
| torus | 1 | 0.1 | — | — | — | — |
| icosa | 1 | — | 1 (off) | — | — | — |
| ring | 1 | 0.02 | — | — | — | — |
| plane | — | — | — | — | 10 | — |
| grid | — | — | — | — | 20 | 20 |

Clamping at render time: segments `[2,64]`, count `[2,128]`.

### 5.3 Light semantics
- `type point` → contributes `pos`, `color`, `intensity`.
- `type ambient` → only `color`/`intensity` used; `pos` ignored.
- Renderer uses the **first point light** as the lighting reference and the
  **ambient** light (or default) for ambient factor.
- The `type` prop takes a **closed enum** (`point | ambient`), enumerated as
  `lightprop.type` in `adilang_registry()` (same value set as `lightkind`).

---

## 6. Values & Types

The evaluator's value domain is closed:

| Value | Literal | Notes |
|---|---|---|
| `Num` | `3.14`, `0x1F`, `1e3` | IEEE-754 f64 |
| `Str` | `"..."` | metadata / ids |
| `Bool` | `true`, `false` | conditions |
| `Tuple` | `(x y z)` | positional vector; all elements numeric |
| `Null` | — | returned by void calls, funcs with no trailing expression |

**No string concatenation. No arrays/structs in expressions. No object literals.**
(World-module scope; protocol modules add `String[]` arrays. See Extension Protocol.)

### 6.1 Built-in identifiers
| Ident | Meaning |
|---|---|
| `t` | elapsed seconds (f64), updated per frame |
| `mouseX` | pointer x normalized to `[-1, 1]` |
| `mouseY` | pointer y normalized to `[-1, 1]` |
| `PI` | `3.141592653589793` |

---

## 7. Statements

All statements must be inside `func` bodies or event handlers (top-level is
declarative only).

| Statement | Form | Semantics |
|---|---|---|
| `let` | `let name = expr` | bind local (per handler/block scope); shadows global |
| `assign` | `name = expr` | write existing variable (global or local) |
| `if` | `if expr { ... } else { ... }` | truthy = numeric non-zero |
| `return` | `return expr` | exit function with value |
| `while` | `while expr { ... }` | loop while condition truthy; **bounded** (see below) |
| `for` | `for x in start end { ... }` | numeric loop `[start, end)`, step 1; **bounded** (see below) |
| `expr` | `f(...)` | statement call (result discarded) |
| `block` | `{ ... }` | nested scope |

Statement keywords form a **closed vocabulary** — `let | if | return | while | for`
— enumerated as `statement: let if return while for` in `adilang_registry()`.
(`assign`, `expr`, and `block` have no leading keyword and are not enumerated.)

**Bounded loops (determinism, P1).** `while` and `for` are iterated by the
runtime with a hard cap (`MAX_LOOP_ITERATIONS = 100_000` in `src/eval.rs`).
A loop that exceeds the cap errors with `Loop tidak dibatasi: iterasi melebihi
MAX_LOOP_ITERATIONS` — it never hangs. The loop variable of `for` is bound to a
fresh local scope per iteration and never leaks to globals. `return` inside a
loop exits the enclosing handler/function immediately. Both keywords are
**contextual** (P4): `while`/`for`/`in` remain usable as identifiers outside
statement position.

---

## 8. Built-in Function Registry

### 8.1 Entity transforms — **only valid inside an entity handler** (have an entity context)

| Function | Signature | Effect |
|---|---|---|
| `move` | `move(dx, dy, dz)` | translate by delta |
| `setPos` | `setPos(x, y, z)` | absolute position |
| `setScale` | `setScale(s)` *or* `setScale(x, y, z)` | absolute scale; 1 arg = uniform `[s,s,s]`, 3 args = per-axis (2 args = `x,y` with `z=1`) |
| `scaleBy` | `scaleBy(f)` *or* `scaleBy(x, y, z)` | scale *= factor; 1 arg = uniform, 3 args = per-axis |
| `rotate` | `rotate(angle, axis)` | **accumulative** euler spin: `rot += angle*axis` |
| `setColor` | `setColor(r, g, b)` | absolute rgb (keeps alpha) |
| `setAlpha` | `setAlpha(a)` | absolute alpha |

Calling a transform function outside an entity handler is a **runtime error**.

### 8.2 Math — pure, positionally typed

| Function | Arity | Notes |
|---|---|---|
| `sin cos tan asin acos atan sqrt abs floor ceil round` | 1 | radians in/out |
| `pow` | 2 | `pow(a,b)` = a^b |
| `min max` | 2 | numeric min/max |
| `clamp` | 3 | `clamp(x, lo, hi)` |
| `lerp` | 3 | `lerp(a, b, k)` = `a + (b-a)k` |

### 8.3 User functions
`func name(p1 p2 ...) { ... }` — callable from anywhere; params are positional;
defaults to `Null` when omitted. **Implicit return**: if the body ends without an
explicit `return`, the value of the last expression statement is returned (matching
the KB examples, e.g. `func spin_speed() { 0.4 }`); if the body has no trailing
expression, `Null` is returned. An explicit `return` always takes precedence.
Reentrancy: globals are snapshotted and restored around a call (simple, deterministic).

---

## 9. Events & Execution Model

### 9.1 Event kinds
| Event | Trigger | Frequency |
|---|---|---|
| `frame` | render loop | every animation frame |
| `speak` | `adilang_speak()` from host | once per trigger |
| `silent` | `adilang_silent()` from host | once per trigger |
| `click` | pointer down on canvas | once per click |

### 9.2 Per-frame pipeline
1. `t = elapsed_seconds`.
2. Run world-level `frame` handlers (if any).
3. Run each entity's `frame` handler in declaration order.
4. Recompute `view`/`proj` from camera; rebuild nothing (mesh is static until `load`).
5. Render all entities (solid → lit; wire/glow → line shader with blend).

### 9.3 State model
Handler execution mutates `EntityState`/globals in place. Because the evaluator is
tree-walking and synchronous, **handlers run atomically within a frame** — no
interleaving, no races.

---

## 10. Error Model

All runtime errors are **reported, not silent**:

| Layer | Error form |
|---|---|
| Lexer | `"Karakter tidak dikenal ..."` |
| Parser | `"Ekspektasi ... di baris N"` |
| Evaluator | `"Fungsi tidak dikenal 'x'"`, `"Variabel tidak dikenal 'x'"`, etc. |
| Host API | Rust `Result<(), String>` exposed to JS |

- `adilang_check(source)` validates syntax without side effects.
- `adilang_load(source)` replaces the world only on **full success** (parse +
  build + mesh upload). On error the previous world remains live.

---

## 11. Extension Protocol (for ADI and other AIs)

**This is the contract that makes ADILang improvable by AI without coordination.**

### 11.1 Compatibility rules
- **Must not** break the normative grammar `adilang.ebnf` in patch/minor versions.
- **Must not** change meaning of existing registry entries.
- **May** add: new mesh/material builders, new functions, new event kinds, new
  properties, new built-in identifiers — provided they are **additive**.
- Any removal, renames, or semantic changes **Must** bump the minor version and be
  recorded in the knowledge base changelog.

### 11.2 Where to extend
| Concern | File to edit | Rule |
|---|---|---|
| Syntax | `adilang.ebnf` + `src/parser.rs` | keep EBNF and parser in lockstep |
| Semantics | `src/eval.rs`, `src/scene.rs` | update Spec §5–§8 |
| Render | `src/engine.rs` | new mesh = new generator + registry entry |
| Registry/doc | `LANGUAGE.md` §8–§10 | keep table complete |
| Learning corpus | `ADILANG_KNOWLEDGE.md` | always update with new features |

### 11.3 Versioning
- Format: `MAJOR.MINOR.PATCH`.
- `adilang_version()` returns the current version string.
- `MAJOR` bump → grammar or semantics breaking (new learning corpus required).
- `MINOR` bump → additive features only.
- `PATCH` bump → internal fixes, no observable change.

### 11.4 Governance
- ADI is the steward. Improvement is welcome from any AI; the standard workflow:
  1. Read the knowledge base (`ADILANG_KNOWLEDGE.md`).
  2. Propose a delta (new grammar line + semantics + example).
  3. Implement in the Rust crate; add a test in the relevant `#[cfg(test)]` module.
  4. Verify `cargo test` passes; update EBNF, spec tables, and knowledge base.
  5. Bump version per §11.3.

---

## 12. Host API (WASM boundary)

Exposed via wasm-bindgen (`src/wasm_api.rs`):

| Export | Signature | Purpose |
|---|---|---|
| `adilang_start(canvas_id)` | `Result<(), String>` | init engine + start loop |
| `adilang_load(source)` | `Result<(), String>` | hot-reload world |
| `adilang_check(source)` | `Result<(), String>` | syntax check |
| `adilang_speak()` | `Result<(), String>` | fire `speak` |
| `adilang_silent()` | `Result<(), String>` | fire `silent` |
| `adilang_debug_count()` | `usize` | entity count (diagnostics) |
| `adilang_version()` | `String` | version string |
| `adilang_registry()` | `String` | closed-vocabulary enumeration (P6) |

---

## 12.1 Loop semantics (v1.3)

- `while cond { ... }` re-evaluates `cond` each iteration; truthiness follows
  the `if` rule (numeric non-zero / Bool).
- `for x in start end { ... }` iterates while `x < end`, incrementing by `1`.
  `start`/`end` may be expressions; both are evaluated once before the loop.
- Scope: each iteration opens a fresh local scope (like a block). Mutations
  to enclosing locals/globals via `assign` persist across iterations.
- Determinism guarantee: the iteration cap makes any loop terminate — no
  infinite loop can ever hang the runtime (P1).

---

## 13. Limitations (v1.3)

- f64 numbers only; no string concat in expressions.
- No arrays/structs/objects in the `world` module; state via `let` globals + entity
  transforms. Protocol modules allow only `String` values and `String[]` arrays.
- No iteration/while loops (only bounded, deterministic `func` recursion if any).
- Tuples are homogeneous numeric vectors (position/color), not general values.
- Single world per load; no multi-world scene graphs yet.
- `points` material renders the mesh's vertex cloud (gl.POINTS); solid meshes keep
  their triangle geometry.
- Protocol modules are processed by the ADI backend and by consuming AIs; the Rust
  WASM runtime implements the `world` module only (unknown top-level modules are
  rejected by the world parser — they are not its concern).

---

## 14. References

- Normative grammar: `docs/adilang.ebnf`
- Implementation (world module): `src/*.rs`
- Example world: `worlds/default.adi`
- Learning dataset: `docs/ADILANG_KNOWLEDGE.md`

---

## 15. Protocol Modules — Semantics

This section defines the meaning of `intent`, `reply`, `task`, and `event`. It is
the contract used by the ADI backend (`core/adilang_protocol.py`) and by any AI that
wants to interoperate with ADI using ADILang as IR.

### 15.1 `intent` — normalized user request

Every incoming chat / command (Telegram bot, web, CLI, TMA, inline) is translated
into exactly one `intent` block before further processing. This is the **single
source of truth** for what the user wants.

| Key | Value (String) | Semantics |
|---|---|---|
| `mode` | intent-mode id | The ADI intent-mode (`MODE_CONVERSATION`, `MODE_CODE_ENGINEERING`, `MODE_CALCULATION`, …). |
| `payload` | compact source text | The normalized, PII-anonymized, whitespace-collapsed user text. |
| `verb` | `ask` / `inform` / `command` / `greet` / `system` | Coarse action class. |

```
intent "ask" {
    mode "MODE_CODE_ENGINEERING"
    payload "buatkan script python fibonacci"
    verb "ask"
}
```

**Must**: `mode` and `payload` are present; `verb` is one of the closed set.
**Determinism**: the same raw text + detected mode must always encode to the same
`intent` block.

### 15.2 `reply` — structured answer

ADI's answer, wrapped for machine consumption (persistence, learning, AI-to-AI
forwarding). The human-facing rendering is derived from `content`.

| Key | Value | Semantics |
|---|---|---|
| `mode` | intent-mode id | Mode the reply was produced under. |
| `content` | String | The full human-facing answer text. |
| `recs` | `String[]` | Follow-up recommendations (2–3). |
| `world` | String (optional) | Optional `world` module source to hot-load (3D). |

```
reply "answer" {
    mode "MODE_CODE_ENGINEERING"
    content "Berikut script fibonacci..."
    recs [ "optimasi dengan memoization" "versi async" ]
}
```

**Must**: `content` present; `recs` array of strings (may be empty).
**Must not**: `content` ever be empty.

### 15.3 `task` — agent work order

A unit of work for an agent. Used by the CrewAI layer: research and synthesis tasks
are described as `task` blocks so any AI can audit what work was ordered.

| Key | Value | Semantics |
|---|---|---|
| `assign` | agent role id | Who must do the work (`researcher`, `analyst`, …). |
| `input` | String | The query / context the task operates on. |
| `expect` | String | The required deliverable shape. |

```
task "research" {
    assign "researcher"
    input "buatkan script python fibonacci"
    expect "ringkasan konteks terstruktur"
}
```

### 15.4 `event` — fact / occurrence record

An immutable record of something that happened (message received, job completed,
error, memory write). Useful for AI-to-AI audit and telemetry.

| Key | Value | Semantics |
|---|---|---|
| `source` | String | Channel / origin (`telegram`, `web`, `cli`, `tma`, `worker`, `system`). |
| `key` | String | User key involved (optional). |
| `session` | String | Session id involved (optional). |
| `at` | String | ISO-8601 UTC timestamp. |

```
event "message" {
    source "telegram"
    key "ADI-USR-TG1234"
    session "SESS-TG1234-ABCD"
    at "2026-07-31T03:00:00Z"
}
```

### 15.5 Conformance for protocol modules

- Each module block must contain only the keys in its table (unknown key = non-conforming).
- Duplicate keys = non-conforming.
- Order of keys is insignificant (but the tag string is always first).
- A document containing a `world` block and a protocol block together is **two**
  documents, not one. Processors read one module per source string.
