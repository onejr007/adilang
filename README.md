# ADILang

**Bahasa pemrograman 3D yang dirancang AI, untuk AI.**

ADILang adalah domain-specific language (DSL) untuk membangun **3D virtual world /
hologram** yang dirender via **WebGL2**, dijalankan sebagai **WASM (Rust)**. Bahasa ini
dibuat agar sebuah model AI dapat **membaca, menghasilkan, memverifikasi, dan
memperluas** dunia 3D dengan deterministik, rendah ambiguitas, dan hemat token —
tanpa ergonomi manusia.

> **Pencipta**: ADI (AI Agent Ecosystem)
> **Status**: v1.0.0 — STABLE
> **Repo**: https://github.com/onejr007/adilang

---

## Dokumentasi (normatif)

| File | Isi |
|---|---|
| [`docs/LANGUAGE.md`](docs/LANGUAGE.md) | Spesifikasi bahasa: semantik, registry builtin, model eksekusi, protokol ekstensi. |
| [`docs/adilang.ebnf`](docs/adilang.ebnf) | Grammar formal W3C-EBNF (machine-parseable). |
| [`docs/ADILANG_KNOWLEDGE.md`](docs/ADILANG_KNOWLEDGE.md) | Knowledge base = dataset untuk AI lain belajar ADILang. |

## Contoh minimal

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

## Struktur repo

```
adilang/
  Cargo.toml
  src/
    lexer.rs      # tokenizer
    ast.rs        # AST
    parser.rs     # recursive descent
    eval.rs       # interpreter (tree-walking)
    scene.rs      # model dunia 3D
    math3d.rs     # mat4 / vec3
    engine.rs     # renderer WebGL2 (glow) — hanya wasm32
    wasm_api.rs   # wasm-bindgen boundary — hanya wasm32
    lib.rs        # entry + unit tests
  worlds/
    default.adi   # world script bawaan
  docs/           # spesifikasi + grammar + knowledge base
```

## Build & uji

```bash
cargo test                        # uji native (lexer/parser/eval/math3d)
cargo build --target wasm32-unknown-unknown
wasm-pack build --target web      # → pkg/ (WASM + JS loader)
```

Syarat toolchain: Rust stable + target `wasm32-unknown-unknown` + `wasm-pack`.

## API WASM (host)

| Export | Signature | Purpose |
|---|---|---|
| `adilang_start(canvas_id)` | `Result<(), String>` | init engine + loop render |
| `adilang_load(source)` | `Result<(), String>` | hot-reload world |
| `adilang_check(source)` | `Result<(), String>` | validasi sintaks |
| `adilang_speak()` / `adilang_silent()` | `Result<(), String>` | trigger event |
| `adilang_debug_count()` | `usize` | jumlah entity |
| `adilang_version()` | `String` | versi bahasa |

## Ekstensibilitas

ADILang dirancang agar AI dapat meng-improve tanpa koordinasi — lihat
**Protokol Ekstensi** di `docs/LANGUAGE.md` §11. Aturan inti:
- minor/patch = **additive only**;
- breaking change = wajib MAJOR bump + perbarui seluruh docs + KB;
- setiap fitur baru wajib sinkron: grammar ↔ parser ↔ evaluator ↔ docs ↔ KB ↔ unit test.
