import type { BrowserEntry } from "../FolderTree/folderTreeTypes";

/**
 * Audio formats supported by the File List. Values map to filename
 * extensions, which are also the browser's explicit allow-list.
 */
export type AudioFileFormat = "mp3" | "flac" | "pcm";

/** Selectable format options shown in the file list filter. */
export const FORMAT_OPTIONS: ReadonlyArray<{
  value: AudioFileFormat;
  label: string;
}> = [
  { value: "mp3", label: "MP3" },
  { value: "flac", label: "FLAC" },
  { value: "pcm", label: "WAV/PCM" },
];

/**
 * Returns the supported filename format of a playable entry, or `null` for
 * folders and every other extension.
 */
export function formatOf(entry: BrowserEntry): AudioFileFormat | null {
  if (entry.kind !== "playable") return null;
  const extension = entry.name.split(".").at(-1)?.toLowerCase();
  switch (extension) {
    case "mp3":
      return "mp3";
    case "flac":
      return "flac";
    case "wav":
    case "wave":
      return "pcm";
    default:
      return null;
  }
}

/**
 * Filters visible playable files by decoded format.
 *
 * This is a pure in-memory filter over entries already streamed by Rust: it
 * never touches the filesystem, so changing the filter never re-enumerates the
 * folder. An empty format list keeps every entry. Folder rows always stay
 * visible so the user can navigate out of a filtered list. Entries with an
 * Unsupported extensions are excluded whenever a format filter is active.
 */
export function filterByFormat(
  entries: BrowserEntry[],
  formats: ReadonlyArray<AudioFileFormat>,
): BrowserEntry[] {
  if (formats.length === 0) return [...entries];

  return entries.filter((entry) => {
    if (entry.kind === "folder") return true;
    const format = formatOf(entry);
    return format !== null && formats.includes(format);
  });
}
