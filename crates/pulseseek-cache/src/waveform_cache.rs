//! Waveform cache: store and retrieve multiresolution waveform data under a
//! versioned file cache key.
//!
//! Fulfills FR-VS-010: cached waveform data remains technical cache data in
//! `app-cache.sqlite` and never creates a manager item. The cache crate owns
//! the binary codec, the versioned key scheme, and the validation rules, so
//! the domain model stays independent of persistence.
//!
//! A stored row carries the source size, modified timestamp, and extraction
//! algorithm version. A load treats a row as a miss and deletes it when any of
//! those no longer matches the caller's identity (stale) or when the blob
//! cannot be decoded (corrupt), so the cache self-heals on access.
//! The file watcher also invalidates rows proactively when an external change
//! is observed (FR-FM-010).
//!
//! # Privacy
//!
//! The key is derived from the source path. It is a technical cache key only,
//! never a manager identifier, and no manager database is ever touched.

use std::fmt;
use std::fmt::Write as _;
use std::path::PathBuf;

use pulseseek_domain::waveform::levels::{Level, LevelIndex, MultiresolutionWaveform, MAX_LEVELS};
use pulseseek_domain::waveform::peak::Peak;
use rusqlite::params;

use crate::sqlite::{now_ms, OpenedDatabase};

/// Format version of the serialized waveform codec.
///
/// Bumping this invalidates every cached blob regardless of algorithm changes.
pub const WAVEFORM_FORMAT_VERSION: u32 = 1;

/// Version of the waveform extraction algorithm whose output the cache stores.
///
/// Rows written by an older algorithm are never reused and are deleted on
/// access.
pub const WAVEFORM_ALGORITHM_VERSION: u32 = 3;

/// Version of the cache-key scheme.
///
/// Bumping this changes every derived key, naturally invalidating all rows
/// written by an older key format.
pub const WAVEFORM_KEY_FORMAT_VERSION: u32 = 2;

/// Magic bytes at the start of every serialized waveform blob.
const BLOB_MAGIC: &[u8; 4] = b"PSWF";

/// Identity of the source file a cached waveform describes.
///
/// The path scopes the cache key; the size and modified timestamp are
/// validation columns compared on every load so an edited source file
/// invalidates its cached row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaveformIdentity {
    /// Path of the source audio file.
    pub path: PathBuf,
    /// Size of the source file in bytes.
    pub size: u64,
    /// Last-modified timestamp of the source file in milliseconds.
    pub modified_ms: u64,
}

impl WaveformIdentity {
    /// Builds a source identity from its path, size, and modified timestamp.
    pub fn new(path: impl Into<PathBuf>, size: u64, modified_ms: u64) -> Self {
        Self { path: path.into(), size, modified_ms }
    }
}

/// Derives the versioned file cache key for `identity`.
///
/// The key is scoped by the source path and prefixed with the key-scheme
/// version. The path is canonicalized when the file exists so the same file
/// reached through a symlinked prefix (for example `/var` on macOS, which
/// resolves to `/private/var`) always derives the same key; the file watcher
/// delivers canonicalized paths, so this keeps proactive invalidation aligned
/// with stored rows. When the file no longer exists the raw path is used.
/// Size and modified timestamp intentionally do not participate in the key:
/// they are validation columns so an edited source invalidates the same row
/// instead of leaving orphaned rows behind.
pub fn waveform_cache_key(identity: &WaveformIdentity) -> String {
    let canonical = std::fs::canonicalize(&identity.path).unwrap_or_else(|_| identity.path.clone());
    format!(
        "waveform:v{WAVEFORM_KEY_FORMAT_VERSION}:{}",
        hex_encode(canonical.as_os_str().as_encoded_bytes())
    )
}

/// Error produced by waveform cache operations.
#[derive(Debug)]
pub enum WaveformCacheError {
    /// A query against the cache database failed.
    Sqlite(rusqlite::Error),
    /// A waveform could not be serialized.
    Encode(&'static str),
    /// Stored bytes are not a decodable waveform.
    Decode(&'static str),
    /// The cache worker stopped before answering.
    WorkerStopped,
}

impl fmt::Display for WaveformCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "waveform cache query failed: {error}"),
            Self::Encode(reason) => write!(f, "waveform encoding failed: {reason}"),
            Self::Decode(reason) => write!(f, "waveform decoding failed: {reason}"),
            Self::WorkerStopped => write!(f, "waveform cache worker stopped"),
        }
    }
}

impl std::error::Error for WaveformCacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Encode(_) | Self::Decode(_) | Self::WorkerStopped => None,
        }
    }
}

/// Port for persisting and loading cached waveform data.
///
/// Implementations must be `Send` and `Sync` so the cache can live behind a
/// shared reference in application state.
pub trait WaveformCachePort: Send + Sync {
    /// Stores `waveform` under `key` with `identity` as validation metadata.
    fn store_waveform(
        &self,
        key: &str,
        identity: &WaveformIdentity,
        waveform: &MultiresolutionWaveform,
    ) -> Result<(), WaveformCacheError>;

    /// Loads the waveform stored under `key`.
    ///
    /// Returns `Ok(None)` on a miss, when the stored row is stale relative to
    /// `identity`, or when the row is corrupt. Stale and corrupt rows are
    /// deleted so the cache self-heals.
    fn load_waveform(
        &self,
        key: &str,
        identity: &WaveformIdentity,
    ) -> Result<Option<MultiresolutionWaveform>, WaveformCacheError>;

    /// Deletes the waveform stored under `key`.
    ///
    /// Deleting a missing key is a no-op. Used by the file watcher so an
    /// externally modified source invalidates its cached row proactively
    /// (FR-FM-010).
    fn delete_waveform(&self, key: &str) -> Result<(), WaveformCacheError>;
}

/// Serializes a validated waveform into the versioned binary format.
pub fn encode(waveform: &MultiresolutionWaveform) -> Result<Vec<u8>, WaveformCacheError> {
    let channels = waveform.channels();
    if channels == 0 {
        return Err(WaveformCacheError::Encode("waveform has no channels"));
    }
    let level_count = waveform.len();
    if level_count == 0 || level_count > MAX_LEVELS as usize {
        return Err(WaveformCacheError::Encode("invalid level count"));
    }

    let total_peaks: usize = waveform.levels().iter().map(|level| level.peaks.len()).sum();
    let capacity = HEADER_BYTES + level_count * LEVEL_METADATA_BYTES + total_peaks * PEAK_BYTES;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(BLOB_MAGIC);
    out.extend_from_slice(&WAVEFORM_FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&WAVEFORM_ALGORITHM_VERSION.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.push(level_count as u8);
    for level in waveform.levels() {
        out.extend_from_slice(&level.index.value().to_le_bytes());
        out.extend_from_slice(&level.samples_per_peak.to_le_bytes());
        out.extend_from_slice(&(level.peaks.len() as u64).to_le_bytes());
        for peak in &level.peaks {
            out.extend_from_slice(&peak.min().to_bits().to_le_bytes());
            out.extend_from_slice(&peak.max().to_bits().to_le_bytes());
        }
    }
    Ok(out)
}

/// Decodes bytes previously produced by [`encode`].
///
/// Structural invariants are validated with
/// [`MultiresolutionWaveform::from_levels`] so malformed, truncated, or
/// version-mismatched blobs fail with [`WaveformCacheError::Decode`].
pub fn decode(bytes: &[u8]) -> Result<MultiresolutionWaveform, WaveformCacheError> {
    let mut cursor = Cursor::new(bytes);
    if cursor.take(BLOB_MAGIC.len())? != *BLOB_MAGIC {
        return Err(WaveformCacheError::Decode("bad magic"));
    }
    if cursor.take_u32()? != WAVEFORM_FORMAT_VERSION {
        return Err(WaveformCacheError::Decode("unsupported format version"));
    }
    if cursor.take_u32()? != WAVEFORM_ALGORITHM_VERSION {
        return Err(WaveformCacheError::Decode("unsupported algorithm version"));
    }
    let channels = cursor.take_u16()?;
    if channels == 0 {
        return Err(WaveformCacheError::Decode("zero channels"));
    }
    let level_count = cursor.take_u8()? as usize;
    if level_count == 0 || level_count > MAX_LEVELS as usize {
        return Err(WaveformCacheError::Decode("invalid level count"));
    }

    let mut levels = Vec::with_capacity(level_count);
    for _ in 0..level_count {
        let index = LevelIndex::new(cursor.take_u32()?)
            .map_err(|_| WaveformCacheError::Decode("invalid level index"))?;
        let samples_per_peak = cursor.take_u64()?;
        let peak_count = cursor.take_u64()?;
        if peak_count > usize::MAX as u64 {
            return Err(WaveformCacheError::Decode("peak count too large"));
        }
        // Bound the allocation by the bytes actually remaining in the blob so
        // a corrupt row cannot force a huge allocation or an out-of-memory
        // abort; each peak occupies exactly `PEAK_BYTES`.
        if peak_count as usize > cursor.remaining() / PEAK_BYTES {
            return Err(WaveformCacheError::Decode("peak count exceeds blob data"));
        }
        let mut peaks = Vec::with_capacity(peak_count as usize);
        for _ in 0..peak_count {
            let min = f32::from_bits(cursor.take_u32()?);
            let max = f32::from_bits(cursor.take_u32()?);
            peaks.push(Peak::from_parts(min, max));
        }
        levels.push(Level { index, samples_per_peak, peaks });
    }
    if cursor.remaining() > 0 {
        return Err(WaveformCacheError::Decode("trailing bytes"));
    }
    MultiresolutionWaveform::from_levels(channels, levels)
        .map_err(|_| WaveformCacheError::Decode("invalid waveform structure"))
}

const HEADER_BYTES: usize = BLOB_MAGIC.len() + 4 + 4 + 2 + 1;
const LEVEL_METADATA_BYTES: usize = 4 + 8 + 8;
const PEAK_BYTES: usize = 4 + 4;

/// Bounds-checked little-endian reader over a byte slice.
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], WaveformCacheError> {
        let end =
            self.offset.checked_add(len).ok_or(WaveformCacheError::Decode("length overflow"))?;
        if end > self.bytes.len() {
            return Err(WaveformCacheError::Decode("truncated blob"));
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn take_u8(&mut self) -> Result<u8, WaveformCacheError> {
        Ok(self.take(1)?[0])
    }

    fn take_u16(&mut self) -> Result<u16, WaveformCacheError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn take_u32(&mut self) -> Result<u32, WaveformCacheError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn take_u64(&mut self) -> Result<u64, WaveformCacheError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().expect("exactly eight bytes")))
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

/// Stores a serialized blob under `key` on the worker's connection.
pub(crate) fn store_waveform_db(
    database: &mut OpenedDatabase,
    key: &str,
    identity: &WaveformIdentity,
    data: &[u8],
) -> Result<(), WaveformCacheError> {
    database
        .connection()
        .execute(
            "INSERT INTO waveform_cache \
             (cache_key, source_size, source_modified_ms, algorithm_version, data, created_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(cache_key) DO UPDATE SET \
             source_size = excluded.source_size, \
             source_modified_ms = excluded.source_modified_ms, \
             algorithm_version = excluded.algorithm_version, \
             data = excluded.data, \
             created_at_ms = excluded.created_at_ms",
            params![
                key,
                identity.size as i64,
                identity.modified_ms as i64,
                WAVEFORM_ALGORITHM_VERSION as i64,
                data,
                now_ms(),
            ],
        )
        .map_err(WaveformCacheError::Sqlite)?;
    Ok(())
}

/// Loads and validates a stored waveform on the worker's connection.
pub(crate) fn load_waveform_db(
    database: &mut OpenedDatabase,
    key: &str,
    identity: &WaveformIdentity,
) -> Result<Option<MultiresolutionWaveform>, WaveformCacheError> {
    let result = database.connection().query_row(
        "SELECT data, source_size, source_modified_ms, algorithm_version \
         FROM waveform_cache WHERE cache_key = ?1",
        params![key],
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    );
    let (data, source_size, source_modified_ms, algorithm_version) = match result {
        Ok(row) => row,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(error) => return Err(WaveformCacheError::Sqlite(error)),
    };

    if source_size != identity.size as i64
        || source_modified_ms != identity.modified_ms as i64
        || algorithm_version != WAVEFORM_ALGORITHM_VERSION as i64
    {
        delete_waveform_db(database, key)?;
        return Ok(None);
    }

    match decode(&data) {
        Ok(waveform) => Ok(Some(waveform)),
        Err(WaveformCacheError::Decode(_)) => {
            delete_waveform_db(database, key)?;
            Ok(None)
        },
        Err(error) => Err(error),
    }
}

/// Removes a waveform row on the worker's connection.
pub(crate) fn delete_waveform_db(
    database: &mut OpenedDatabase,
    key: &str,
) -> Result<(), WaveformCacheError> {
    database
        .connection()
        .execute("DELETE FROM waveform_cache WHERE cache_key = ?1", params![key])
        .map_err(WaveformCacheError::Sqlite)?;
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn key_prefix_encodes_format_version() {
        let identity = WaveformIdentity::new(Path::new("/music/track.wav"), 1000, 100);
        let key = waveform_cache_key(&identity);
        assert!(key.starts_with("waveform:v2:"));
        assert!(key.contains("2f6d757369632f747261636b2e776176"), "hex-encoded path");
    }

    #[test]
    fn hex_encoding_round_trips() {
        assert_eq!(hex_encode(b"\x00\x01\xff"), "0001ff");
    }
}
