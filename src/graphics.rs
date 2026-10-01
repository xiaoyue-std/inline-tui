//! Terminal graphics: kitty image protocol passthrough (PNG).
//!
//! Encodes PNG bytes into [kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol.html)
//! escape sequences for terminals that support it (kitty, WezTerm, ghostty;
//! **not** Windows Terminal). No image decoding happens here — pass an
//! already-encoded PNG (e.g. `std::fs::read(path)`), keeping the crate
//! zero-dependency. The image is sized in terminal cells via `columns`/`rows`.
//!
//! ```
//! use inline_tui::graphics::{base64, kitty_png};
//!
//! assert_eq!(base64(b"hello"), "aGVsbG8=");
//!
//! let seq = kitty_png(b"\x89PNG tiny", 40, 12, 1);
//! assert!(seq.starts_with("\x1b_Gf=100,a=T,q=1,i=1,c=40,r=12;"));
//! assert!(seq.ends_with("\x1b\\"));
//! ```

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 (with `=` padding).
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Builds the kitty graphics escape sequence displaying `png` sized to
/// `columns` × `rows` terminal cells. Large payloads are chunked at 4096
/// base64 chars (`m=1` / final `m=0`) per the spec.
pub fn kitty_png(png: &[u8], columns: u16, rows: u16, id: u32) -> String {
    let b64 = base64(png);
    let mut out = String::with_capacity(b64.len() + b64.len() / 4096 * 24 + 64);
    let chunks: Vec<&str> = if b64.len() <= 4096 {
        vec![&b64]
    } else {
        b64.as_bytes().chunks(4096).map(|c| std::str::from_utf8(c).unwrap()).collect()
    };
    if chunks.len() == 1 {
        out.push_str(&format!(
            "\x1b_Gf=100,a=T,q=1,i={id},c={columns},r={rows};{}\x1b\\",
            chunks[0]
        ));
        return out;
    }
    for (i, chunk) in chunks.iter().enumerate() {
        if i == 0 {
            out.push_str(&format!(
                "\x1b_Gf=100,a=T,q=1,i={id},c={columns},r={rows},m=1;{chunk}\x1b\\"
            ));
        } else if i + 1 == chunks.len() {
            out.push_str(&format!("\x1b_Gi={id},q=1,m=0;{chunk}\x1b\\"));
        } else {
            out.push_str(&format!("\x1b_Gi={id},q=1,m=1;{chunk}\x1b\\"));
        }
    }
    out
}

/// Writes the kitty graphics sequence for `png` to stdout.
pub fn print_png(png: &[u8], columns: u16, rows: u16, id: u32) {
    use std::io::Write;
    let mut o = std::io::stdout().lock();
    let _ = o.write_all(kitty_png(png, columns, rows, id).as_bytes());
    let _ = o.flush();
}
