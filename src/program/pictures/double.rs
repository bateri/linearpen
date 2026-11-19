//! What stands in for the terminal in the tests: it holds what is attached, gives the files the person
//! "gave" by their handles, takes the files that are saved, and says which bytes were let go.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::{Files, FilesError};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
pub struct Double {
    /// The bytes attached, in order.
    attached: Mutex<Vec<Vec<u8>>>,
    /// The names let go, in order.
    detached: Mutex<Vec<String>>,
    /// The name the page's own attach gave bytes: what the terminal would answer with, so that the
    /// page draws the picture the program attached.
    names: Mutex<HashMap<Vec<u8>, String>>,
    /// The files the person gave, by handle.
    given: Mutex<HashMap<String, Vec<u8>>>,
    /// The handles that were received, in order.
    read: Mutex<Vec<String>>,
    /// The files saved, by the handle of the place and what was sent to it.
    saved: Mutex<Vec<(String, Vec<u8>)>>,
    /// The handles of files that are over the most.
    huge: Mutex<Vec<String>>,
    /// Saving fails (the place is full).
    save_fails: Mutex<bool>,
    /// No view that holds files runs.
    unsupported: Mutex<bool>,
}

impl Double {
    /// The bytes the page was `attach`ed under `name` (the name the terminal made of them).
    pub fn know(&self, bytes: &[u8], name: &str) {
        lock(&self.names).insert(bytes.to_vec(), name.to_owned());
    }

    /// The file the person gave, by `handle`.
    pub fn give(&self, handle: &str, bytes: &[u8]) {
        lock(&self.given).insert(handle.to_owned(), bytes.to_vec());
    }

    /// The file the person gave by `handle` is more than a file may be (none of it is held here).
    pub fn give_huge(&self, handle: &str) {
        lock(&self.huge).push(handle.to_owned());
    }

    /// From now on a save fails.
    pub fn save_fails(&self) {
        *lock(&self.save_fails) = true;
    }

    /// From now on no view holds files.
    pub fn no_view(&self) {
        *lock(&self.unsupported) = true;
    }

    pub fn attached(&self) -> Vec<Vec<u8>> {
        lock(&self.attached).clone()
    }

    pub fn detached(&self) -> Vec<String> {
        lock(&self.detached).clone()
    }

    pub fn read(&self) -> Vec<String> {
        lock(&self.read).clone()
    }

    pub fn saved(&self) -> Vec<(String, Vec<u8>)> {
        lock(&self.saved).clone()
    }
}

impl Files for Double {
    fn attach(&self, bytes: Vec<u8>) -> Result<String, FilesError> {
        if *lock(&self.unsupported) {
            return Err(FilesError::Unsupported);
        }
        let name = lock(&self.names).get(&bytes).cloned().unwrap_or_else(|| {
            // (A name no page drew by: the digest the terminal makes is another's work.)
            let sum = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |sum, byte| {
                (sum ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
            });
            format!("sha256:{sum:064x}")
        });
        lock(&self.attached).push(bytes);
        Ok(name)
    }

    fn detach(&self, name: &str) {
        lock(&self.detached).push(name.to_owned());
    }

    fn receive(&self, handle: &str, to: &Path, most: u64) -> Result<u64, FilesError> {
        lock(&self.read).push(handle.to_owned());
        if lock(&self.huge).iter().any(|huge| huge == handle) {
            return Err(FilesError::TooLarge);
        }
        let given = lock(&self.given);
        let bytes = given.get(handle).ok_or(FilesError::Refused)?;
        if bytes.len() as u64 > most {
            return Err(FilesError::TooLarge);
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)
            .map_err(|_| FilesError::Refused)?;
        file.write_all(bytes).map_err(|_| FilesError::Refused)?;
        Ok(bytes.len() as u64)
    }

    fn save(&self, handle: &str, from: &Path) -> Result<u64, FilesError> {
        if *lock(&self.unsupported) {
            return Err(FilesError::Unsupported);
        }
        if *lock(&self.save_fails) {
            return Err(FilesError::Refused);
        }
        let bytes = std::fs::read(from).map_err(|_| FilesError::Refused)?;
        let size = bytes.len() as u64;
        lock(&self.saved).push((handle.to_owned(), bytes));
        Ok(size)
    }
}

// --- Pictures to draw ----------------------------------------------------------------------------------

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// A PNG of `width` by `height` points that a viewer takes: a soft gradient from `tint` to a light
/// grey with a bar of the tint across the middle, so that a frame shows a picture and not a block of
/// colour. Not compressed (stored deflate blocks).
pub fn png(width: u32, height: u32, tint: [u8; 3]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(((width * 3 + 1) * height) as usize);
    for y in 0..height {
        raw.push(0);
        for x in 0..width {
            let across = f64::from(x) / f64::from(width.max(1));
            let down = f64::from(y) / f64::from(height.max(1));
            let bar = (down - 0.5).abs() < 0.08;
            for (at, channel) in tint.iter().enumerate() {
                let light = 236.0 - 30.0 * across - 24.0 * down - 6.0 * at as f64;
                let value = if bar {
                    f64::from(*channel)
                } else {
                    light * (0.55 + 0.45 * (1.0 - across)) + f64::from(*channel) * 0.18 * across
                };
                raw.push(value.clamp(0.0, 255.0) as u8);
            }
        }
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        let length = u16::try_from(block.len()).unwrap();
        z.extend_from_slice(&length.to_le_bytes());
        z.extend_from_slice(&(!length).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// The picture Linear gives a test that stored none for the address (640 by 360, in the blue of the
/// page's accent).
pub fn the_picture() -> Vec<u8> {
    png(640, 360, [64, 110, 230])
}

/// A PDF of `pages` pages that a reader takes: each page is a coloured bar and the words `Page n` (in a
/// font every reader has), written as a PDF is, with the table of where each object is.
pub fn pdf(pages: usize) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());
    let first = 4;
    let kids: Vec<String> = (0..pages)
        .map(|n| format!("{} 0 R", first + 2 * n))
        .collect();
    objects.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {pages} /MediaBox [0 0 420 560] >>",
        kids.join(" ")
    ));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned());
    for n in 0..pages {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>",
            first + 2 * n + 1
        ));
        let tint = [(0.25, 0.43, 0.9), (0.9, 0.5, 0.2), (0.25, 0.7, 0.45)][n % 3];
        let content = format!(
            "q {} {} {} rg 40 440 340 80 re f Q BT /F1 36 Tf 40 360 Td (Page {}) Tj ET BT /F1 14 Tf 40 320 Td (The text of the page) Tj ET",
            tint.0,
            tint.1,
            tint.2,
            n + 1
        );
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (at, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", at + 1).as_bytes());
    }
    let table = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{table}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod pdf_tests {
    #[test]
    fn the_test_pdf_says_its_pages_and_its_size_to_the_scan() {
        let bytes = super::pdf(3);
        let facts = crate::linear::pdf::facts(&bytes);
        assert_eq!(facts.pages, Some(3));
        assert_eq!(facts.size, Some((420, 560)));
    }
}
