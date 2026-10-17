//! Minimal bounded BitTorrent metainfo reader used only to derive a magnet URI.
//! Mokawa never opens peer connections or runs a BitTorrent engine.
use crate::error::{Error, Result};
use sha1::{Digest, Sha1};

const MAX_TORRENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 64;

pub fn magnet_from_torrent(bytes: &[u8]) -> Result<String> {
    if bytes.is_empty() || bytes.len() > MAX_TORRENT_BYTES || bytes[0] != b'd' {
        return Err(Error::Invalid("种子文件无效或超过 8 MB"));
    }
    let mut cursor = 1;
    let mut info = None;
    while cursor < bytes.len() && bytes[cursor] != b'e' {
        let key = parse_bytes(bytes, &mut cursor)?;
        if key == b"info" {
            if info.is_some() {
                return Err(Error::Invalid("种子文件包含重复的 info 字段"));
            }
            let start = cursor;
            parse_value(bytes, &mut cursor, 1)?;
            if bytes.get(start) != Some(&b'd') {
                return Err(Error::Invalid("种子文件的 info 字段无效"));
            }
            info = Some(&bytes[start..cursor]);
        } else {
            parse_value(bytes, &mut cursor, 1)?;
        }
    }
    if bytes.get(cursor) != Some(&b'e') || cursor + 1 != bytes.len() {
        return Err(Error::Invalid("种子文件结构不完整"));
    }
    let info = info.ok_or(Error::Invalid("种子文件缺少 info 字段"))?;
    let digest = Sha1::digest(info);
    let hash = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("magnet:?xt=urn:btih:{hash}"))
}

fn parse_value(input: &[u8], cursor: &mut usize, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(Error::Invalid("种子文件嵌套层数超出限制"));
    }
    match input.get(*cursor).copied() {
        Some(b'i') => {
            *cursor += 1;
            let start = *cursor;
            while input.get(*cursor).is_some_and(|byte| *byte != b'e') {
                *cursor += 1;
            }
            if *cursor == start || input.get(*cursor) != Some(&b'e') {
                return Err(Error::Invalid("种子文件整数编码无效"));
            }
            let number = std::str::from_utf8(&input[start..*cursor])
                .map_err(|_| Error::Invalid("种子文件整数编码无效"))?;
            let digits = number.strip_prefix('-').unwrap_or(number);
            if digits.is_empty()
                || number == "-0"
                || number.starts_with('0') && number.len() > 1
                || number.starts_with("-0") && number.len() > 2
                || digits.bytes().any(|byte| !byte.is_ascii_digit())
            {
                return Err(Error::Invalid("种子文件整数编码无效"));
            }
            *cursor += 1;
            Ok(())
        }
        Some(b'l') => {
            *cursor += 1;
            while input.get(*cursor) != Some(&b'e') {
                parse_value(input, cursor, depth + 1)?;
            }
            *cursor += 1;
            Ok(())
        }
        Some(b'd') => {
            *cursor += 1;
            while input.get(*cursor) != Some(&b'e') {
                parse_bytes(input, cursor)?;
                parse_value(input, cursor, depth + 1)?;
            }
            *cursor += 1;
            Ok(())
        }
        Some(b'0'..=b'9') => {
            parse_bytes(input, cursor)?;
            Ok(())
        }
        _ => Err(Error::Invalid("种子文件 bencode 结构无效")),
    }
}

fn parse_bytes<'a>(input: &'a [u8], cursor: &mut usize) -> Result<&'a [u8]> {
    let start = *cursor;
    while input.get(*cursor).is_some_and(u8::is_ascii_digit) {
        *cursor += 1;
    }
    if *cursor == start || input.get(*cursor) != Some(&b':') {
        return Err(Error::Invalid("种子文件字符串长度无效"));
    }
    let digits = &input[start..*cursor];
    if digits.len() > 1 && digits[0] == b'0' {
        return Err(Error::Invalid("种子文件字符串长度无效"));
    }
    let length = std::str::from_utf8(digits)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or(Error::Invalid("种子文件字符串长度无效"))?;
    *cursor += 1;
    let end = (*cursor)
        .checked_add(length)
        .filter(|end| *end <= input.len())
        .ok_or(Error::Invalid("种子文件被截断"))?;
    let value = &input[*cursor..end];
    *cursor = end;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_info_hash_from_a_bounded_valid_torrent() {
        let torrent = b"d4:infod4:name4:test6:pieces0:ee";
        assert_eq!(
            magnet_from_torrent(torrent).unwrap(),
            "magnet:?xt=urn:btih:0bff18183af51b68eb41eaacef1d09f11e1fd14c"
        );
    }

    #[test]
    fn rejects_malformed_deep_and_oversized_metainfo() {
        assert!(magnet_from_torrent(b"not torrent").is_err());
        assert!(magnet_from_torrent(b"d4:info1:xe").is_err());
        assert!(magnet_from_torrent(&vec![b'a'; MAX_TORRENT_BYTES + 1]).is_err());
    }
}
