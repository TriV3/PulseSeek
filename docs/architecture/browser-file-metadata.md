# Browser File Metadata

Folder enumeration reads filesystem and basic audio metadata on dedicated
workers, not on React or audio threads. Direct-child files are decoder-verified
by a bounded pool of up to six workers, while keeping two logical cores free,
and streamed to React in batches of at most 16 entries. Verification uses the
container and codec headers without constructing playback decoders. This lets
trusted rows appear progressively while preserving CPU and I/O capacity for
playback. Playable rows expose duration, size, modification date, channels,
sample rate, bit depth, and codec when available.

Before verification, the lightweight directory preview exposes recognized
audio filenames with loading metadata. A failed content probe emits a tombstone
that removes the candidate; files with unsupported extensions never enter the
default list.

Metadata is best-effort. Missing filesystem fields, unknown duration, absent bit
depth, or decoder metadata errors leave individual values unavailable and never
remove an otherwise playable row. While folder enumeration remains active, a
row without metadata displays loading placeholders. The loading state remains
active until file verification finishes, even when the folder-only preview has
already completed, so an in-progress folder is never labelled empty. Completed
missing values display an em dash.

Zero-valued decoder fields used to represent unknown stream metadata are treated
as unavailable. Folder events are validated at the frontend boundary; malformed,
unsafe-integer, or invalid-date payloads are ignored rather than rendered.

UI formatting uses compact duration, IEC file sizes, local date/time, channel
labels, kHz sample rates, and bit-depth labels. Narrow layouts scroll
horizontally; column layout and sorting remain fixed until later browser work.
