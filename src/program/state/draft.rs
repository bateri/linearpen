//! The identity of a draft of an issue: a UUID version 4 that the program makes, which goes to
//! Linear as the id of the issue. The id is made before the issue is sent, so that an answer that
//! never came can be answered by asking for the issue under it.

use std::fs::File;
use std::io::Read;

/// `bytes` as a UUID version 4: the version and the variant are set, the rest is what was given.
#[must_use]
pub fn uuid_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Whether `text` has the shape of a UUID: 8-4-4-4-12 hexadecimal digits.
#[must_use]
pub fn is_uuid(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(part, len)| part.len() == len && part.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// A new draft id, from the random bytes of the machine.
pub fn new_id() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut bytes))
        .map_err(|why| format!("no id could be made for the issue: {why}"))?;
    Ok(uuid_v4(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_and_the_variant_are_set_whatever_the_bytes_are() {
        assert_eq!(uuid_v4([0x00; 16]), "00000000-0000-4000-8000-000000000000");
        assert_eq!(uuid_v4([0xff; 16]), "ffffffff-ffff-4fff-bfff-ffffffffffff");
        let mut bytes = [0u8; 16];
        for (n, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::try_from(n * 17).unwrap();
        }
        assert_eq!(uuid_v4(bytes), "00112233-4455-4677-8899-aabbccddeeff");
    }

    #[test]
    fn a_uuid_is_told_from_what_is_not_one() {
        assert!(is_uuid("11111111-2222-4333-8444-555555555555"));
        assert!(is_uuid("ABCDEF01-2222-4333-8444-555555555555"));
        assert!(!is_uuid(""));
        assert!(!is_uuid("issue-412"));
        assert!(!is_uuid("11111111-2222-4333-8444-55555555555"));
        assert!(!is_uuid("11111111-2222-4333-8444-55555555555g"));
        assert!(!is_uuid("111111112222433384445555555555555555"));
    }

    #[test]
    fn a_new_id_is_a_uuid_and_not_the_same_twice() {
        let (a, b) = (new_id().unwrap(), new_id().unwrap());
        assert!(is_uuid(&a), "{a}");
        assert_ne!(a, b);
        assert_eq!(a.as_bytes()[14], b'4');
    }
}
