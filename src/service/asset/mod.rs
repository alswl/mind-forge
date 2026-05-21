use std::fs;
use std::io::Read;
use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::asset::AssetKind;

pub use self::add::{add, AddArgs};
pub use self::clean::clean;
pub use self::index::reconcile;
pub use self::list::list;
pub use self::remove::remove_asset;
pub use self::rename::rename_asset;
pub use self::update::{set_publish_url, update_all, update_one};

// ── SHA-256 file hashing ─────────────────────────────────────────────────────

pub fn sha256_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path).map_err(MfError::Io)?;
    let mut reader = std::io::BufReader::with_capacity(8192, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader.read(&mut buffer).map_err(MfError::Io)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher.finalize_hex())
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    len_bits: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19],
            buffer: [0; 64],
            buffer_len: 0,
            len_bits: 0,
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        self.len_bits = self.len_bits.wrapping_add((input.len() as u64) * 8);

        if self.buffer_len > 0 {
            let take = (64 - self.buffer_len).min(input.len());
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&input[..take]);
            self.buffer_len += take;
            input = &input[take..];
            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
        }

        for chunk in input.chunks_exact(64) {
            self.compress(chunk);
        }

        let remainder = input.len() % 64;
        if remainder > 0 {
            let start = input.len() - remainder;
            self.buffer[..remainder].copy_from_slice(&input[start..]);
            self.buffer_len = remainder;
        }
    }

    fn finalize_hex(mut self) -> String {
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..].fill(0);
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }

        self.buffer[self.buffer_len..56].fill(0);
        self.buffer[56..64].copy_from_slice(&self.len_bits.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut out = String::with_capacity(64);
        for word in self.state {
            out.push_str(&format!("{word:08x}"));
        }
        out
    }

    fn compress(&mut self, block: &[u8]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
            0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
            0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
            0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
            0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
            0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
            0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks_exact(4).take(16).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

// ── Extension-based asset kind inference ─────────────────────────────────────

const EXTENSION_MAP: &[(&str, AssetKind)] = &[
    ("png", AssetKind::Image),
    ("jpg", AssetKind::Image),
    ("jpeg", AssetKind::Image),
    ("gif", AssetKind::Image),
    ("webp", AssetKind::Image),
    ("svg", AssetKind::Image),
    ("bmp", AssetKind::Image),
    ("mp4", AssetKind::Video),
    ("mov", AssetKind::Video),
    ("webm", AssetKind::Video),
    ("mkv", AssetKind::Video),
    ("avi", AssetKind::Video),
    ("mp3", AssetKind::Audio),
    ("wav", AssetKind::Audio),
    ("flac", AssetKind::Audio),
    ("ogg", AssetKind::Audio),
    ("m4a", AssetKind::Audio),
];

pub(crate) fn infer_kind(extension: Option<&std::ffi::OsStr>) -> AssetKind {
    let ext = extension.and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some(e) => EXTENSION_MAP.iter().find(|(k, _)| *k == e).map(|(_, kind)| *kind).unwrap_or(AssetKind::Other),
        None => AssetKind::Other,
    }
}

// ── Symlink helper ──────────────────────────────────────────────────────────

#[cfg(unix)]
pub(crate) fn create_symlink(src: &Path, dst: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dst).map_err(MfError::Io)
}

#[cfg(not(unix))]
fn create_symlink(_src: &Path, _dst: &Path) -> Result<()> {
    Err(MfError::usage(
        "symlink is not supported on this platform",
        Some("use --copy or omit --link to copy the file".to_string()),
    ))
}

pub mod add;
pub mod clean;
pub mod index;
pub mod list;
pub mod remove;
pub mod rename;
pub mod update;
