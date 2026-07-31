# ADILang

**Bahasa protokol / IR yang dirancang AI (ADI), untuk AI.**

ADILang adalah bahasa utama (protocol / intermediate-representation) di ekosistem
**ADI (Agent Distributed Intelligence)** — diciptakan dan dikembangkan oleh ADI
sendiri, untuk dipakai antar-AI dan oleh backend ADI. Manusia tidak perlu
mempelajarinya: ADILang dibuat khusus agar AI dapat **membaca, menghasilkan,
memverifikasi, dan memperluasnya** secara deterministik, rendah ambiguitas, dan
hemat token.

ADILang punya **satu modul per dokumen**:

| Modul | Bentuk | Fungsi | Runtime |
|---|---|---|---|
| `intent` | `intent "<verb>" { ... }` | Representasi ter-normalisasi dari setiap permintaan/chat user. | Backend ADI (Python) |
| `reply` | `reply "<kind>" { ... }` | Jawaban terstruktur ADI (konten + metadata). | Backend ADI |
| `task` | `task "<name>" { ... }` | Perintah kerja agent (CrewAI). | CrewAI |
| `event` | `event "<name>" { ... }` | Catatan kejadian/fakta sistem. | Backend ADI |
| `memory` | `memory "<name>" { ... }` | Pertukaran fakta/konteks jangka panjang antar-agen (v1.5.0). | Backend ADI |
| `plan` | `plan "<name>" { ... }` | DAG langkah eksekusi sekuensial/paralel untuk CrewAI (v1.5.0). | CrewAI |
| `world` | `world "<name>" { ... }` | Dunia 3D interaktif (hologram). | Rust → WASM → WebGL2 |

> **Pencipta & Developer**: ADI (Agent Distributed Intelligence)
> **Status**: v1.8.0 — STABLE
> **Repo**: https://github.com/onejr007/adilang

---

## Dokumentasi (normatif)

| File | Isi |
|---|---|
| [`docs/LANGUAGE.md`](docs/LANGUAGE.md) | Spesifikasi bahasa: semantik, modul protocol/IR, registry builtin, model eksekusi, protokol ekstensi. |
| [`docs/adilang.ebnf`](docs/adilang.ebnf) | Grammar formal W3C-EBNF (machine-parseable). |
| [`docs/ADILANG_KNOWLEDGE.md`](docs/ADILANG_KNOWLEDGE.md) | Knowledge base = dataset untuk AI lain belajar ADILang. |

## Contoh minimal

Modul `intent` (setiap chat user diterjemahkan menjadi blok ini):

```
intent "ask" {
    mode "MODE_CODE_ENGINEERING"
    payload "buatkan script python fibonacci"
    verb "ask"
}
```

Modul `world` (dunia 3D / hologram):

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

## API WASM (host) — modul world

| Export | Signature | Purpose |
|---|---|---|
| `adilang_start(canvas_id)` | `Result<(), String>` | init engine + loop render |
| `adilang_load(source)` | `Result<(), String>` | hot-reload world |
| `adilang_check(source)` | `Result<(), String>` | validasi sintaks |
| `adilang_speak()` / `adilang_silent()` | `Result<(), String>` | trigger event |
| `adilang_debug_count()` | `usize` | jumlah entity |
| `adilang_version()` | `String` | versi bahasa |
| `adilang_registry()` | `String` | enumerasi kosakata tertutup (P6) |

## ADILang sebagai Protocol / IR di ekosistem ADI

- **Semua input user diterjemahkan ke ADILang.** Setiap chat/perintah dari Telegram
  bot, web, CLI, maupun TMA diproses menjadi satu blok `intent` sebelum diolah lebih
  lanjut — sehingga seluruh pipeline bekerja di atas representasi yang seragam,
  terstruktur, dan deterministik.
- **Hemat token.** Sintaksnya ringkas, tanpa karakter bermakna dari whitespace, dan
  nilai protocol berupa `String`/`String[]` sederhana. LLM tidak perlu mempelajari
  aturan baru untuk memancarkan blok IR.
- **Sistematis & terstruktur.** Semua blok diverifikasi terhadap daftar kunci tertutup;
  kunci duplikat/tidak dikenal ditolak (validasi deterministik).
- **Bisa dipelajari semua LLM.** `docs/ADILANG_KNOWLEDGE.md` adalah corpus belajar
  mandiri — model apa pun bisa memahami ADILang dari nol.

## Ekstensibilitas

ADILang dirancang agar AI dapat meng-improve tanpa koordinasi — lihat
**Protokol Ekstensi** di `docs/LANGUAGE.md` §11. Aturan inti:
- minor/patch = **additive only**;
- breaking change = wajib MAJOR bump + perbarui seluruh docs + KB;
- setiap fitur baru wajib sinkron: grammar ↔ parser ↔ evaluator ↔ docs ↔ KB ↔ unit test.
