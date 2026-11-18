//! Pure-Rust in-memory `.tar.gz` unpacker for Typst `@preview` package archives.
//!
//! Avoids OS filesystem and `libc`/`xattr`/`filetime` dependencies so `@preview/name:version`
//! archives fetched in Web Workers (`wasm32-unknown-unknown`) unpack directly into virtual `FileId`s.

use std::collections::HashMap;
use std::io::Read;

use flate2::read::GzDecoder;
use typst::foundations::Bytes;
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};

/// Decompress a `.tar.gz` Typst package archive and insert all regular files into `dst`
/// keyed by `RootedPath::new(VirtualRoot::Package(spec.clone()), vpath).intern()`.
///
/// Returns the number of unpacked files.
pub fn unpack_package_tar_gz(
    spec: &PackageSpec,
    tar_gz_bytes: &[u8],
    dst: &mut HashMap<FileId, Bytes>,
) -> Result<usize, String> {
    let mut decoder = GzDecoder::new(tar_gz_bytes);
    let mut tar_bytes = Vec::new();
    decoder
        .read_to_end(&mut tar_bytes)
        .map_err(|e| format!("failed to gunzip package {spec}: {e}"))?;

    unpack_tar_bytes(spec, &tar_bytes, dst)
}

/// Parse an uncompressed POSIX UStar / PAX / GNU `.tar` byte slice into `dst`.
pub fn unpack_tar_bytes(
    spec: &PackageSpec,
    tar_bytes: &[u8],
    dst: &mut HashMap<FileId, Bytes>,
) -> Result<usize, String> {
    let mut offset = 0usize;
    let mut count = 0usize;
    let mut pax_path_override: Option<String> = None;
    let mut gnu_long_name: Option<String> = None;

    while offset + 512 <= tar_bytes.len() {
        let header = &tar_bytes[offset..offset + 512];
        offset += 512;

        // Two consecutive all-zero 512-byte blocks mark EOF.
        if header.iter().all(|&b| b == 0) {
            break;
        }

        let size = parse_tar_size(&header[124..136])
            .ok_or_else(|| format!("invalid octal/base-256 size in tar header for {spec}"))?;
        let data_end = offset
            .checked_add(size)
            .filter(|&end| end <= tar_bytes.len())
            .ok_or_else(|| format!("truncated tar entry in package {spec}"))?;
        let payload = &tar_bytes[offset..data_end];

        // Advance `offset` padded up to next 512-byte boundary.
        let padded = (size + 511) & !511;
        offset = offset.saturating_add(padded).min(tar_bytes.len());

        let typeflag = header[156];
        match typeflag {
            // GNU LongLink ('L'): payload contains NUL-terminated UTF-8 path for the next entry.
            b'L' => {
                gnu_long_name = Some(trim_nul_utf8(payload));
                continue;
            }
            // PAX extended header ('x'): parse `path=...` record for the next entry.
            b'x' => {
                if let Some(p) = parse_pax_path(payload) {
                    pax_path_override = Some(p);
                }
                continue;
            }
            // Global PAX header ('g') or directory ('5') or symlink/other: clear overrides if non-file.
            b'g' => continue,
            0 | b'0' | b'7' => {}
            _ => {
                pax_path_override = None;
                gnu_long_name = None;
                continue;
            }
        }

        let raw_path = pax_path_override
            .take()
            .or_else(|| gnu_long_name.take())
            .unwrap_or_else(|| extract_ustar_path(header));

        let normalized = normalize_archive_rel_path(&raw_path);
        if normalized.is_empty() {
            continue;
        }

        if let Ok(vpath) = VirtualPath::new(&normalized) {
            let file_id = RootedPath::new(VirtualRoot::Package(spec.clone()), vpath).intern();
            dst.insert(file_id, Bytes::new(payload.to_vec()));
            count += 1;
        }
    }

    Ok(count)
}

fn parse_tar_size(field: &[u8]) -> Option<usize> {
    if field.is_empty() {
        return Some(0);
    }
    // Base-256 binary extension (high bit set on first byte).
    if field[0] & 0x80 != 0 {
        let mut val: usize = 0;
        for &b in &field[1..] {
            val = val.checked_shl(8)?.checked_add(b as usize)?;
        }
        return Some(val);
    }
    let s = trim_nul_utf8(field);
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Some(0);
    }
    usize::from_str_radix(trimmed, 8).ok()
}

fn extract_ustar_path(header: &[u8]) -> String {
    let name = trim_nul_utf8(&header[0..100]);
    let magic = &header[257..263];
    if magic.starts_with(b"ustar") {
        let prefix = trim_nul_utf8(&header[345..500]);
        if !prefix.is_empty() {
            return format!("{prefix}/{name}");
        }
    }
    name
}

fn trim_nul_utf8(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn parse_pax_path(payload: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(payload).ok()?;
    let mut rest = text;
    while !rest.is_empty() {
        let space_idx = rest.find(' ')?;
        let rec_len: usize = rest[..space_idx].parse().ok()?;
        if rec_len == 0 || rec_len > rest.len() {
            break;
        }
        let record = &rest[space_idx + 1..rec_len];
        rest = &rest[rec_len..];
        let record = record.strip_suffix('\n').unwrap_or(record);
        if let Some(val) = record.strip_prefix("path=") {
            return Some(val.to_string());
        }
    }
    None
}

fn normalize_archive_rel_path(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for seg in replaced.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;
    use typst::syntax::package::PackageVersion;

    fn make_tar_entry(path: &str, content: &[u8]) -> Vec<u8> {
        let mut block = vec![0u8; 512];
        block[..path.len()].copy_from_slice(path.as_bytes());
        let size_octal = format!("{:011o}\0", content.len());
        block[124..136].copy_from_slice(size_octal.as_bytes());
        block[156] = b'0';
        block[257..263].copy_from_slice(b"ustar\0");
        // Compute header checksum (treating checksum bytes 148..156 as spaces).
        block[148..156].fill(b' ');
        let sum: u32 = block.iter().map(|&b| b as u32).sum();
        let chk = format!("{:06o}\0 ", sum);
        block[148..156].copy_from_slice(chk.as_bytes());

        block.extend_from_slice(content);
        let rem = content.len() % 512;
        if rem != 0 {
            block.resize(block.len() + (512 - rem), 0);
        }
        block
    }

    #[test]
    fn unpacks_in_memory_package_tar_gz() {
        let mut tar = Vec::new();
        tar.extend_from_slice(&make_tar_entry(
            "./typst.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nentrypoint = \"src/lib.typ\"\n",
        ));
        tar.extend_from_slice(&make_tar_entry("src/lib.typ", b"#let hello() = [Hi]"));
        tar.resize(tar.len() + 1024, 0);

        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(&tar).unwrap();
        let tar_gz = gz.finish().unwrap();

        let spec = PackageSpec {
            namespace: "preview".into(),
            name: "demo".into(),
            version: PackageVersion {
                major: 0,
                minor: 1,
                patch: 0,
            },
        };
        let mut files = HashMap::new();
        let n = unpack_package_tar_gz(&spec, &tar_gz, &mut files).unwrap();
        assert_eq!(n, 2);

        let ep_id = RootedPath::new(
            VirtualRoot::Package(spec),
            VirtualPath::new("src/lib.typ").unwrap(),
        )
        .intern();
        assert_eq!(files.get(&ep_id).unwrap().as_slice(), b"#let hello() = [Hi]");
    }
}
