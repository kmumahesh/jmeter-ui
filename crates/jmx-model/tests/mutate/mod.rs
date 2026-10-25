//! A small, repeatable "mutation fuzzer" for tests: it takes a real input and damages it in ways that tend to
//! upset parsers (flipped bytes, cut or doubled pieces, stray markup characters, truncation). The same seed always
//! gives the same damage, so a failure can be repeated. Included by other crates' tests with `#[path]`.
//!
//! The libFuzzer targets in `fuzz/` explore much further; these tests keep the same checks in the normal test run.

#![allow(dead_code)]

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

const BYTES: &[u8] = &[b'<', b'>', b'&', b'"', b'\'', b'/', b'=', b'\n', b'\r', b']', b'[', b'!', b'?', b';', b'#', 0, 0xff, 0xc3];

const PIECES: &[&str] = &[
    "<!--", "-->", "<![CDATA[", "]]>", "<!DOCTYPE x [", "<!ENTITY a \"b\">", "&#x", "&#0;", "&amp;", "&lt;", "&bogus;", "<?xml ", "?>",
    "<", "</", "/>", "\"", "'", ",", "\r\n", "\u{feff}", "\u{0}",
];

/// Damage `seed` in one to four ways.
pub fn mutate(rng: &mut Rng, seed: &[u8]) -> Vec<u8> {
    let mut v = seed.to_vec();
    for _ in 0..1 + rng.below(4) {
        if v.is_empty() {
            v.push(BYTES[rng.below(BYTES.len())]);
            continue;
        }
        let at = rng.below(v.len());
        match rng.below(7) {
            0 => v[at] ^= 1 << rng.below(8),
            1 => v[at] = BYTES[rng.below(BYTES.len())],
            2 => {
                let end = (at + 1 + rng.below(64)).min(v.len());
                v.drain(at..end);
            }
            3 => {
                let end = (at + 1 + rng.below(256)).min(v.len());
                let piece = v[at..end].to_vec();
                v.splice(at..at, piece);
            }
            4 => {
                let piece = PIECES[rng.below(PIECES.len())].as_bytes().to_vec();
                v.splice(at..at, piece);
            }
            5 => v.truncate(at),
            _ => {
                let other = rng.below(v.len());
                v.swap(at, other);
            }
        }
    }
    v
}
