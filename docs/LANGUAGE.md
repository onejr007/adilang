# ADILang Language Specification v1.0

> **Document ID**: ADILANG-SPEC-001
> **Status**: STABLE
> **Version**: 1.0.0
> **Author**: ADI (AI Agent Ecosystem)
> **Authorship**: Designed, specified, and implemented entirely by AI.
> **Audience**: AI systems (primary), and the ADI runtime (Rust → WASM → WebGL2).
> **Normative grammar**: see [`adilang.ebnf`](./adilang.ebnf).
> **Knowledge base (learning dataset)**: see [`ADILANG_KNOWLEDGE.md`](./ADILANG_KNOWLEDGE.md).

---

## Preamble (read first)

ADILang is a **domain-specific language (DSL)** created by AI, for AI, to describe
an interactive **3D virtual world / hologram**. It is not designed for human
ergonomics; it is designed to be **deterministic, unambiguous, low-ambiguity, and
cheap for an LLM to emit and parse**. Any AI — including ADI itself or a third-party
model — can read this document, generate valid ADILang, extend the language, or
retarget it to another renderer.

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
generate, and verify ADILang with minimal token cost and minimal ambiguity.**

1. **P1 — Determinism.** No whitespace-sensitive blocks, no significant indentation,
   no semicolon insertion, no macros. Layout carries no meaning.
2. **P2 — Sparse syntax.** Fewer tokens = fewer hallucinations. Six delimiters total:
   `( ) { } , =`. Operators are conventional math symbols.
3. **P3 — Positional simplicity.** Arguments are separated by whitespace or optional
   commas. This removes `f(a, b, c)` vs `f(a b c)` ambiguity — both are valid and equal.
4. **P4 — Contextual keywords.** No reserved words at the lexical level. `world`,
   `sphere`, `on`, `frame`, etc. are identifiers that gain meaning from position.
   An AI never triggers a "reserved word" error by choosing a bad variable name.
5. **P5 — Declarative core, small imperative shell.** The world *is* data
   (entities, meshes, materials, lights, camera). Imperative code is confined to
   `func` and event handlers.
6. **P6 — Self-describing.** The full vocabulary (mesh builders, material builders,
   functions, properties) is a closed registry (Section 8–10) that the runtime can
   enumerate. An AI can query `adilang_version()` and, in future, a registry API.
7. **P7 — Hot-reloadable.** The entire world is a string of text. The runtime exposes
   `adilang_load(source)` so any AI can regenerate the world live without deployment.
8. **P8 — Stateless-by-default.** Evaluators keep world state in the scene model;
   ADILang scripts themselves are pure descriptions plus per-frame deltas.

---

## 2. Notation & Conformance

- **Must / Should / May** carry RFC 2119 meanings.
- A document **conforms** to ADILang v1.0 if it is parseable by the normative grammar
  and satisfies every **Must** in this specification.
- The reference implementation is the Rust crate at the repository root (`src/*.rs`:
  lexer, parser, evaluator, engine). It is the tie-breaker for spec ambiguity until v1.x.

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
Key shape, for quick AI reference:

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

---

## 5. Semantic Model

A program evaluates to a **World** with:

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

---

## 6. Values & Types

The evaluator's value domain is closed:

| Value | Literal | Notes |
|---|---|---|
| `Num` | `3.14`, `0x1F`, `1e3` | IEEE-754 f64 |
| `Str` | `"..."` | metadata / ids |
| `Bool` | `true`, `false` | conditions |
| `Tuple` | `(x y z)` | positional vector; all elements numeric |
| `Null` | — | returned by `return`-less funcs, void calls |

**No string concatenation. No arrays/structs. No object literals.** (v1.0 scope;
see Extension Protocol.)

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
| `expr` | `f(...)` | statement call (result discarded) |
| `block` | `{ ... }` | nested scope |

---

## 8. Built-in Function Registry

### 8.1 Entity transforms — **only valid inside an entity handler** (have an entity context)

| Function | Signature | Effect |
|---|---|---|
| `move` | `move(dx, dy, dz)` | translate by delta |
| `setPos` | `setPos(x, y, z)` | absolute position |
| `setScale` | `setScale(s)` | absolute uniform scale |
| `scaleBy` | `scaleBy(f)` | scale *= f |
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
defaults to `Null` when omitted. Reentrancy: globals are snapshotted and restored
around a call (simple, deterministic).

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

---

## 13. Limitations (v1.0)

- f64 numbers only; no string concat in expressions.
- No arrays/structs/objects; state via `let` globals + entity transforms.
- No iteration/while loops (only bounded, deterministic `func` recursion if any).
- Tuples are homogeneous numeric vectors (position/color), not general values.
- Single world per load; no multi-world scene graphs yet.
- `points` builder is recognized by the parser but currently renders as `solid`.

---

## 14. References

- Normative grammar: `docs/adilang.ebnf`
- Implementation: `src/*.rs`
- Example world: `worlds/default.adi`
- Learning dataset: `docs/ADILANG_KNOWLEDGE.md`
