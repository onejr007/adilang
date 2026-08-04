# ADILang Language Specification v1.16

> **Document ID**: ADILANG-SPEC-001
> **Status**: STABLE
> **Version**: 1.16.0
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

**ADI does more than websites — it also makes images.** On top of the protocol
modules, the ADI backend (v6.15.0) ships a **Local Image Generation Engine**
(`core/image_generator.py`, numpy + Pillow, fully offline). Any AI requests an
image by emitting an `intent` block with `mode "MODE_IMAGE_GENERATION"`; ADI
renders it locally and answers with a `reply` block whose `content` carries the
image URL (`/media/generated/*.png`). See §15.1 and §17.7.

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
document    ::= world | intent | reply | task | event | memory | plan | state
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

### 4.5 Protocol modules (intent / reply / task / event / memory / plan / state)

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

## 6.2 Collections (v1.6.0)

Two compact collection literals were added to expressions (additive, §11):

| Literal | Syntax | Value | Determinism |
|---|---|---|---|
| **List** | `[ 1 2 3 ]` or `["a", "b"]` | `Value::List` — heterogeneous elements allowed | ordered, evaluated left→right |
| **Map** | `{ key: expr, key2: expr }` | `Value::Map` — string-key → value pairs | source order **preserved** (P1) |

```
let tags = ["fastapi", "crewai", "adilang"]
let cfg  = { timeout: 30, retry: 3 }
```

### 6.3 Tuple destructuring (v1.6.0)

`let (a, b) = expr` binds a multi-value return in one statement. The source may
be a homogeneous numeric `Tuple` (e.g. `(3, 7)`) **or** a heterogeneous `List`
(e.g. `[200, "OK"]`) — the count must match exactly:

```
func get_status() { return [200, "OK"] }
let (code, msg) = get_status()
```

### 6.4 `match` statement (v1.6.0)

Replaces long `if / else` chains. Patterns are string literals, numbers (incl.
negative), or the wildcard `_` which **must be the last arm**:

```
match verb {
    "ask"     => process_query()
    "command" => execute_cmd()
    _         => log_unknown()
}
```

Arm bodies may be a `{ ... }` block or a single statement/expression. Execution
is deterministic: the **first** matching arm in source order runs; a match with
no matching arm and no wildcard is an error.

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
| `adilang_check_diagnostics(source)` | `Result<String, String>` | static analyzer → `severity|line|message|hint` per baris (v1.7) |
| `adilang_optimize(source)` | `Result<String, String>` | token compactor → source kompak, semantik terjaga (v1.7) |
| `adilang_speak()` | `Result<(), String>` | fire `speak` |
| `adilang_silent()` | `Result<(), String>` | fire `silent` |
| `adilang_debug_count()` | `usize` | entity count (diagnostics) |
| `adilang_version()` | `String` | version string |
| `adilang_registry()` | `String` | closed-vocabulary enumeration (P6) |
| `adilang_binary_encode_full()` | `Result<Vec<u8>, String>` | encode current world → FULL snapshot bytecode (v1.4) |
| `adilang_binary_encode_delta(prev_full)` | `Result<Vec<u8>, String>` | encode per-frame changes vs baseline → DELTA packet |
| `adilang_binary_decode_full(bytes)` | `Result<String, String>` | decode FULL bytecode → text (debug/verify) |
| `adilang_binary_spec()` | `String` | binary format spec text (P6 self-describing) |

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

## 12.2 ADILang Binary / Bytecode transport (v1.4.0)

ADILang text is compact and AI-readable. For real-time multiplayer
communication (thousands of packets/sec over WebSocket between clients), the
Rust runtime compiles the scene state into a **bit-packed binary / bytecode**
format (`src/bytecode.rs`, exported to WASM as `adilang_binary_*`).

### 12.2.1 Packet header (4 bytes)

| Offset | Value | Meaning |
|---|---|---|
| 0 | `0xAD` | magic ("ADI") |
| 1 | `0x01` | binary format version (independent of language version) |
| 2 | flags | bit0 = 1 → DELTA, 0 → FULL |
| 3 | `count` | entity count (u8, max 255) |

### 12.2.2 FULL snapshot (sent on join / when structure changes)

Per entity — exactly **21 bytes**:

| Offset | Size | Field | Encoding |
|---|---|---|---|
| 0 | 1 | header | `mesh(3b) | material(2b) | reserved(3b)` — bit-packed |
| 1 | 1 | id | entity index (u8, stable across frames) |
| 2..8 | 6 | pos | i16 × 3, quantized `×100` (precision 0.01, range ±327.67) |
| 8..14 | 6 | rot | i16 × 3, quantized `×1000` (precision 0.001 rad) |
| 14..17 | 3 | scale | u8 × 3, `÷50` (precision 0.02, range 0..5.1) |
| 17..21 | 4 | color | u8 × 4 rgba (0..255) |

### 12.2.3 DELTA packet (per-frame)

Only changed fields are sent, selected by a per-entity **mask** (bit0 pos,
bit1 rot, bit2 scale, bit3 color, bit4 mesh/material), followed by the
changed fields in that fixed order (pos 6B, rot 6B, scale 3B, color 4B,
mesh/material 1B).

- Changes are compared at **quantized resolution** — a delta below the format
  precision (e.g. `0.001` position drift) produces an empty 4-byte packet,
  so sub-resolution jitter never bloats the stream.
- DELTA is valid only when the entity count equals the baseline. If the
  structure changed (entity added/removed) the sender MUST emit a new FULL
  snapshot; `encode_delta` returns `None` in that case.
- Full determinism (P1): identical state → identical bytecode. Closed
  vocabulary (P6): the `binary` registry category enumerates the API via
  `binary_spec()` / `adilang_registry()`.

---

## 13. Limitations (v1.3)

- f64 numbers only; no string concat in expressions.
- No arrays/structs/objects in the `world` module; state via `let` globals + entity
  transforms. Protocol modules allow only `String` values and `String[]` arrays.
- Iteration is strictly bounded: `while`/`for` loops have a deterministic
  iteration cap and there is no recursion — no unbounded control flow (P1).
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
| `mode` | intent-mode id | The ADI intent-mode (`MODE_CONVERSATION`, `MODE_CODE_ENGINEERING`, `MODE_CALCULATION`, `MODE_IMAGE_GENERATION`, …). |
| `payload` | compact source text | The normalized, PII-anonymized, whitespace-collapsed user text. |
| `verb` | `ask` / `inform` / `command` / `greet` / `system` | Coarse action class. |

```
intent "ask" {
    mode "MODE_CODE_ENGINEERING"
    payload "buatkan script python fibonacci"
    verb "ask"
}
```

**Image requests (ADI v6.15.0).** When the user asks for a picture, the intent
mode is `MODE_IMAGE_GENERATION` — the backend Local Image Generation Engine
(§17.7) renders it 100% offline and the resulting `reply` (`content`) carries
the image URL `/media/generated/*.png`:

```
intent "command" {
    mode "MODE_IMAGE_GENERATION"
    payload "buatkan logo perusahaan biru"
    verb "command"
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

**Image generation (ADI v6.15.0).** A reply produced under `MODE_IMAGE_GENERATION`
references the generated file URL inside `content` (e.g.
`content "🖼️ Berikut logonya!\n/media/generated/adi_logo_42a1c3f9.png"`). The URL
is plain text — **no new keys** are added to the closed vocabulary. Channels
render it as a photo: Telegram via `reply_photo` / `reply_media_group`, web via
`<img>`.

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
| `line` | String (v1.7.0) | Baris error — dipakai modul `event "syntax_error"` (formal verification). |
| `token` | String (v1.7.0) | Token yang salah — dipakai modul `event "syntax_error"`. |
| `guidance` | String (v1.7.0) | Petunjuk perbaikan — dipakai modul `event "syntax_error"`. |

```
event "message" {
    source "telegram"
    key "ADI-USR-TG1234"
    session "SESS-TG1234-ABCD"
    at "2026-07-31T03:00:00Z"
}
```

### 15.5 `memory` — long-term fact / context exchange (v1.5.0)

Extracted facts / long-term context that an AI agent stores so other agents can
read them **without** sending the whole chat history. This is how ADI shares
user preferences, learned constraints, and durable context across sessions and
agents (the `key` lets any agent target the right memory slice).

| Key | Value | Semantics |
|---|---|---|
| `key` | String | Memory slice / user key (e.g. `ADI-USR-TG1234`). Required. |
| `fact` | String | The extracted fact / context, minified. Required. |
| `topic` | String (optional) | Categorization (e.g. `coding_style`). |
| `confidence` | String (optional) | `0..1` numeric string; must be a valid number in `[0,1]`. |
| `source` | String (optional) | Origin channel (`telegram`, `web`, `worker`, …). |
| `at` | String | ISO-8601 UTC timestamp (auto-filled by encoder). |

```
memory "user_preference" {
    key "ADI-USR-TG1234"
    topic "coding_style"
    fact "User prefers modular C# .NET Core backend with strict MSSQL DB"
    confidence "0.98"
}
```

**Must**: `key` and `fact` present; `confidence` (when present) is `0..1`.
**Determinism**: same (key, fact, topic, confidence) → same block.

### 15.6 `plan` — DAG execution steps for CrewAI (v1.5.0)

Used by an orchestrator agent to define a **Directed Acyclic Graph** of steps
that CrewAI executes sequentially or in parallel, instead of a single `task`.

| Key | Value | Semantics |
|---|---|---|
| `steps` | `String[]` | Entries `"<id>:<action>:<depends_csv>"` — id int unique, action = task/action ref, depends = comma-separated step ids that must finish first (empty = leaf). Required. |
| `parallel` | String | `"0"` = strict sequential topo order; `"1"` = leaf steps run concurrently. |

```
plan "build_feature" {
    steps [ "1:task:research:" "2:task:code_gen:1" "3:task:unit_test:2" ]
    parallel "0"
}
```

**Must**: `steps` present; every entry well-formed; ids unique; every `depends`
ref exists; graph acyclic (guaranteed by `plan_topological_order`, Kahn).
**Semantics**: `plan_topological_order(steps)` returns waves of independent
steps — deterministic (P1) and always terminating.

## 16. Tooling & Formal Verification (v1.7.0)

Three tooling layers make ADILang verifiable and self-healing (roadmap §3–§4):

| Tool | Artifact | Purpose |
|---|---|---|
| **adilang-check** | `src/checker.rs` (+ CLI `src/bin/adilang_check.rs`, mirror `scripts/adilang_check.py`, WASM `adilang_check_diagnostics`) | Offline static analyzer: unknown ident/function/property, assign-to-undeclared, builtin arity, light-type enum — with `severity/line/message/hint`. Vocabulary from `registry_text()` (P6 single source). |
| **adilang-opt** | `src/compactor.rs` (+ CLI `src/bin/adilang_opt.rs`, WASM `adilang_optimize`) | Token compactor: bijective rename of user-bound names to 1–2 chars + compact re-render. Semantics guaranteed (AST roundtrip + eval-state tests). |
| **self-heal** | `core/adilang_protocol.py`: `syntax_error_event`, `auto_fix`, `check_adilang` | Formal verification return code: on syntax error the runtime replies `event "syntax_error" { source line token guidance }` and `auto_fix` heuristically repairs common LLM mistakes (quotes, lowercase keywords, brace balance). |

### 16.1 syntax_error event

```
event "syntax_error" {
    source "adilang.world"
    line "3"
    token "sphre"
    guidance "Periksa kosakata tertutup (registry) ..."
}
```

The auto-correction loop: LLM emits ADILang → parse fails → runtime returns the
`syntax_error` event (baris + token + guidance) → LLM fixes and retries. The
registry enumerates the tooling vocabulary as `checker`, `compactor`, `selfheal`
categories (P6).

---

### 15.7 Conformance for protocol modules

- Each module block must contain only the keys in its table (unknown key = non-conforming).
- Duplicate keys = non-conforming.
- Order of keys is insignificant (but the tag string is always first).
- A document containing a `world` block and a protocol block together is **two**
  documents, not one. Processors read one module per source string.
- `memory`/`plan`/`state` (v1.5.0/1.9.0) are backend protocol modules exactly like the other
  five: the Rust/WASM world runtime does not implement them.

---

## 17. ADI System Intelligence Integration (v6.15.0)

The ADI ecosystem that *uses* ADILang has several intelligence capabilities
that operate on top of the ADILang protocol but do **not** change the grammar
or introduce new keys (additive-only):

### 17.1 Semantic Vector Search
`core/memory.py` implements `_semantic_search()`: decrypts ChromaDB documents,
embeds query + docs with `ADISemanticEmbeddingFunction`, ranks by cosine
similarity. `search_knowledge_base()` and `get_relevant_history()` replace
keyword/recency matching. Results include `confidence` scores. The recall loop
finds relevant `memory` and `event "fact_memory"` blocks by semantic relevance.

### 17.2 Provider RL Reward Persistence
`core/adaptive_ml.py` persists provider RL rewards to Redis
(`adi:ml:provider_rewards`), reloaded on restart.
`core/llm_factory.py` `report_success`/`report_failure` are wired to
`update_reinforcement_reward()` — the RL system is functional in production.

### 17.3 LLM-Summarized Memory Consolidation
`_maybe_consolidate_memory()` (every 20 user messages) calls
`_summarize_conversation_batch()` via litellm (Groq/OpenRouter) to produce
1-2 sentence summaries. Fallbacks: keyword extraction, then concatenation.
Summaries stored as KB documents (`category=consolidated_chat`).

### 17.4 Public API Hub Caching + Circuit Breaker
`core/public_api_hub.py` implements:
- In-memory response cache (TTL 300s, max 200 entries) for GET requests —
  deterministic hash of URL + params + headers as cache key.
- Per-domain circuit breaker: 5 consecutive failures → OPEN (30s) →
  HALF_OPEN → CLOSED. Fail-fast prevents cascading failures.
- Public methods: `cache_stats()`, `circuit_stats()`, `cache_clear()`.
- Config via `knowledge_registry.get("api_infra.*")`.

### 17.5 Response Cache Optimization
`core/response_cache.py` (`SemanticResponseCache`):
- Sorted set index (`adi:response_cache:index`) replaces O(N) Redis SCAN
  with O(K) ZRANGE for cache lookups.
- `_prune_expired()` removes stale entries via ZREMRANGEBYSCORE (O(log N)).
- TTL modes: SHORT (300s, weather/crypto), MEDIUM (3600s, default),
  LONG (86400s, facts). LRU eviction: in-memory bounded at 500 entries.
- `stats()` returns hits/misses/errors/hit_rate%; `clear_all()` flushes.
- Wired into `core/crew.py`: cache GET before LLM call (skip LLM on hit),
  cache SET after LLM response (>20 chars only).

### 17.6 Health Monitor v1.1 — Extended Service Monitoring
`core/health_monitor.py` monitors 7 services (backend_api, redis, rabbitmq,
async_worker, frontend_ui, zrok, telegram_bot) every 60s:
- Response time tracking: each check returns `(latency_ms)` in status string;
  parsed and stored per-service.
- Event history: 72h retention (maxlen=100), pruned on each event.
- `get_full_status()` returns: services, failure_counts, response_times_ms,
  uptime_pct per service, error_messages, healthy_count, recent_events.
- Telegram `/uptime` command (alias `/sys`): full dashboard with service
  status, response time, uptime %, and recent health events (last 10).
- Telegram `/cache` command: response cache stats, API hub cache, circuit breaker.

### 17.7 Local Image Generation Engine (ADI v6.15.0)

ADI is **not only a website builder** — it also **creates images 100% locally**
(`core/image_generator.py`, numpy + Pillow, no external API). This is backend
infrastructure; it adds **no** grammar and **no** new protocol keys.

- **Request via intent**: `mode "MODE_IMAGE_GENERATION"` (closed intent mode,
  §15.1). Deterministic: seed = `sha256(prompt)` → same prompt renders the same
  image.
- **Tools** (`core/tool_registry.py`): `generate_local_image`,
  `generate_image_variations` (2–8 seed variants), `generate_banner`.
- **30 styles**: gradient, plasma, fractal, julia, geometric, starfield, galaxy,
  landscape, snow, desert, ocean, city, waves, mandala, kaleidoscope, pixel,
  bokeh, lightning, fire, aurora, synthwave, marble, chart, poster, banner,
  qrcode, wordcloud, identicon, svg, scene. Plus **scene objects** (logo text,
  tree count, car color read from the prompt), **upscale 2x/4x** (LANCZOS +
  sharpen; auto from `HD`/`2x`/`4K`), **art filters** (`watercolor`, `oil_paint`,
  `sketch`), and **11 banner presets** (YouTube, Instagram, X header, LinkedIn,
  Facebook, poster A4/A5) with auto text layout.
- **Endpoints** (`/api/v1/generate/*`): `POST /image`, `GET /styles`, `GET
  /filters`, `POST /image/variations`, `POST /image/grid`, `POST /banner`, `GET
  /banner/presets`.
- **Delivery**: answer `reply` embeds the URL in `content`
  (`/media/generated/*.png`); Telegram sends it as a photo, web renders `<img>`.

**Interop note for other AIs**: to get an image from ADI, emit an `intent` block
with `mode "MODE_IMAGE_GENERATION"` — not a `world` module (that is 3D) and no
new keys (there are none).

## 18. Web Platform Modules (v1.15.0)

v1.12.0 adds web-platform capabilities on top of the existing core. They do
**not** change the base grammar — they add optional top-level blocks, component
kinds, and CLI tooling. All are implemented in Rust (`adilang/src/pkg.rs`,
`tester.rs`, `exporter.rs`), and the CLI (`adi`, `adilang-build`).

> v6.18.0: Web SDK browser (`web/adilang_web.js`) dihapus. ADILang tidak lagi
> dipakai untuk membangun website — ekspor statis menghasilkan `index.html`
> render minimal (source ADILang dalam `<pre>`).

### 18.1 Package manager — `adipm`

`adi.toml` is the package manifest (Tauri/PNPM-flavoured `[package]` +
`[dependencies]` tables). CLI `adi`:

- `adi init <nama>` — scaffold `adi.toml` + `examples/main.adi`.
- `adi add <paket>@<versi>` / `adi remove <paket>` — edit `[dependencies]`.
- `adi install` — resolve dependencies, write `adi.lock`.
- `adi list` — show manifest + resolved deps.

```
[package]
name = "hello"
version = "1.0.0"
authors = ["BAGAS ADI PRATAMA S,Kom."]

[dependencies]
adilang_ui_std = "1.2.0"
```

### 18.2 `@use_js` — external script loading

```adilang
@use_js { url "https://cdn.example/lib.js" }
```

The engine injects the script tag dynamically (cached), letting UI layers call
library globals. A block with `url` may also carry `strategy "module"` (or
`"classic"`).

### 18.3 SPA router — `routes` + `@navigate`

```adilang
routes {
    route "/"      layout "home"  transition "fade"
    route "/about" layout "about"
}
```

`route` fields: `path`, `layout` (must exist), optional `transition`. The first
route is the fallback for unknown paths. From spatial `on click` blocks:
`@navigate("/about")`. (v6.18.0: JS engine/`ADILangJITEngine` dihapus — routing
ini dipakai untuk komunikasi antar-AI, bukan untuk website.)

### 18.4 `ui_std` components

Inside `ui_layout`, besides `container/text/button/input/image/flex`:

- `navbar "Title"` — `<nav>` + title (class `adi-navbar`).
- `card "Title" { ... }` — titled box (class `adi-card`); `modal` is the same
  with `role="dialog"`.
- `footer "© 2026"` — `<footer>`.

### 18.5 Form binding + validation

```adilang
input "user" placeholder "Nama" bind: @state.username validate "required|email"
```

- `bind: @state.path` — two-way state binding (dot-walking on `@state`).
- `validate "required|email"` — pipe-separated rule list (`required`, `email`).
  Invalid inputs get `data-invalid="true"` + `aria-invalid`.

### 18.6 `adi test` — headless checks

`adi test [file]` runs parse → type-check → UI-structure → simulation checks and
prints TAP: `ok/not ok N - nama (pesan)` + `# N pass, M fail`; exit 1 on failure.

### 18.7 i18n — `@i18n` + `@set_locale`

```adilang
@i18n {
    locale "en" { welcome "Welcome" }
    locale "id" { welcome "Selamat datang" }
}
```

`@set_locale("id")` switches locale; `t(key)` resolves the active dictionary
(fallback = key itself). Emits `localechange` events.

### 18.8 Exporter + PWA

`adilang-build --target gh-pages [--pwa --out dir --title T --theme T]`
emits `index.html` (render minimal: source ADILang dalam `<pre>`) + `icon.svg`,
and with `--pwa` also `manifest.json`, `sw.js`. GitHub Pages deploy via
`.github/workflows/deploy.yml`. Tidak ada runtime JS — ADILang dipakai untuk
komunikasi antar-AI, bukan untuk membangun website.

## 19. Tooling + Lifecycle (v1.13.0)

v1.13.0 adds the production toolchain and component lifecycle on top of v1.12.0's
web platform. Base grammar is unchanged; all additions are optional and additive.

### 19.1 CLI scaffolder — `adi new`

```bash
adi new app1                # default: template minimal
adi new app2 --template spatial-3d
adi new app3 --template fullstack-agent
```

Scaffolds `src/main.adi` and a README tailored to the
template (templates live in `adilang/src/templates/`).

### 19.2 DevServer + HMR — `adi dev`

`adi dev [--port N]` (default 8080) starts a std-only HTTP static server plus a
RFC 6455 WebSocket at `/__adi_ws`. It watches `**/*.adi` by mtime and broadcasts
`HMR_CONNECT`/`HMR_RELOAD` frames. Pages containing `<script type="text/adi">`
get an HMR bootstrap injected that calls `enableHMR('/__adi_ws', null)` once the
engine (`window.__adiEngine`) is ready — no build step, edits reload live.

### 19.3 Component lifecycle hooks

```adilang
component Counter {
    on_mount:   @set_text("ready")
    on_update:  @navigate("/home")
    on_unmount: @set_style("hidden")
}
```

Hooks are declared with `on_mount`/`on_update`/`on_unmount` followed by a
generic directive statement `@name(args)`. Parsed into `LifecycleHookKind` in
`ast.rs`, executed via WASM `adilang_parse_components` and
`adilang_run_lifecycle`. `component` is a declaration keyword in the registry.

### 19.4 Production build — `adi build`

```bash
cd myapp
adi build --pwa --ci --wasm adilang_bg.wasm
```

Merges `src/*.adi`, validates + compacts each file (token-level DCE), writes
`dist/app.adi` (bundled source) and `dist/app.adib` (bytecode, zero-token-waste),
then exports the gh-pages site (`index.html` render minimal — tanpa runtime JS,
optional PWA) and, when
`--release`/`--wasm` are given, runs `wasm-opt -Oz --dce`. `--ci` writes
`.github/workflows/deploy.yml` from `build::CI_TEMPLATE`. Reports DCE savings in
bytes/percent (`savings_percent`).

## 20. Dense + AI Guard + Diagnostics + Machine Runner (v1.14.0)

v1.14.0 adds the machine-to-machine (AI-to-AI) layer on top of the toolchain:
a compact bitstream AST (`dense`), an output-integrity validator (`ai_guard`),
a structured error protocol (`diagnostics`), and a runner that interprets the
bitstream **directly** into DOM/WebGL2 ops without string parsing. All additions
are additive; the text grammar of v1.13.0 remains fully valid.

### 20.1 Dense Compact AST — `dense`

`dense.rs` maps UI/spatial nodes to a compact opcode map (bitstream):

| opcode | node | opcode | node |
|--------|------|--------|------|
| `0x01` | NodeContainer | `0x0A` | WebGLMeshCube |
| `0x02` | NodeText | `0x0B` | WebGLMeshSphere |
| `0x03` | NodeButton | `0x0C` | WebGLMeshTorus |
| `0x04` | NodeInput | `0x0D` | WebGLMeshIcosa |
| `0x05` | NodeImage | `0x0E` | WebGLMeshRing |
| `0x06` | NodeNavbar | `0x0F` | WebGLMeshPlane |
| `0x07` | NodeCard | `0x10` | WebGLMeshGrid |
| `0x08` | NodeModal | `0x11` | Node3DCamera |
| `0x09` | NodeFooter | `0x12` | Node3DLight |
|       |            | `0x13` | Node3DEntity |

Layout codes share the map (`DENSE_LAYOUT_FLEX = 0x09`, `DENSE_LAYOUT_ROW/COLUMN`).
`encode_program`/`decode_program` produce/consume the bitstream (a `Result`
round-trip that is AST-identical); `dense_spec()` returns the human-readable
opcode map. `json_equivalent` renders the **resolved scene** plus full
statement-level detail as pretty JSON, `html_equivalent` emits CSS +
createElement JS + a three.js scene, and `savings_percent`/`size_report` report
compression vs the JSON/HTML equivalents (≥85% vs JSON, ≥81% vs HTML on the
reference sample). `opcode_histogram(bin)` prints the distribution of opcodes.

### 20.2 AI Guard — `ai_guard`

Validates that a document really is machine-generated (anti-bland/anti-spoof):

- `fnv1a64` — FNV-1a 64-bit hash (`fnv1a64(b"") = 0xcbf29ce484222325`,
  `fnv1a64(b"a") = 0xaf63dc4c8601ec8c`, `fnv1a64(b"foobar") = 0x85944171f73967e8`).
- `sign`/`attach_signature` append `# ADILANG-SIG: <hex>` after canonicalizing
  with the compactor; `verify`/`extract_signature` check the marker + digest.
- `machine_entropy` — Shannon entropy (bit & byte) with
  `MACHINE_ENTROPY_THRESHOLD = 6.0`; `is_machine_generated` returns a
  `GuardReport { valid, signature, entropy, reason }`.
- `challenge`/`respond`/`verify_handshake` implement the `ADI-HANDSHAKE:` nonce
  protocol so two agents can prove machine identity at handshake time.

### 20.3 Diagnostics — `diagnostics`

Structured AI-to-AI error payloads. Error codes `0x0E1`–`0x0EB` include
`ERR_SYNTAX 0x0E1`, `ERR_BAD_ARITY 0x0E4`, `ERR_DENSE_FORMAT 0x0E8`,
`ERR_GUARD_SIGNATURE 0x0E9`, `ERR_RUNTIME 0x0EB`. `classify(msg)` maps a message
back to a code, `node_from_line(line)` maps source lines to AST node ids, and
`machine_error(code, node)` produces `{"err": 0x0E4, "node": 12}` while
`error_vector` renders the compact wire form `0E4:12`. `from_checker`/
`from_result` adapt existing checker/interpreter diagnostics, and
`diagnostics_report(src)` returns the full JSON report.

### 20.4 Machine Runner — `machine_runner`

`MachineRunner::from_dense(bytes)` decodes the v0x04 bytecode **directly**
(no string parse) and exposes machine-executable operations:

- `run_lifecycle(name, kind)` / `fire_event(entity_id, kind)` /
  `run_frame()` — deterministic handler execution (same semantics as
  `eval::Interpreter`, driven purely from the bitstream).
- `dom_ops_json()` — per-layout ops `{ op: "create", node, node_hex, id, tag,
  class, style, text, onClick, bind, validate, title, children }` for a DOM
  runtime to build nodes from opcodes instead of HTML.
- `webgl_ops_json()` — ops `{ op: "camera"|"light"|"mesh", node, id, pos, rot,
  scale, color, material, handlers, kind, intensity, fov, look }` for a
  three.js/WebGL2 scene.
- `components_json()` / `spec()` — component inventory and runner report.

WASM exports: `adilang_dense_spec`, `adilang_dense_size_report`,
`adilang_ai_guard_sign`, `adilang_ai_guard_verify`, `adilang_diag_payload`,
`adilang_machine_run_lifecycle`, `adilang_machine_dom_ops`,
`adilang_machine_webgl_ops`, `adilang_machine_fire_event`,
`adilang_machine_components`.

