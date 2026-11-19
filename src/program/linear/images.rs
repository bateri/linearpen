//! The files of Linear: which address is one the program may fetch (and send its key to), what a file
//! is by its first bytes (a picture of one of four kinds, a PDF, or any other), how large a picture is
//! (from its header; no picture is decoded and there is no library for it), the Markdown that names a
//! file in a description or a comment, and the line that puts a sent one there.
//!
//! A file is sent up to `FILE_MOST`, of any kind; a picture is drawn, and a file is shown, up to
//! `BYTES_MOST` (it is held in memory then).
//!
//! The picture-reading code (`size_of`, the PNG and JPEG headers) is adapted from textpen's
//! `project/image.rs` (the same owner, the same licence).

/// Where the pictures of Linear are: the only address that is fetched, with the key or without it.
/// The host is the whole of it, which is why a prefix is enough: a user name (`...app@evil/`), a port
/// (`...app:80/`) or a longer name (`...app.evil/`) is not this prefix.
pub const UPLOADS: &str = "https://uploads.linear.app/";

/// The most bytes of a picture or a PDF fetched into memory, to be drawn.
pub const BYTES_MOST: u64 = 8 * 1024 * 1024;

/// The most bytes of a file sent to Linear, or saved from it.
pub const FILE_MOST: u64 = 100 * 1024 * 1024;

/// The most files `show --files` saves in one run, and the most bytes of them together: the rest are
/// told as refused, with the reason. A folder the pen put away holds no more (`work/handoff.rs`).
pub const SAVED_FILES_MOST: usize = 100;
pub const SAVED_BYTES_MOST: u64 = 1024 * 1024 * 1024;

/// `FILE_MOST` in MiB, for the words that tell it (a size is said in MiB where it is a power of two).
pub const FILE_MOST_MIB: u64 = FILE_MOST / (1024 * 1024);

/// The most pixels a picture is wide or tall, to be drawn.
pub const SIDE_MOST: u32 = 4096;

/// The most files one comment, one new issue or one addition to a description carries.
pub const SEND_MOST: usize = 10;

/// The longest address that is taken.
const URL_MOST: usize = 2048;

/// The longest name a picture's Markdown line carries, in characters.
const NAME_MOST: usize = 100;

/// The longest description that is written, in characters (Linear's own limit is above it).
pub const DESCRIPTION_MOST: usize = 200_000;

/// Whether `src` is an address of `https://uploads.linear.app/`: the host as it stands, the path of
/// printable characters of a URL and nothing else (no space, no backslash, no quote, no control
/// character, no non-ASCII).
#[must_use]
pub fn is_upload(src: &str) -> bool {
    let Some(rest) = src.strip_prefix(UPLOADS) else {
        return false;
    };
    src.len() <= URL_MOST
        && !rest.is_empty()
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=%".contains(&byte))
}

/// The two formats that are drawn and sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
}

impl Format {
    /// The format of `bytes`, by their first bytes and not by a name.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else {
            None
        }
    }
}

/// A picture, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// The size of the picture in `bytes` of `format`, as it is shown (a photograph turned by its EXIF
/// orientation is as wide as it is shown); none where the header says no size.
#[must_use]
pub fn size_of(bytes: &[u8], format: Format) -> Option<Size> {
    let size = match format {
        Format::Png => png(bytes),
        Format::Jpeg => jpeg(bytes),
    }?;
    (size.width > 0 && size.height > 0).then_some(size)
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let four: [u8; 4] = bytes.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(four))
}

fn be16(bytes: &[u8], at: usize) -> Option<u32> {
    let two: [u8; 2] = bytes.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(u32::from(u16::from_be_bytes(two)))
}

/// The `IHDR` chunk comes first, after the signature: its length, its name, the width and the height.
fn png(bytes: &[u8]) -> Option<Size> {
    if bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    Some(Size {
        width: be32(bytes, 16)?,
        height: be32(bytes, 20)?,
    })
}

/// The first frame header (`SOF0` to `SOF15`, but for the three that are no frames) has the height
/// and the width. The segments before it are skipped by their lengths; the EXIF block among them
/// says how the photograph was held.
fn jpeg(bytes: &[u8]) -> Option<Size> {
    let mut at = 2;
    let mut turned = false;
    loop {
        if *bytes.get(at)? != 0xFF {
            return None;
        }
        while *bytes.get(at)? == 0xFF {
            at += 1;
        }
        let marker = *bytes.get(at)?;
        at += 1;
        match marker {
            0x00 | 0x01 | 0xD0..=0xD8 => continue,
            0xD9 | 0xDA => return None,
            _ => {}
        }
        let length = usize::try_from(be16(bytes, at)?).ok()?;
        if length < 2 {
            return None;
        }
        let body = bytes.get(at + 2..at.checked_add(length)?)?;
        match marker {
            0xE1 if body.starts_with(b"Exif\0\0") => {
                turned = body.get(6..).is_some_and(turns_a_quarter);
            }
            0xC4 | 0xC8 | 0xCC => {}
            0xC0..=0xCF => {
                let (height, width) = (be16(body, 1)?, be16(body, 3)?);
                return Some(if turned {
                    Size {
                        width: height,
                        height: width,
                    }
                } else {
                    Size { width, height }
                });
            }
            _ => {}
        }
        at += length;
    }
}

/// Whether the EXIF block `tiff` (from its TIFF header on) says the photograph is to be turned a
/// quarter, or mirrored along a diagonal: orientations 5 to 8, which swap the width and the height.
fn turns_a_quarter(tiff: &[u8]) -> bool {
    let little = match tiff.get(..4) {
        Some(b"II*\0") => true,
        Some(b"MM\0*") => false,
        _ => return false,
    };
    let short = |at: usize| -> Option<u16> {
        let two: [u8; 2] = tiff.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(if little {
            u16::from_le_bytes(two)
        } else {
            u16::from_be_bytes(two)
        })
    };
    let long = |at: usize| -> Option<u32> {
        let four: [u8; 4] = tiff.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(if little {
            u32::from_le_bytes(four)
        } else {
            u32::from_be_bytes(four)
        })
    };
    let orientation = || -> Option<u16> {
        let directory = usize::try_from(long(4)?).ok()?;
        let entries = usize::from(short(directory)?);
        (0..entries).find_map(|n| {
            let entry = directory.checked_add(2)?.checked_add(n.checked_mul(12)?)?;
            if short(entry)? != 0x0112 {
                return None;
            }
            short(entry.checked_add(8)?)
        })
    };
    matches!(orientation(), Some(5..=8))
}

/// The name a file is given in its Markdown line: `[`, `]`, `(`, `)`, `` ` `` and `\` are left out
/// (they would end the line's own text or the address), and so is every control character; the white
/// space at its ends goes, it is at most 100 characters, and a name of nothing is `file`.
#[must_use]
pub fn clean_name(name: &str) -> String {
    let kept = cleaned(name);
    if kept.is_empty() {
        "file".to_owned()
    } else {
        kept
    }
}

/// `clean_name` with no name for nothing: empty where there is none.
fn cleaned(name: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| !c.is_control() && !"[]()`\\".contains(*c))
        .collect();
    let kept: String = kept.trim().chars().take(NAME_MOST).collect();
    kept.trim().to_owned()
}

/// A file named in Markdown, and where it stands in the text (byte offsets): a picture, `![alt](src)`,
/// or a link to one of Linear's files, `[name](src)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub start: usize,
    pub end: usize,
    /// The words between the brackets: the picture's alt, or the link's text.
    pub name: String,
    pub src: String,
    /// It is a picture (`![]()`), not a link.
    pub image: bool,
}

/// The files `text` names, in the order they stand: a picture, `![alt](src)` with any address, and a
/// link, `[name](src)`, to an address of Linear's files (any other link is words). The words in the
/// brackets are anything on one line that has no `]`, and `src` is what follows the `(` up to a space
/// or the `)` (a title after it is allowed and left out). Nothing is fetched or followed; this only
/// reads.
#[must_use]
pub fn find(text: &str) -> Vec<Named> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find('[') {
        let open = from + at;
        from = open + 1;
        let image = open > 0 && text.as_bytes()[open - 1] == b'!';
        let start = if image { open - 1 } else { open };
        let rest = &text[open + 1..];
        let Some(close) = rest.find(']') else {
            break;
        };
        let name = &rest[..close];
        if name.contains('\n') {
            continue;
        }
        let Some(after) = rest[close + 1..].strip_prefix('(') else {
            continue;
        };
        let src_len = after
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(after.len());
        if src_len == 0 {
            continue;
        }
        let src = &after[..src_len];
        if !image && !is_upload(src) {
            continue;
        }
        let tail = &after[src_len..];
        // The `)` that ends it: at once, or after a title on the same line.
        let Some(shut) = tail.split('\n').next().and_then(|line| line.find(')')) else {
            continue;
        };
        let end = open + 1 + close + 1 + 1 + src_len + shut + 1;
        found.push(Named {
            start,
            end,
            name: name.to_owned(),
            src: src.to_owned(),
            image,
        });
        from = end;
    }
    found
}

/// What a file is by its first bytes: what Linear and its storage are told it is, whether it is a
/// picture (put in Markdown as one: PNG, JPEG, GIF or WebP), and the extension that goes with it, where
/// it is a kind that has one here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sniffed {
    pub content_type: &'static str,
    pub image: bool,
    pub extension: Option<&'static str>,
}

/// What `head` (the first bytes of a file) says it is: PNG, JPEG, GIF, WebP, PDF, and for anything else
/// `application/octet-stream`, which says nothing and is not a kind.
#[must_use]
pub fn sniff(head: &[u8]) -> Sniffed {
    let picture = |content_type, extension| Sniffed {
        content_type,
        image: true,
        extension: Some(extension),
    };
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        picture("image/png", "png")
    } else if head.starts_with(&[0xff, 0xd8, 0xff]) {
        picture("image/jpeg", "jpg")
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        picture("image/gif", "gif")
    } else if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        picture("image/webp", "webp")
    } else if head.starts_with(b"%PDF-") {
        Sniffed {
            content_type: "application/pdf",
            image: false,
            extension: Some("pdf"),
        }
    } else {
        Sniffed {
            content_type: "application/octet-stream",
            image: false,
            extension: None,
        }
    }
}

/// The extension of a kind of file that `bytes` is, as a file is named for it: PNG, JPEG, GIF, WebP or
/// PDF; none for any other.
#[must_use]
pub fn kind_of(bytes: &[u8]) -> Option<&'static str> {
    sniff(bytes).extension
}

/// The name an address ends in (its last part before a `?` or `#`), for a file that has no name of
/// its own: cleaned, and empty where there is none.
#[must_use]
pub fn name_in(src: &str) -> String {
    let path = src.split(['?', '#']).next().unwrap_or("");
    cleaned(path.rsplit('/').next().unwrap_or(""))
}

/// The extension a name ends in: one to eight letters or digits after the last dot (that is not the
/// first character), in lower case.
#[must_use]
pub fn name_ext(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    let fine = !stem.is_empty()
        && (1..=8).contains(&ext.len())
        && ext.bytes().all(|byte| byte.is_ascii_alphanumeric());
    fine.then(|| ext.to_ascii_lowercase())
}

/// A name as part of a file name: letters, digits, dots, dashes and underscores (anything else is a
/// dash), no extension at its end (`name_ext` is what it was), at most 60 characters, and `file` for
/// nothing.
#[must_use]
pub fn file_stem(name: &str) -> String {
    let mut stem = String::new();
    for c in name.chars() {
        let c = if c.is_ascii_alphanumeric() || "._-".contains(c) {
            c
        } else {
            '-'
        };
        if c == '-' && stem.ends_with('-') {
            continue;
        }
        stem.push(c);
    }
    if let Some(ext) = name_ext(&stem) {
        stem.truncate(stem.len() - ext.len() - 1);
    }
    let stem: String = stem
        .trim_matches(|c| c == '-' || c == '.' || c == '_')
        .chars()
        .take(60)
        .collect();
    if stem.is_empty() {
        "file".to_owned()
    } else {
        stem
    }
}

/// The Markdown line of a file sent: its name and the address Linear gave it, as a picture (`![]()`)
/// where it is one and as a link (`[]()`) where it is not.
#[must_use]
pub fn line(name: &str, asset: &str, image: bool) -> String {
    format!(
        "{}[{}]({asset})",
        if image { "!" } else { "" },
        clean_name(name)
    )
}

/// The words of a comment or of a new issue with the picture lines under them, a blank line between
/// (where there are words), one picture to a line.
#[must_use]
pub fn with_lines(words: &str, lines: &[String]) -> String {
    let pictures = lines.join("\n");
    if words.trim().is_empty() {
        pictures
    } else if pictures.is_empty() {
        words.to_owned()
    } else {
        format!("{}\n\n{pictures}", words.trim_end())
    }
}

/// The longest path after `https://uploads.linear.app/` that the length checks allow a file's line:
/// the addresses Linear gives are far shorter, and a check made before the file is sent cannot know
/// the address.
pub const ASSET_PATH_MOST: usize = 200;

/// The most characters `words` and the lines of files with these `names` can come to once `with_lines`
/// puts them together, worked out before the files are sent: each line is `![name](https://uploads.linear.app/`,
/// a path of at most `ASSET_PATH_MOST` and `)`, and the lines and the blank line before them are
/// counted in full. A text that is within its limit by this is within it whatever the addresses are.
#[must_use]
pub fn with_lines_most(words: &str, names: &[&str]) -> usize {
    let lines: usize = names
        .iter()
        .map(|name| {
            format!("![{}](https://uploads.linear.app/)", clean_name(name))
                .chars()
                .count()
                + ASSET_PATH_MOST
                + 1
        })
        .sum();
    if names.is_empty() {
        words.chars().count()
    } else {
        words.trim_end().chars().count() + 2 + lines
    }
}

/// A description as it is with the picture lines at its end: it is kept as it stands, whatever it
/// holds, and the lines come after a blank line. A description that is nothing is just the lines.
#[must_use]
pub fn description_with(old: &str, lines: &[String]) -> String {
    let pictures = lines.join("\n");
    if old.trim().is_empty() {
        return pictures;
    }
    let gap = if old.ends_with("\n\n") {
        ""
    } else if old.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{old}{gap}{pictures}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_of(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    fn jpeg_of(width: u16, height: u16, exif: Option<u8>) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0];
        if let Some(orientation) = exif {
            // APP1 "Exif\0\0", a little-endian TIFF with one entry: the orientation.
            let mut body = b"Exif\0\0II*\0".to_vec();
            body.extend_from_slice(&8u32.to_le_bytes());
            body.extend_from_slice(&1u16.to_le_bytes());
            body.extend_from_slice(&0x0112u16.to_le_bytes());
            body.extend_from_slice(&3u16.to_le_bytes());
            body.extend_from_slice(&1u32.to_le_bytes());
            body.extend_from_slice(&u16::from(orientation).to_le_bytes());
            body.extend_from_slice(&[0, 0]);
            bytes.extend_from_slice(&[0xFF, 0xE1]);
            bytes.extend_from_slice(&u16::try_from(body.len() + 2).unwrap().to_be_bytes());
            bytes.extend_from_slice(&body);
        }
        bytes.extend_from_slice(&[0xFF, 0xC0, 0, 11, 8]);
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&[1, 1, 0x11, 0]);
        bytes
    }

    #[test]
    fn only_an_address_of_the_uploads_host_is_one() {
        assert!(is_upload("https://uploads.linear.app/ws/a-b/c.png"));
        assert!(is_upload(
            "https://uploads.linear.app/ws/a/b?signature=a%20b&x=1"
        ));
        let not = [
            "",
            "https://uploads.linear.app",
            "https://uploads.linear.app/",
            "http://uploads.linear.app/a.png",
            "https://uploads.linear.app.evil.com/a.png",
            "https://uploads.linear.app@evil.com/a.png",
            "https://uploads.linear.app:443/a.png",
            "https://user:pass@uploads.linear.app/a.png",
            "https://evil.com/https://uploads.linear.app/a.png",
            "https://api.linear.app/graphql",
            "https://Uploads.linear.app/a.png",
            "//uploads.linear.app/a.png",
            "file:///etc/passwd",
            "https://uploads.linear.app/a b.png",
            "https://uploads.linear.app/a\\@evil",
            "https://uploads.linear.app/a\".png",
            "https://uploads.linear.app/a\n.png",
            "https://uploads.linear.app/\u{e7}.png",
            "https://uploads.linear.app/a<b>",
        ];
        for src in not {
            assert!(!is_upload(src), "{src:?}");
        }
        let long = format!("https://uploads.linear.app/{}", "a".repeat(URL_MOST));
        assert!(!is_upload(&long));
    }

    #[test]
    fn a_format_is_known_by_its_bytes_and_not_by_a_name() {
        assert_eq!(Format::of(&png_of(2, 3)), Some(Format::Png));
        assert_eq!(Format::of(&jpeg_of(2, 3, None)), Some(Format::Jpeg));
        assert_eq!(
            Format::of(b"GIF89a"),
            None,
            "(`Format` is the two that are measured)"
        );
        assert_eq!(Format::of(b"<svg xmlns=\"x\"/>"), None);
        assert_eq!(Format::of(b""), None);
    }

    #[test]
    fn the_size_is_read_from_the_header() {
        assert_eq!(
            size_of(&png_of(640, 480), Format::Png),
            Some(Size {
                width: 640,
                height: 480
            })
        );
        assert_eq!(
            size_of(&jpeg_of(800, 600, None), Format::Jpeg),
            Some(Size {
                width: 800,
                height: 600
            })
        );
        // A photograph held on its side is as wide as it is shown.
        assert_eq!(
            size_of(&jpeg_of(800, 600, Some(6)), Format::Jpeg),
            Some(Size {
                width: 600,
                height: 800
            })
        );
        assert_eq!(
            size_of(&jpeg_of(800, 600, Some(1)), Format::Jpeg),
            Some(Size {
                width: 800,
                height: 600
            })
        );
        // A header cut short, or one that says no size, is none.
        assert_eq!(size_of(&png_of(0, 5), Format::Png), None);
        assert_eq!(size_of(&png_of(5, 5)[..20], Format::Png), None);
        assert_eq!(size_of(&[0xFF, 0xD8, 0xFF], Format::Jpeg), None);
    }

    #[test]
    fn a_name_loses_what_would_end_the_line_and_is_never_empty() {
        assert_eq!(clean_name("screen shot.png"), "screen shot.png");
        assert_eq!(clean_name("a](http://evil)[b"), "ahttp://evilb");
        assert_eq!(clean_name("x`y\\z"), "xyz");
        assert_eq!(clean_name("a\nb\tc\u{7}"), "abc");
        assert_eq!(clean_name("  "), "file");
        assert_eq!(clean_name("()[]"), "file");
        assert_eq!(clean_name(""), "file");
        assert_eq!(clean_name(&"a".repeat(300)).chars().count(), NAME_MOST);
        // A picture is put as a picture and any other file as a link.
        assert_eq!(
            line("a (1).png", "https://uploads.linear.app/s/1", true),
            "![a 1.png](https://uploads.linear.app/s/1)"
        );
        assert_eq!(
            line("build log.txt", "https://uploads.linear.app/s/2", false),
            "[build log.txt](https://uploads.linear.app/s/2)"
        );
    }

    #[test]
    fn what_words_and_files_can_come_to_is_worked_out_before_the_addresses_are_known() {
        let line_of = |name: &str, path: &str| {
            line(name, &format!("https://uploads.linear.app/{path}"), true)
        };
        // The longest path the check allows, and a usual one: the text made is within what was counted.
        for path in [
            "a".repeat(ASSET_PATH_MOST),
            "ws/9a1f0c/checkout-error.png".to_owned(),
        ] {
            let lines = [line_of("Checkout error", &path), line_of("two.png", &path)];
            for words in ["", "Look", "Look\n\n", "  \n"] {
                let made = with_lines(words, &lines).chars().count();
                let most = with_lines_most(words, &["Checkout error", "two.png"]);
                assert!(made <= most, "{words:?}: {made} > {most}");
            }
        }
        // No file: the words as they are. A name is counted as it is cleaned (at most a hundred).
        assert_eq!(with_lines_most("four", &[]), 4);
        let long = "n".repeat(500);
        assert!(with_lines_most("", &[&long]) < 500);
    }

    #[test]
    fn the_lines_come_after_the_words_and_a_blank_line() {
        let lines = vec![
            "![a](https://uploads.linear.app/1)".to_owned(),
            "![b](https://uploads.linear.app/2)".to_owned(),
        ];
        assert_eq!(
            with_lines("Look\n", &lines),
            "Look\n\n![a](https://uploads.linear.app/1)\n![b](https://uploads.linear.app/2)"
        );
        // No words: the pictures alone.
        assert_eq!(
            with_lines(" \n", &lines),
            "![a](https://uploads.linear.app/1)\n![b](https://uploads.linear.app/2)"
        );
        assert_eq!(with_lines("Look", &[]), "Look");
    }

    #[test]
    fn a_description_is_kept_as_it_is_and_the_lines_are_added_at_its_end() {
        let lines = vec!["![a](https://uploads.linear.app/1)".to_owned()];
        for old in [
            "Text",
            "Text\n",
            "Text\n\n",
            "- a\n- b  ",
            "# T\n\n  indented",
        ] {
            let new = description_with(old, &lines);
            assert!(new.starts_with(old), "{old:?}");
            assert!(new.ends_with(&lines[0]), "{old:?}");
            assert!(new[old.len()..].starts_with('\n') || old.ends_with('\n'));
            // One blank line between, whichever way the text ended.
            let between = &new[old.trim_end().len()..new.len() - lines[0].len()];
            assert!(between.contains("\n\n"), "{old:?} {between:?}");
        }
        assert_eq!(description_with("", &lines), lines[0]);
        assert_eq!(description_with(" \n ", &lines), lines[0]);
    }

    #[test]
    fn files_named_in_markdown_are_found_in_order_with_their_places() {
        let text = "a ![one](https://uploads.linear.app/a.png) b\n![](https://x/y.jpg \"title\")\n![no](\n![bad\nalt](https://x/z.png) ![t](https://x/t.gif)\n[log](https://uploads.linear.app/w/b.log) and [a site](https://example.com/x) and [near](https://uploads.linear.app.evil/x)";
        let found = find(text);
        let srcs: Vec<&str> = found.iter().map(|n| n.src.as_str()).collect();
        assert_eq!(
            srcs,
            [
                "https://uploads.linear.app/a.png",
                "https://x/y.jpg",
                "https://x/t.gif",
                "https://uploads.linear.app/w/b.log"
            ]
        );
        assert_eq!(found[0].name, "one");
        assert!(found[0].image && !found[3].image);
        assert_eq!(
            &text[found[0].start..found[0].end],
            "![one](https://uploads.linear.app/a.png)"
        );
        assert_eq!(
            &text[found[1].start..found[1].end],
            "![](https://x/y.jpg \"title\")"
        );
        assert_eq!(found[1].name, "");
        // A link is a file only where it is one of Linear's; the others are words.
        assert_eq!(found[3].name, "log");
        assert_eq!(
            &text[found[3].start..found[3].end],
            "[log](https://uploads.linear.app/w/b.log)"
        );
        // The `!` belongs to a picture and not to the link in it.
        assert_eq!(find("![a](https://uploads.linear.app/p)").len(), 1);
    }

    #[test]
    fn what_a_file_is_goes_by_its_bytes_and_a_name_is_made_safe() {
        assert_eq!(kind_of(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(kind_of(&[0xff, 0xd8, 0xff, 0xe0]), Some("jpg"));
        assert_eq!(kind_of(b"GIF89a.."), Some("gif"));
        assert_eq!(kind_of(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(kind_of(b"%PDF-1.7\n"), Some("pdf"));
        assert_eq!(kind_of(b"<html>"), None);
        // The pictures are put in Markdown as pictures; a PDF and the rest are links.
        assert!(sniff(b"GIF89a..").image && sniff(b"\xff\xd8\xff").image);
        assert!(!sniff(b"%PDF-1.4").image && !sniff(b"PK\x03\x04").image);
        assert_eq!(sniff(b"%PDF-1.4").content_type, "application/pdf");
        assert_eq!(
            sniff(b"PK\x03\x04").content_type,
            "application/octet-stream"
        );
        assert_eq!(sniff(b"").content_type, "application/octet-stream");
        assert_eq!(
            name_in("https://uploads.linear.app/ws/9a/shot.png?x=1#y"),
            "shot.png"
        );
        assert_eq!(name_in("https://uploads.linear.app/"), "");
        assert_eq!(name_ext("Build.LOG").as_deref(), Some("log"));
        assert_eq!(name_ext(".hidden"), None);
        assert_eq!(name_ext("archive.tar.gz").as_deref(), Some("gz"));
        assert_eq!(name_ext("noext"), None);
        assert_eq!(name_ext("a.toolongextension"), None);
        assert_eq!(file_stem("Checkout error (1).PNG"), "Checkout-error-1");
        assert_eq!(file_stem("build.log"), "build");
        assert_eq!(file_stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(file_stem("...."), "file");
    }
}
