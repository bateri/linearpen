//! What a PDF says of itself where a program that has no PDF reader can find it: the number of its
//! pages and the size of its first page. The terminal draws the pages (a page of the kit,
//! `image(name):page(n)`); this only tells the page how many there are and how large to make the box.
//! The words are searched for in the bytes as they are, so a file that keeps them in a compressed
//! stream (the newest kind of PDF may) says nothing here: the numbers are then unknown, which the page
//! lives with (`None`).

/// The page box of a PDF with no size of its own (a Letter page, in points).
pub const DEFAULT_SIZE: (u32, u32) = (612, 792);

/// The most pages the program says a PDF has.
const PAGES_MOST: u32 = 5000;

/// What was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    /// How many pages, where the file says.
    pub pages: Option<u32>,
    /// The first page's `MediaBox`, whole points, where the file says.
    pub size: Option<(u32, u32)>,
}

/// Where `needle` is in `hay`, from `from`.
fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() || needle.is_empty() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| at + from)
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | 0x0c | 0)
}

/// The position after the white space that starts at `at`.
fn skip(hay: &[u8], mut at: usize) -> usize {
    while at < hay.len() && is_space(hay[at]) {
        at += 1;
    }
    at
}

/// The whole number that starts at `at` (white space before it allowed), and where it ends.
fn number(hay: &[u8], at: usize) -> Option<(f64, usize)> {
    let start = skip(hay, at);
    let mut end = start;
    while end < hay.len() && (hay[end].is_ascii_digit() || matches!(hay[end], b'-' | b'+' | b'.')) {
        end += 1;
    }
    let text = std::str::from_utf8(&hay[start..end]).ok()?;
    text.parse::<f64>().ok().map(|value| (value, end))
}

/// What `bytes`, a PDF, says of its pages.
#[must_use]
pub fn facts(bytes: &[u8]) -> Facts {
    Facts {
        pages: pages(bytes),
        size: media_box(bytes),
    }
}

/// What one object (the words between one `obj` and the next) says of the pages.
#[derive(Default)]
struct Node {
    /// It is of `/Type /Pages`.
    tree: bool,
    /// Its first `/Count` was met (whether or not it was a number).
    counted: bool,
    count: Option<f64>,
}

impl Node {
    /// The object is over: a page tree node with a count that is a number of pages makes the most.
    fn close(&mut self, most: &mut Option<u32>) {
        if self.tree
            && let Some(value) = self.count
            && value >= 1.0
            && value <= f64::from(PAGES_MOST)
        {
            let value = value as u32;
            *most = Some(most.map_or(value, |had| had.max(value)));
        }
        *self = Self::default();
    }
}

/// The pages: the `/Count` of the page tree's root, which is the largest `/Count` among the objects
/// that are of `/Type /Pages` (the nodes under it count fewer), or else how many objects are of `/Type
/// /Page`. The bytes are read once, from the front to the end (a file made of the same words over and
/// over takes no longer than any other of its size): an object is what lies between one `obj` and the
/// next, and its `/Count` is its first.
fn pages(bytes: &[u8]) -> Option<u32> {
    let mut most: Option<u32> = None;
    let mut singles: u32 = 0;
    let mut node = Node::default();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'o' if bytes[at..].starts_with(b"obj") => {
                node.close(&mut most);
                at += 3;
                continue;
            }
            b'/' if bytes[at..].starts_with(b"/Type") => {
                let after = skip(bytes, at + 5);
                if bytes[after..].starts_with(b"/Page") {
                    match bytes.get(after + 5) {
                        Some(b's') => node.tree = true,
                        Some(byte) if byte.is_ascii_alphanumeric() => {}
                        _ => singles += 1,
                    }
                }
                at += 5;
                continue;
            }
            b'/' if bytes[at..].starts_with(b"/Count") && !node.counted => {
                node.counted = true;
                node.count = number(bytes, at + 6).map(|(value, _)| value);
                at += 6;
                continue;
            }
            _ => {}
        }
        at += 1;
    }
    node.close(&mut most);
    most.or((1..=PAGES_MOST).contains(&singles).then_some(singles))
}

/// The first `/MediaBox [x1 y1 x2 y2]`: its width and height, whole points.
fn media_box(bytes: &[u8]) -> Option<(u32, u32)> {
    let at = find(bytes, b"/MediaBox", 0)?;
    let open = skip(bytes, at + 9);
    if bytes.get(open) != Some(&b'[') {
        return None;
    }
    let mut at = open + 1;
    let mut values = [0.0_f64; 4];
    for value in &mut values {
        let (got, end) = number(bytes, at)?;
        *value = got;
        at = end;
    }
    let width = (values[2] - values[0]).abs().round();
    let height = (values[3] - values[1]).abs().round();
    ((1.0..=20_000.0).contains(&width) && (1.0..=20_000.0).contains(&height))
        .then_some((width as u32, height as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pages_and_the_size_are_read_from_the_words_of_a_plain_pdf() {
        let pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Count 3 /Kids [3 0 R 4 0 R 5 0 R] /MediaBox [0 0 595.28 841.89] >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R >>\nendobj\n";
        assert_eq!(
            facts(pdf),
            Facts {
                pages: Some(3),
                size: Some((595, 842))
            }
        );
    }

    #[test]
    fn without_a_count_the_pages_are_counted_and_without_either_they_are_not_known() {
        let pdf = b"%PDF-1.4\n3 0 obj\n<</Type/Page>>\nendobj\n4 0 obj\n<</Type /Page /Parent 2 0 R>>\nendobj\n";
        assert_eq!(facts(pdf).pages, Some(2));
        assert_eq!(facts(b"%PDF-1.7\nstream\n\x00\x01\nendstream").pages, None);
        assert_eq!(facts(b"%PDF-1.7").size, None);
        // An outline that counts more than the pages is not the pages.
        let outline = b"1 0 obj\n<< /Type /Outlines /Count 40 >>\nendobj\n2 0 obj\n<< /Type /Pages /Count 2 >>\nendobj\n";
        assert_eq!(facts(outline).pages, Some(2));
        // A size that is nonsense is none.
        assert_eq!(facts(b"/MediaBox [0 0 0 0]").size, None);
    }

    #[test]
    fn a_file_made_to_be_slow_is_read_once_through() {
        // The same words over and over, 8 MiB of them: with no `endobj` after a page tree node, or no
        // `obj` before it, each of a half a million would send the reader to the ends of the file.
        let started = std::time::Instant::now();
        for unit in [
            &b"/Type /Pages "[..],
            b"obj /Type /Pages ",
            b"/Type /Pages /Count 7 ",
            b"/Type /Page ",
            b"/Count /Count /Type ",
            b"/MediaBox ",
        ] {
            let bytes: Vec<u8> = unit.iter().copied().cycle().take(8 * 1024 * 1024).collect();
            let _ = facts(&bytes);
        }
        // (A debug build reads that in a fraction of a second; a quadratic reader would not end.)
        assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
        // The single pass says what the longer scans said.
        let tree = b"1 0 obj << /Count 9 /Type /Pages >> endobj 2 0 obj << /Type /Pages /Count 4 /Count 99 >> endobj";
        assert_eq!(facts(tree).pages, Some(9));
    }
}
