//! Byte serializers for the `dictionary.bin` (TKDB) and `association.bin`
//! (TKWA) fixtures the lexicon readers open.

const TKDB_HEADER_SIZE: usize = 16;
const TKWA_HEADER_SIZE: usize = 20;

/// One TKDB record. The optional fields drive the on-disk record layout,
/// independently of the header `version` (tests forge mismatches on purpose):
/// - `syllable_count = None` → v1 layout (no syllable_count byte);
///   `Some(n)` → v2+ layout with that count.
/// - `kautian_subtag = None` → v1/v2 layout (no subtag bytes);
///   `Some(s)` → v3 layout with the 2-byte subtag after syllable_count.
pub struct TkdbRow<'a> {
    pub bitmask: u16,
    pub frequency: u32,
    pub syllable_count: Option<u8>,
    pub kautian_subtag: Option<u16>,
    pub hanji: &'a str,
    pub tl: &'a str,
}

/// Build a TKDB byte sequence with the given `magic`, header `version` and
/// rows (offset table, then the record payload).
pub fn build_tkdb(magic: &[u8; 4], version: u32, rows: &[TkdbRow<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(magic);
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // build_ts

    let offset_table_size = rows.len() * 4;
    let mut offsets = Vec::<u32>::with_capacity(rows.len());
    let mut payload = Vec::<u8>::new();
    for row in rows {
        offsets.push((TKDB_HEADER_SIZE + offset_table_size + payload.len()) as u32);
        payload.extend_from_slice(&row.bitmask.to_le_bytes());
        payload.extend_from_slice(&row.frequency.to_le_bytes());
        payload.push(row.hanji.len() as u8);
        payload.push(row.tl.len() as u8);
        if let Some(syll) = row.syllable_count {
            payload.push(syll);
        }
        if let Some(subtag) = row.kautian_subtag {
            payload.extend_from_slice(&subtag.to_le_bytes());
        }
        payload.extend_from_slice(row.hanji.as_bytes());
        payload.extend_from_slice(row.tl.as_bytes());
    }
    for off in &offsets {
        out.extend_from_slice(&off.to_le_bytes());
    }
    out.extend_from_slice(&payload);
    out
}

/// One TKWA entry: `(bitmask, count, next_word, next_tl)`.
pub type TkwaEntry<'a> = (u16, u32, &'a str, &'a str);

/// Build a TKWA byte sequence: `version`, then `keys` in the order given
/// (the caller sorts them by raw UTF-8 bytes — the reader binary-searches).
pub fn build_tkwa(version: u32, keys: &[(&str, &[TkwaEntry<'_>])]) -> Vec<u8> {
    let key_section_start = TKWA_HEADER_SIZE + keys.len() * 4;
    let key_sizes: Vec<usize> = keys.iter().map(|(key, _)| 1 + key.len() + 4 + 2).collect();
    let entry_section_start = key_section_start + key_sizes.iter().sum::<usize>();
    let entry_size = |(_, _, nw, nt): &TkwaEntry<'_>| 8 + nw.len() + nt.len();

    let mut out = Vec::new();
    out.extend_from_slice(b"TKWA");
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&(keys.len() as u32).to_le_bytes());
    let entry_count: usize = keys.iter().map(|(_, entries)| entries.len()).sum();
    out.extend_from_slice(&(entry_count as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // build_ts

    let mut key_offset = key_section_start;
    for size in &key_sizes {
        out.extend_from_slice(&(key_offset as u32).to_le_bytes());
        key_offset += size;
    }
    let mut entry_offset = entry_section_start;
    for (key, entries) in keys {
        assert!(key.len() <= u8::MAX as usize, "key too long");
        out.push(key.len() as u8);
        out.extend_from_slice(key.as_bytes());
        out.extend_from_slice(&(entry_offset as u32).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        entry_offset += entries.iter().map(entry_size).sum::<usize>();
    }
    for (_, entries) in keys {
        for (bitmask, count, next_word, next_tl) in *entries {
            out.extend_from_slice(&bitmask.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            out.push(next_word.len() as u8);
            out.push(next_tl.len() as u8);
            out.extend_from_slice(next_word.as_bytes());
            out.extend_from_slice(next_tl.as_bytes());
        }
    }
    out
}
