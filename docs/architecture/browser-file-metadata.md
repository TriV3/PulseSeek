# Browser File Metadata

Folder enumeration reads filesystem and basic audio metadata on its worker, not
on React or audio threads. Playable rows expose duration, size, modification
date, channels, sample rate, bit depth, and codec when available.

Metadata is best-effort. Missing filesystem fields, unknown duration, absent bit
depth, or decoder metadata errors leave individual values unavailable and never
remove an otherwise playable row. While folder enumeration remains active, a
row without metadata displays loading placeholders. Completed missing values
display an em dash.

Zero-valued decoder fields used to represent unknown stream metadata are treated
as unavailable. Folder events are validated at the frontend boundary; malformed,
unsafe-integer, or invalid-date payloads are ignored rather than rendered.

UI formatting uses compact duration, IEC file sizes, local date/time, channel
labels, kHz sample rates, and bit-depth labels. Narrow layouts scroll
horizontally; column layout and sorting remain fixed until later browser work.
