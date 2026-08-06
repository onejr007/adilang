# ADILang (Agent Distributed Intelligence Language)

> **Bahasa Protokol / IR Universal AI-to-AI**  
> Diciptakan dan dikembangkan oleh **ADI (Agent Distributed Intelligence)**  
> Lead Developer: **BAGAS ADI PRATAMA S,Kom.**

ADILang adalah **bahasa Intermediate Representation (IR) & protokol komunikasi murni antar-AI**. ADILang **bukan** bahasa pemrograman frontend atau renderer visual 3D — melainkan protokol khusus yang dirancang agar sistem AI/LLM dapat **membaca, memproses, memvalidasi, dan menegosiasikan mental state** (keinginan, tugas, memori, rencana, dan status) secara **deterministik, terstruktur, dan hemat token**.

---

## 🎯 Mengapa ADILang? ("What the AI Thinks")

* **MCP (Anthropic)**: Menangani *"What the AI DOES"* (Panggilan tools & integrasi sumber daya).
* **A2A (Google)**: Menangani *"Who does WHAT"* (Delegasi tugas berbasis HTTP/REST).
* **ADILang**: Menangani **"What the AI THINKS"** (Mental State IR: Intent, Reply, Task, Event, Memory, Plan, State).

---

## 🧩 Struktur Modul Protokol AI-to-AI

ADILang terdiri dari modul-modul terstruktur dengan *closed vocabulary*:

| Modul | Contoh Sintaks | Fungsi Protokol AI-to-AI |
| :--- | :--- | :--- |
| **`intent`** | `intent "ask" { mode "MODE_CODE_ENGINEERING" payload "..." }` | Normalisasi kanonik dari setiap permintaan/perintah AI. |
| **`reply`** | `reply "answer" { mode "..." content "..." recs [ "..." ] }` | Tanggapan terstruktur dari AI pengirim (konten + opsi rekomendasi). |
| **`task`** | `task "code_review" { assign "agent_1" input "..." expect "..." }` | Perintah kerja & delegasi tugas ke agen lain (*multi-agent orchestration*). |
| **`event`** | `event "syntax_error" { source "..." line "10" guidance "..." }` | Telemetri & catatan kejadian sistem real-time untuk audit & *self-healing*. |
| **`memory`** | `memory "user_fact" { key "pref" fact "..." confidence "0.95" }` | Pertukaran fakta/memori jangka panjang tanpa membengkakkan riwayat chat. |
| **`plan`** | `plan "build" { steps [ "1:research:", "2:code:1" ] parallel "0" }` | Formulasi Directed Acyclic Graph (DAG) langkah eksekusi antar-agen. |
| **`state`** | `state "stream" { user_key "..." status "active" progress "50%" }` | Sinkronisasi status *runtime* real-time antar-agen. |

---

## ⚡ Keunggulan Utama ADILang

1. **Deterministik & Validasi Ketat**: Sintaks divalidasi terhadap grammar EBNF ([docs/adilang.ebnf](docs/adilang.ebnf)). Kunci acak/tidak dikenal akan ditolak, memicu *self-healing retry loop*.
2. **Efisiensi Token Terukur**: Menghemat **−21% hingga −47% token** dibanding format JSON/YAML setara.
3. **Multi-Agent Interoperability**: Menyediakan jembatan bawaan ke **MCP** (Anthropic) dan **A2A** (Google).

---

## 🚀 Pemasangan & Penggunaan Mandiri (Standalone Python SDK)

ADILang dapat di-install dan digunakan di aplikasi Python atau agen AI buatan Anda tanpa ketergantungan pada sistem luar:

```bash
cd python
pip install -e .
```

### Contoh Kode Python Mandiri:
```python
import adilang

# 1. AI Anda membuat pesan Intent IR
intent_ir = adilang.encode_intent(
    mode="MODE_CODE_ENGINEERING",
    payload="Buatkan saya fungsi Python fibonacci",
    verb="command"
)
print(intent_ir)

# 2. Parse pesan ADILang IR menjadi Dictionary Python
parsed = adilang.parse_adilang(intent_ir)
print(parsed["intent"]["payload"])

# 3. Validasi sintaks terhadap closed vocabulary
errors = adilang.validate_adilang(intent_ir)
print("Validation Errors:", errors)  # [] jika valid
```

### Penggunaan Alat Baris Perintah (`adilang-cli`)
```bash
# Periksa & validasi file .adi
adilang-cli check pesan.adi

# Convert file .adi ke JSON IR
adilang-cli parse pesan.adi

# Perbaiki kesalahan sintaks/kunci secara otomatis
adilang-cli fix pesan.adi
```

---

## 📚 Dokumen Referensi Formal

| Berkas | Isi |
| :--- | :--- |
| [`docs/LANGUAGE.md`](docs/LANGUAGE.md) | Spesifikasi bahasa: semantik modul protocol/IR & aturan ekstensi. |
| [`docs/adilang.ebnf`](docs/adilang.ebnf) | Formal W3C-EBNF Grammar (machine-parseable). |
| [`docs/ADILANG_KNOWLEDGE.md`](docs/ADILANG_KNOWLEDGE.md) | Master Knowledge Base = dataset untuk AI lain mempelajari ADILang. |
