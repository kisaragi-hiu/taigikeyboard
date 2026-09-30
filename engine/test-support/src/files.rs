//! Temp fixture files and FST sets.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fst::SetBuilder;

/// Byte between a `dictionary.fst` key and its little-endian rowid.
const ROWID_SEPARATOR: u8 = 0xFF;

/// Fresh temp path per call: pid plus a per-process counter, so parallel
/// tests in one binary — or two concurrent runs of it — never truncate a
/// fixture another test still has open (mmapped). `tag` only aids debugging.
fn temp_path(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("taigi-test-{pid}-{n}-{tag}"))
}

/// Write `bytes` to a fresh [`temp_path`] and return it.
pub fn write_temp(tag: &str, bytes: &[u8]) -> PathBuf {
    let path = temp_path(tag);
    std::fs::write(&path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    path
}

/// `<family><body> + 0xFF + rowid_le_u32` — the production `dictionary.fst`
/// entry shape (`dictionary/build/create_fst.py`). `family` is the key
/// prefix (`b"tl:"`), or empty when `body` already carries it.
pub fn fst_entry(family: &[u8], body: &str, rowid: u32) -> Vec<u8> {
    let mut e = Vec::with_capacity(family.len() + body.len() + 5);
    e.extend_from_slice(family);
    e.extend_from_slice(body.as_bytes());
    e.push(ROWID_SEPARATOR);
    e.extend_from_slice(&rowid.to_le_bytes());
    e
}

/// Write `entries` (sorted and deduped here) as an FST set to a fresh
/// [`temp_path`] and return it.
pub fn write_fst_set(tag: &str, mut entries: Vec<Vec<u8>>) -> PathBuf {
    entries.sort();
    entries.dedup();
    let path = temp_path(tag);
    let file = std::fs::File::create(&path).unwrap_or_else(|e| panic!("create {tag}: {e}"));
    let mut builder = SetBuilder::new(std::io::BufWriter::new(file)).expect("fst builder");
    for entry in &entries {
        builder.insert(entry).expect("fst insert");
    }
    builder.finish().expect("fst finish");
    path
}
