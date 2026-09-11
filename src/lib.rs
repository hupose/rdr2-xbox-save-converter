use aes::Aes256;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit, generic_array::GenericArray};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;
use zip::ZipArchive;

const PC_HEADER_SIZE: usize = 0x110;
const XBOX_HEADER_SIZE: usize = 0x118;
const XBOX_HEADER_VERSION: u32 = 5;
const RSAV_VERSION: u32 = 3;
const RSAV_HEADER_SIZE: usize = 0xB0;
const RSAV_TABLE_COUNT: usize = 9;
const MAX_ARCHIVE_UNCOMPRESSED: u64 = 256 * 1024 * 1024;
const CHKS_MARKER: &[u8; 8] = b"CHKS\0\0\0\x14";
const ENDS_BLOCK: &[u8; 16] = b"ENDS\0\0\0\0\0\0\0\0\0\0\0\0";
const RSAV_BLOCK_IDS: [u32; 9] = [
    0x41D6F794, 0x130F999E, 0x2016AB2F, 0xF0565D1D, 0x9466E30C, 0xF80C8931, 0x702A6025, 0x32D99C58,
    0x0C6ADEDD,
];

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("{0}")]
    Message(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("config error: {0}")]
    Config(#[from] toml::de::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, ConvertError>;

fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(ConvertError::Message(message.into()))
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyConfig {
    pub xbox_save_key: String,
    pub pc_save_key: String,
}

impl KeyConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|error| {
            ConvertError::Message(format!("cannot read config {}: {error}", path.display()))
        })?;
        Ok(toml::from_str(&text)?)
    }

    pub fn keys(&self) -> Result<([u8; 32], [u8; 32])> {
        Ok((
            parse_key("xbox_save_key", &self.xbox_save_key)?,
            parse_key("pc_save_key", &self.pc_save_key)?,
        ))
    }
}

fn parse_key(label: &str, value: &str) -> Result<[u8; 32]> {
    let compact: String = value
        .trim()
        .strip_prefix("0x")
        .unwrap_or(value.trim())
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    if compact.len() != 64 {
        return fail(format!(
            "{label} must contain exactly 64 hexadecimal characters"
        ));
    }
    let bytes = hex::decode(&compact)
        .map_err(|_| ConvertError::Message(format!("{label} is not valid hexadecimal")))?;
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

pub const CONFIG_TEMPLATE: &str = r#"# RDR2 Xbox Save Converter key configuration
# This application and its releases intentionally contain no game encryption keys.
#
# Xbox: find the `GTAV=` line in the 2013 Xbox 360 source post:
# https://community.wemod.com/t/gta-v-save-block-editor-0-0-3-x360-source/2899
#
# PC: find `PC_KEY` in this public source file:
# https://github.com/hzhreal/HTOS/blob/31fca60508251e591afe261b927d73c76a7b3503/data/crypto/rstar_crypt.py

xbox_save_key = ""
pc_save_key = ""
"#;

pub fn create_config_template(path: &Path) -> Result<()> {
    if path.exists() {
        return fail(format!(
            "refusing to overwrite existing config: {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, CONFIG_TEMPLATE)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct XboxSlot {
    pub xbox_name: String,
    pub pc_name: String,
    pub title: String,
    pub timestamp: u32,
    pub data: Vec<u8>,
    pub source_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsavValidation {
    pub size: usize,
    pub block_count: usize,
    pub checksum_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub source_slot: String,
    pub pc_file: String,
    pub title: String,
    pub timestamp: u32,
    pub source_sha256: String,
    pub pc_sha256: String,
    pub pc_size: usize,
    pub rsav_checksums: usize,
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| ConvertError::Message("unexpected end of data".into()))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")))
}

fn read_u32_be(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| ConvertError::Message("unexpected end of data".into()))?;
    Ok(u32::from_be_bytes(bytes.try_into().expect("four bytes")))
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn safe_zip_parts(name: &str) -> Result<Vec<&str>> {
    if name.contains('\0') || name.contains('\\') || name.starts_with('/') {
        return fail(format!("unsafe ZIP path: {name:?}"));
    }
    let parts: Vec<_> = name.split('/').filter(|part| !part.is_empty()).collect();
    if parts.iter().any(|part| *part == "." || *part == "..")
        || parts.first().is_some_and(|part| part.contains(':'))
    {
        return fail(format!("unsafe ZIP path: {name:?}"));
    }
    Ok(parts)
}

fn pc_name(xbox_name: &str) -> Result<String> {
    if let Some(slot) = xbox_name.strip_prefix("SRDR3") {
        if xbox_name.len() == 9 && slot.chars().all(|character| character.is_ascii_digit()) {
            return Ok(xbox_name.to_owned());
        }
    }
    match xbox_name {
        "SRDR300014" => Ok("SRDR30014".into()),
        "SRDR300015" => Ok("SRDR30015".into()),
        _ => fail(format!("cannot safely map Xbox slot name {xbox_name}")),
    }
}

fn decode_utf16le_field(raw: &[u8]) -> Result<String> {
    if raw.len() % 2 != 0 {
        return fail("UTF-16LE field has an odd byte length");
    }
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16(&units).map_err(|_| ConvertError::Message("invalid UTF-16LE title".into()))
}

pub fn load_xbox_slots(zip_path: &Path) -> Result<Vec<XboxSlot>> {
    let file = File::open(zip_path).map_err(|error| {
        ConvertError::Message(format!("cannot open {}: {error}", zip_path.display()))
    })?;
    let mut archive = ZipArchive::new(file)?;
    let mut total_size = 0_u64;
    let mut seen = HashSet::new();
    let mut members: BTreeMap<(String, String), Vec<u8>> = BTreeMap::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        total_size = total_size.saturating_add(entry.size());
        if total_size > MAX_ARCHIVE_UNCOMPRESSED {
            return fail("ZIP uncompressed size exceeds the 256 MiB safety limit");
        }
        let name = entry.name().to_owned();
        if !seen.insert(name.clone()) {
            return fail(format!("duplicate ZIP member: {name}"));
        }
        let parts = safe_zip_parts(&name)?;
        if entry.is_dir() || parts.len() < 2 {
            continue;
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return fail(format!("symbolic link is not allowed in ZIP: {name}"));
        }
        let folder = parts[parts.len() - 2];
        let leaf = parts[parts.len() - 1];
        if folder
            .strip_prefix("SRDR3")
            .is_none_or(|slot| !slot.chars().all(|character| character.is_ascii_digit()))
            || !matches!(leaf, "Save Game Data" | "Save Game Header")
        {
            continue;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        if members
            .insert((folder.to_owned(), leaf.to_owned()), bytes)
            .is_some()
        {
            return fail(format!("duplicate {leaf} for slot {folder}"));
        }
    }

    let folders: HashSet<String> = members.keys().map(|(folder, _)| folder.clone()).collect();
    if folders.is_empty() {
        return fail("no RDR2 SRDR story-save atoms were found in the ZIP");
    }

    let mut slots = Vec::new();
    for folder in folders {
        let data = members
            .remove(&(folder.clone(), "Save Game Data".into()))
            .ok_or_else(|| ConvertError::Message(format!("slot {folder} has no Save Game Data")))?;
        let header = members
            .remove(&(folder.clone(), "Save Game Header".into()))
            .ok_or_else(|| {
                ConvertError::Message(format!("slot {folder} has no Save Game Header"))
            })?;
        if header.len() != XBOX_HEADER_SIZE {
            return fail(format!(
                "slot {folder} header is not {XBOX_HEADER_SIZE} bytes"
            ));
        }
        if read_u32_le(&header, 0)? != XBOX_HEADER_VERSION {
            return fail(format!(
                "slot {folder} has an unexpected Xbox header version"
            ));
        }
        if read_u32_le(&header, 4)? as usize != data.len() {
            return fail(format!("slot {folder} header/data length mismatch"));
        }
        if data.is_empty() || data.len() % 16 != 0 {
            return fail(format!(
                "slot {folder} encrypted data is not AES-block aligned"
            ));
        }
        slots.push(XboxSlot {
            xbox_name: folder.clone(),
            pc_name: pc_name(&folder)?,
            title: decode_utf16le_field(&header[0x18..0x118])?,
            timestamp: read_u32_le(&header, 0x10)?,
            source_sha256: sha256_hex(&data),
            data,
        });
    }
    slots.sort_by(|left, right| left.pc_name.cmp(&right.pc_name));
    let unique: HashSet<_> = slots.iter().map(|slot| &slot.pc_name).collect();
    if unique.len() != slots.len() {
        return fail("multiple Xbox slots map to the same PC filename");
    }
    Ok(slots)
}

fn aes_ecb(data: &[u8], key: &[u8; 32], encrypt: bool) -> Result<Vec<u8>> {
    if data.len() % 16 != 0 {
        return fail("AES-ECB input is not 16-byte aligned");
    }
    let cipher = Aes256::new_from_slice(key)
        .map_err(|_| ConvertError::Message("invalid AES-256 key".into()))?;
    let mut output = data.to_vec();
    for chunk in output.chunks_exact_mut(16) {
        let block = GenericArray::from_mut_slice(chunk);
        if encrypt {
            cipher.encrypt_block(block);
        } else {
            cipher.decrypt_block(block);
        }
    }
    Ok(output)
}

fn digest_step(mut value: u32) -> u32 {
    value = value.wrapping_add(value.wrapping_shl(3));
    value ^= value >> 11;
    value.wrapping_add(value.wrapping_shl(15))
}

fn jooat_update(mut value: u32, data: &[u8]) -> u32 {
    for byte in data {
        let signed = (*byte as i8 as i32) as u32;
        value = value.wrapping_add(signed);
        value = value.wrapping_add(value.wrapping_shl(10));
        value ^= value >> 6;
    }
    value
}

fn jooat(data: &[u8]) -> u32 {
    digest_step(jooat_update(0x3FAC7125, data))
}

fn marker_offsets(data: &[u8]) -> Vec<usize> {
    data.windows(CHKS_MARKER.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == CHKS_MARKER).then_some(offset))
        .collect()
}

pub fn validate_rsav(payload: &[u8]) -> Result<RsavValidation> {
    if payload.len() < 0x100 || payload.len() % 16 != 0 {
        return fail("decrypted payload is not a valid AES/RSAV length");
    }
    if payload.get(..4) != Some(b"RSAV")
        || read_u32_le(payload, 4)? != RSAV_VERSION
        || read_u32_le(payload, 8)? as usize != RSAV_HEADER_SIZE
        || read_u32_le(payload, 0x10)? as usize != RSAV_TABLE_COUNT
    {
        return fail("invalid RSAV v3 header");
    }
    if payload.get(RSAV_HEADER_SIZE..RSAV_HEADER_SIZE + 4) != Some(b"CODE") {
        return fail("CODE block is not at the expected offset");
    }

    let mut block_ranges = Vec::with_capacity(RSAV_TABLE_COUNT);
    for (index, expected_block_id) in RSAV_BLOCK_IDS.iter().enumerate() {
        let entry = 0x14 + index * 0x10;
        let block_id = read_u32_le(payload, entry)?;
        let reserved = read_u32_le(payload, entry + 4)?;
        let block_offset = read_u32_le(payload, entry + 8)? as usize;
        let block_size = read_u32_le(payload, entry + 12)? as usize;
        if block_id != *expected_block_id || reserved != 0 {
            return fail(format!("invalid RSAV block-table entry {}", index + 1));
        }
        if block_offset % 16 != 0 || block_size < 0x20 || block_size % 16 != 0 {
            return fail(format!("unaligned RSAV block-table entry {}", index + 1));
        }
        let block_end = block_offset
            .checked_add(block_size)
            .ok_or_else(|| ConvertError::Message("RSAV block offset overflow".into()))?;
        if block_offset < RSAV_HEADER_SIZE || block_end > payload.len() {
            return fail(format!("out-of-range RSAV block-table entry {}", index + 1));
        }
        let expected_tag = format!("S00{index}");
        if payload.get(block_offset..block_offset + 4) != Some(expected_tag.as_bytes()) {
            return fail(format!("invalid RSAV section tag for block {}", index + 1));
        }
        block_ranges.push((block_offset, block_size));
    }
    for index in 0..RSAV_TABLE_COUNT - 1 {
        if block_ranges[index + 1].0 != block_ranges[index].0 + block_ranges[index].1 + 0x20 {
            return fail(format!(
                "RSAV blocks {} and {} are not contiguous",
                index + 1,
                index + 2
            ));
        }
    }
    let (last_offset, last_size) = block_ranges[RSAV_TABLE_COUNT - 1];
    if payload.len() != last_offset + last_size + 0x20 {
        return fail("last RSAV block does not line up with the file ending");
    }

    let code_size = read_u32_le(payload, 0x0C)? as usize;
    if code_size != block_ranges[0].0.saturating_sub(0xD0) {
        return fail("CODE length does not line up with the first data block");
    }
    let checksums = marker_offsets(payload);
    if checksums.len() != RSAV_TABLE_COUNT + 1 {
        return fail(format!(
            "expected 10 CHKS records, found {}",
            checksums.len()
        ));
    }

    for (index, &offset) in checksums.iter().enumerate() {
        let header_size = read_u32_be(payload, offset + 4)? as usize;
        let data_size = read_u32_be(payload, offset + 8)? as usize;
        let expected = read_u32_be(payload, offset + 12)?;
        if header_size != 0x14 || data_size < 0x10 {
            return fail(format!("invalid CHKS header {}", index + 1));
        }
        let start = offset
            .checked_sub(data_size)
            .and_then(|value| value.checked_add(header_size))
            .ok_or_else(|| ConvertError::Message(format!("CHKS {} range underflow", index + 1)))?;
        let end = offset + header_size;
        let (logical_offset, logical_size, next_limit) = if index == 0 {
            (RSAV_HEADER_SIZE, code_size, block_ranges[0].0)
        } else {
            let block_index = index - 1;
            let (block_offset, block_size) = block_ranges[block_index];
            let next = if block_index + 1 < block_ranges.len() {
                block_ranges[block_index + 1].0
            } else {
                payload.len()
            };
            (block_offset, block_size, next)
        };
        if start != logical_offset + 0x10
            || data_size.div_ceil(16) * 16 != logical_size
            || offset >= logical_offset + logical_size
            || end > next_limit
        {
            return fail(format!(
                "CHKS {} does not belong to its declared block",
                index + 1
            ));
        }
        let mut checked = payload[start..end].to_vec();
        let relative = offset - start;
        checked[relative + 8..relative + 16].fill(0);
        let actual = jooat(&checked);
        if actual != expected {
            return fail(format!(
                "CHKS {} mismatch: stored {expected:08x}, calculated {actual:08x}",
                index + 1
            ));
        }
    }
    if checksums.last().copied() != Some(payload.len() - 0x30)
        || payload.get(payload.len() - 16..) != Some(ENDS_BLOCK)
    {
        return fail("invalid RSAV ending");
    }

    Ok(RsavValidation {
        size: payload.len(),
        block_count: RSAV_TABLE_COUNT,
        checksum_count: checksums.len(),
    })
}

fn encode_utf16le_field(text: &str, size: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(size);
    for unit in text.encode_utf16() {
        if output.len() + 4 > size {
            break;
        }
        output.extend_from_slice(&unit.to_le_bytes());
    }
    output.extend_from_slice(&[0, 0]);
    output.resize(size, 0);
    output
}

fn swap_each_u32(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() % 4 != 0 {
        return fail("u32 byte-swap input is not aligned");
    }
    let mut output = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        output.extend(chunk.iter().rev());
    }
    Ok(output)
}

fn pc_header_checksum(header: &[u8]) -> Result<u32> {
    if header.len() != PC_HEADER_SIZE {
        return fail("PC header has the wrong length");
    }
    let first = swap_each_u32(&header[..4])?;
    let second = swap_each_u32(&header[0x104..0x10C])?;
    let value = digest_step(jooat_update(0, &first));
    Ok(digest_step(jooat_update(value, &second)))
}

fn build_pc_header(title: &str, timestamp: u32) -> Result<Vec<u8>> {
    let mut header = vec![0_u8; PC_HEADER_SIZE];
    header[..4].copy_from_slice(&4_u32.to_be_bytes());
    header[4..0x104].copy_from_slice(&encode_utf16le_field(title, 0x100));
    header[0x104..0x108].copy_from_slice(&timestamp.to_be_bytes());
    let checksum = pc_header_checksum(&header)?;
    header[0x10C..0x110].copy_from_slice(&checksum.to_be_bytes());
    Ok(header)
}

pub fn validate_pc_file(pc_file: &[u8], pc_key: &[u8; 32]) -> Result<RsavValidation> {
    if pc_file.len() < PC_HEADER_SIZE + 16 || (pc_file.len() - PC_HEADER_SIZE) % 16 != 0 {
        return fail("PC save has an invalid length");
    }
    if read_u32_be(pc_file, 0)? != 4 {
        return fail("PC save has an invalid header version");
    }
    let expected = read_u32_be(pc_file, 0x10C)?;
    let mut scratch = pc_file[..PC_HEADER_SIZE].to_vec();
    scratch[0x10C..0x110].fill(0);
    if pc_header_checksum(&scratch)? != expected {
        return fail("PC header checksum mismatch");
    }
    let payload = aes_ecb(&pc_file[PC_HEADER_SIZE..], pc_key, false)?;
    let validation = validate_rsav(&payload)?;
    if aes_ecb(&payload, pc_key, true)? != pc_file[PC_HEADER_SIZE..] {
        return fail("PC encryption round trip failed");
    }
    Ok(validation)
}

fn verify_xbox_key(key: &[u8; 32], slots: &[XboxSlot]) -> Result<()> {
    for slot in slots {
        let first = aes_ecb(&slot.data[..16], key, false)?;
        let last = aes_ecb(&slot.data[slot.data.len() - 16..], key, false)?;
        if first.get(..4) != Some(b"RSAV")
            || read_u32_le(&first, 4)? != RSAV_VERSION
            || read_u32_le(&first, 8)? as usize != RSAV_HEADER_SIZE
            || read_u32_le(&first, 12)? == 0
            || last.as_slice() != ENDS_BLOCK
        {
            return fail(format!("Xbox key does not match slot {}", slot.xbox_name));
        }
    }
    Ok(())
}

pub fn convert_archive(
    zip_path: &Path,
    output_dir: &Path,
    config_path: &Path,
) -> Result<Vec<ManifestEntry>> {
    let config = KeyConfig::load(config_path)?;
    let (xbox_key, pc_key) = config.keys()?;
    let slots = load_xbox_slots(zip_path)?;
    verify_xbox_key(&xbox_key, &slots)?;

    let mut prepared = Vec::with_capacity(slots.len());
    for slot in &slots {
        let payload = aes_ecb(&slot.data, &xbox_key, false)?;
        let xbox_validation = validate_rsav(&payload)?;
        if aes_ecb(&payload, &xbox_key, true)? != slot.data {
            return fail(format!(
                "Xbox encryption round trip failed for {}",
                slot.xbox_name
            ));
        }
        let mut pc_file = build_pc_header(&slot.title, slot.timestamp)?;
        pc_file.extend_from_slice(&aes_ecb(&payload, &pc_key, true)?);
        let pc_validation = validate_pc_file(&pc_file, &pc_key)?;
        if pc_validation != xbox_validation {
            return fail(format!("PC validation differs for {}", slot.xbox_name));
        }
        let decrypted_pc = aes_ecb(&pc_file[PC_HEADER_SIZE..], &pc_key, false)?;
        if decrypted_pc != payload {
            return fail(format!("RSAV payload changed for {}", slot.xbox_name));
        }
        prepared.push((slot.clone(), pc_file, pc_validation));
    }

    if output_dir.exists() {
        return fail(format!(
            "refusing to overwrite existing output directory: {}",
            output_dir.display()
        ));
    }
    let parent = output_dir.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let leaf = output_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("converted_pc_saves");
    let temporary = parent.join(format!(".{leaf}.{}.tmp", std::process::id()));
    if temporary.exists() {
        return fail(format!(
            "temporary output already exists: {}",
            temporary.display()
        ));
    }
    fs::create_dir(&temporary)?;

    let result = (|| -> Result<Vec<ManifestEntry>> {
        let mut manifest = Vec::with_capacity(prepared.len());
        for (slot, pc_file, validation) in &prepared {
            fs::write(temporary.join(&slot.pc_name), pc_file)?;
            manifest.push(ManifestEntry {
                source_slot: slot.xbox_name.clone(),
                pc_file: slot.pc_name.clone(),
                title: slot.title.clone(),
                timestamp: slot.timestamp,
                source_sha256: slot.source_sha256.clone(),
                pc_sha256: sha256_hex(pc_file),
                pc_size: pc_file.len(),
                rsav_checksums: validation.checksum_count,
            });
        }
        let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";
        fs::write(temporary.join("conversion_manifest.json"), manifest_json)?;
        fs::rename(&temporary, output_dir)?;
        Ok(manifest)
    })();

    if result.is_err() && temporary.exists() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

pub fn default_config_path() -> PathBuf {
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        let portable = parent.join("rdr2-converter.toml");
        if portable.exists() {
            return portable;
        }
    }
    directories::ProjectDirs::from("", "hupose", "RDR2 Xbox Save Converter")
        .map(|dirs| dirs.config_dir().join("keys.toml"))
        .unwrap_or_else(|| PathBuf::from("keys.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_names_are_mapped_without_guessing() {
        assert_eq!(pc_name("SRDR30000").unwrap(), "SRDR30000");
        assert_eq!(pc_name("SRDR300014").unwrap(), "SRDR30014");
        assert_eq!(pc_name("SRDR300015").unwrap(), "SRDR30015");
        assert!(pc_name("SRDR300016").is_err());
    }

    #[test]
    fn blank_or_short_keys_are_rejected() {
        assert!(parse_key("test", "").is_err());
        assert!(parse_key("test", "0011").is_err());
        assert!(parse_key("test", &"00".repeat(32)).is_ok());
    }

    #[test]
    fn pc_header_is_self_consistent() {
        let header = build_pc_header("Test save", 1_700_000_000).unwrap();
        let expected = read_u32_be(&header, 0x10C).unwrap();
        let mut scratch = header;
        scratch[0x10C..0x110].fill(0);
        assert_eq!(pc_header_checksum(&scratch).unwrap(), expected);
    }
}
