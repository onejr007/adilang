// ADILang DevServer + HMR — `adi dev` (v1.13.0).
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).
//
// HTTP server statis + WebSocket HMR, TANPA dependency eksternal (std only).
// Kompilasi hanya untuk target native (`#[cfg(not(target_arch = "wasm32"))]`
// di lib.rs) sehingga target wasm32 tetap bersih.
//
// Protokol HMR:
//   connect   → server mengirim teks frame `HMR_CONNECT` (handshake).
//   perubahan → server mengirim teks frame `HMR_RELOAD` ke semua klien.
//   klien     → memuat ulang source dan memanggil adilang_load / re-render.
//
// Implementasi WebSocket server (RFC 6455) minimal: text frame out,
// menangani ping/pong/close dari klien. SHA-1 diimplementasikan inline.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Frame teks yang dikirim saat koneksi WebSocket HMR dibuka.
pub const HMR_CONNECT: &str = "HMR_CONNECT";
/// Frame teks yang dikirim saat file `*.adi` berubah.
pub const HMR_RELOAD: &str = "HMR_RELOAD";
/// Path WebSocket endpoint di DevServer.
pub const WS_PATH: &str = "/__adi_ws";

/// Jalankan DevServer: HTTP statis di `root` + WebSocket HMR di `port`.
/// `port == 0` → port acak (dipakai tes). Memantau `**/*.adi` di `root`.
/// Blocking sampai error atau penerima di-drop.
pub fn serve(port: u16, root: &Path) -> Result<u16, String> {
    let root = root.to_path_buf();
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("Gagal bind port {port}: {e}"))?;
    let bound = listener
        .local_addr()
        .map_err(|e| format!("local_addr gagal: {e}"))?
        .port();
    eprintln!(
        "adi dev: http://127.0.0.1:{bound}  (root: {})",
        root.display()
    );

    let clients: Arc<Mutex<Vec<Sender<()>>>> = Arc::new(Mutex::new(Vec::new()));
    let watcher_root = root.clone();
    let watcher_clients = clients.clone();
    let watcher = std::thread::spawn(move || watch_loop(watcher_root, watcher_clients));
    let _ = watcher;

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let clients = clients.clone();
                let root = root.clone();
                std::thread::spawn(move || {
                    let _ = handle_connection(s, &root, &clients);
                });
            }
            Err(e) => eprintln!("adi dev: accept error: {e}"),
        }
    }
    Ok(bound)
}

fn handle_connection(
    mut stream: TcpStream,
    root: &Path,
    clients: &Arc<Mutex<Vec<Sender<()>>>>,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    // Baca header request (sampai \r\n\r\n)
    loop {
        let n = stream
            .read(&mut tmp)
            .map_err(|e| format!("read request: {e}"))?;
        if n == 0 {
            return Err("EOF saat baca request".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 64 * 1024 {
            return Err("Request terlalu besar".into());
        }
    }

    let head = String::from_utf8_lossy(&buf);
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or("GET / HTTP/1.1").to_string();
    let mut headers: HashMap<String, String> = HashMap::new();
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let target = parts.next().unwrap_or("/");
    if method != "GET" {
        write_http(&mut stream, 405, "Method Not Allowed", b"only GET", "text/plain")?;
        return Ok(());
    }

    let is_ws = target.starts_with(WS_PATH) && headers.get("upgrade").map(|u| u.to_ascii_lowercase()) == Some("websocket".into());
    if is_ws {
        return websocket_upgrade(&mut stream, &headers, clients);
    }

    serve_file(&mut stream, root, target)
}

fn serve_file(stream: &mut TcpStream, root: &Path, target: &str) -> Result<(), String> {
    let path_str = if target == "/" { "index.html" } else { target.trim_start_matches('/') };
    let mut p = root.join(path_str);
    if p.is_dir() {
        p = p.join("index.html");
    }
    // Anti path traversal: pastikan hasil ada di dalam root.
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let canonical = p.canonicalize().unwrap_or_else(|_| p.clone());
    if !canonical.starts_with(&canonical_root) {
        return write_http(stream, 403, "Forbidden", b"forbidden", "text/plain");
    }

    match fs::read(&p) {
        Ok(body) => {
            // v1.13.0: HMR bootstrap — HTML berisi <script type="text/adi">
            // mendapat wiring WebSocket /__adi_ws + JIT engine (auto, tanpa
            // menyentuh file sumber). HMR_RELOAD memicu _hmrReload di engine.
            let mut body = body;
            const ADI_MARKER: &[u8] = b"type=\"text/adi\"";
            if p.extension().map(|e| e == "html").unwrap_or(false)
                && body.windows(ADI_MARKER.len()).any(|w| w == ADI_MARKER)
            {
                let injected = br#"
<script>
(function () {
  var t = setInterval(function () {
    if (window.__adiEngine && window.__adiEngine.enableHMR) {
      clearInterval(t);
      if (window.WebSocket) {
        window.__adiEngine.enableHMR('/__adi_ws', null);
        window.__adiEngine.on('hmr', function () { console.log('[adi dev] HMR_RELOAD'); });
      }
    }
  }, 50);
  setTimeout(function () { clearInterval(t); }, 8000);
})();
</script>
"#;
                let mut out = Vec::with_capacity(body.len() + injected.len() + 32);
                out.extend_from_slice(&body);
                out.extend_from_slice(injected);
                body = out;
            }
            let mime = mime_for(&p);
            let len = body.len();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {len}\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
            stream.write_all(&body).map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(_) => write_http(stream, 404, "Not Found", b"not found", "text/plain"),
    }
}

fn write_http(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    body: &[u8],
    mime: &str,
) -> Result<(), String> {
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    stream.write_all(body).map_err(|e| e.to_string())?;
    Ok(())
}

fn mime_for(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "adib" => "application/octet-stream",
        "adi" | "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

// ── WebSocket (RFC 6455, server minimal) ───────────────────────────────────

fn websocket_upgrade(
    stream: &mut TcpStream,
    headers: &HashMap<String, String>,
    clients: &Arc<Mutex<Vec<Sender<()>>>>,
) -> Result<(), String> {
    let key = headers.get("sec-websocket-key").ok_or("WS: tanpa key")?;
    let accept = ws_accept(key);
    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(resp.as_bytes()).map_err(|e| e.to_string())?;

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    clients.lock().map_err(|_| "mutex poisoned")?.push(tx);

    // Kirim handshake HMR_CONNECT
    ws_write_text(stream, HMR_CONNECT)?;
    eprintln!("adi dev: ws klien terhubung");

    // Thread pembaca: tangani ping/pong/close dari klien.
    let mut read_stream = stream.try_clone().map_err(|e| e.to_string())?;
    let reader = std::thread::spawn(move || ws_read_loop(&mut read_stream));
    // Penulis: tunggu sinyal reload lalu kirim HMR_RELOAD.
    let result = ws_write_loop(stream, &rx);
    let _ = reader.join();
    result
}

fn ws_write_loop(stream: &mut TcpStream, rx: &Receiver<()>) -> Result<(), String> {
    loop {
        match rx.recv() {
            Ok(()) => {
                eprintln!("adi dev: ws kirim HMR_RELOAD");
                ws_write_text(stream, HMR_RELOAD)?
            }
            Err(_) => {
                eprintln!("adi dev: ws write loop selesai (sender drop)");
                return Ok(());
            }
        }
    }
}

fn ws_write_text(stream: &mut TcpStream, text: &str) -> Result<(), String> {
    let payload = text.as_bytes();
    let mut frame = Vec::with_capacity(payload.len() + 10);
    frame.push(0x81); // FIN=1, opcode text
    if payload.len() < 126 {
        frame.push(payload.len() as u8);
    } else if payload.len() < 65536 {
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame).map_err(|e| format!("WS write: {e}"))
}

fn ws_read_loop(stream: &mut TcpStream) {
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => {
                let mut pos = 0usize;
                while pos + 2 <= n {
                    let b0 = buf[pos];
                    let b1 = buf[pos + 1];
                    let opcode = b0 & 0x0F;
                    let masked = b1 & 0x80 != 0;
                    let mut len = (b1 & 0x7F) as usize;
                    pos += 2;
                    if len == 126 {
                        if pos + 2 > n {
                            return;
                        }
                        len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
                        pos += 2;
                    } else if len == 127 {
                        if pos + 8 > n {
                            return;
                        }
                        len = u64::from_be_bytes(buf[pos..pos + 8].try_into().unwrap()) as usize;
                        pos += 8;
                    }
                    if masked {
                        if pos + 4 > n {
                            return;
                        }
                        let mask = [buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]];
                        pos += 4;
                        // payload tidak dibutuhkan — klien hanya kirim ping/close
                    }
                    // Bail bila frame lebih panjang dari buffer (client idle)
                    if pos + len > n {
                        return;
                    }
                    match opcode {
                        0x8 => return,      // close
                        0x9 => { /* ping — server baca saja */ }
                        _ => {}
                    }
                    pos += len;
                }
            }
            Err(_) => return,
        }
    }
}

// SHA-1 (RFC 3174) — inline agar std-only.
fn ws_accept(key: &str) -> String {
    let mut input = key.as_bytes().to_vec();
    input.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let digest = sha1(&input);
    base64_encode(&digest)
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    let mut w = [0u32; 80];
    for chunk in msg.chunks_exact(64) {
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

fn base64_encode(data: &[u8]) -> String {
    const TBL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TBL[(n >> 18) as usize & 63] as char);
        out.push(TBL[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TBL[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TBL[n as usize & 63] as char } else { '=' });
    }
    out
}

// ── File watcher (polling mtime, std only) ─────────────────────────────────

fn watch_loop(root: PathBuf, clients: Arc<Mutex<Vec<Sender<()>>>>) {
    let mut last: HashMap<PathBuf, SystemTime> = HashMap::new();
    loop {
        let mut changed = false;
        let files = collect_adi_files(&root);
        let mut seen: HashMap<PathBuf, SystemTime> = HashMap::new();
        for f in files {
            if let Ok(meta) = fs::metadata(&f) {
                if let Ok(mtime) = meta.modified() {
                    seen.insert(f.clone(), mtime);
                    if last.get(&f) != Some(&mtime) {
                        eprintln!("adi dev: perubahan terdeteksi — {}", f.display());
                        changed = true;
                    }
                }
            }
        }
        // Baseline pertama hanya membangun peta mtime, tanpa broadcast.
        if last.is_empty() {
            last = seen;
            std::thread::sleep(Duration::from_millis(300));
            continue;
        }
        last = seen;
        if changed {
            broadcast_reload(&clients);
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn collect_adi_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let name = e.file_name();
                    if name != "target" && name != ".git" && name != "local_modules" {
                        walk(&p, out);
                    }
                } else if p.extension().map(|x| x == "adi").unwrap_or(false) {
                    out.push(p);
                }
            }
        }
    }
    walk(root, &mut out);
    out
}

/// Kirim HMR_RELOAD ke semua klien. Mengembalikan true bila tak ada klien lagi.
fn broadcast_reload(clients: &Arc<Mutex<Vec<Sender<()>>>>) -> bool {
    let mut guard = match clients.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    guard.retain(|tx| tx.send(()).is_ok());
    guard.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_dan_base64_benar() {
        // vektor RFC 6455: key contoh → accept
        let accept = ws_accept("dGhlIHNhbXBsZSBub25jZQ==");
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn ws_accept_deterministik() {
        assert_eq!(ws_accept("abc"), ws_accept("abc"));
        assert_ne!(ws_accept("abc"), ws_accept("abd"));
    }

    #[test]
    fn mime_umum() {
        assert_eq!(mime_for(Path::new("a.adi")), "text/plain; charset=utf-8");
        assert_eq!(mime_for(Path::new("b.wasm")), "application/wasm");
        assert_eq!(mime_for(Path::new("c.js")), "text/javascript; charset=utf-8");
    }
}
