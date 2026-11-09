/** Packages a probe directory into a sideload zip the device accepts (explicit directory entries, no symlinks). */
import { lstatSync, readdirSync, readFileSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { zipSync, type Zippable } from "fflate";

/** Files/dirs inside a probe directory that are oracle metadata, not channel content. */
const EXCLUDED_ENTRIES = new Set(["golden", "probe.json", ".DS_Store"]);

/** Upper bound for a sideloaded package; keeps a runaway directory from being zipped. */
const MAX_PACKAGE_BYTES = 64 * 1024 * 1024;

export class PackageError extends Error {
  override name = "PackageError";
}

/**
 * Zips a channel directory into an in-memory sideload package.
 * Symlinks are rejected so a probe cannot pull files from outside its folder.
 */
export function packChannel(channelDir: string): Uint8Array {
  const files: Zippable = {};
  let totalBytes = 0;

  const walk = (dir: string): void => {
    for (const entry of readdirSync(dir).sort()) {
      if (dir === channelDir && EXCLUDED_ENTRIES.has(entry)) continue;
      if (entry === ".DS_Store") continue;
      const fullPath = join(dir, entry);
      const stats = lstatSync(fullPath);
      if (stats.isSymbolicLink()) {
        throw new PackageError(`Symlinks are not allowed in channel packages: ${fullPath}`);
      }
      // Zip entries always use forward slashes, whatever the host OS.
      const entryName = relative(channelDir, fullPath).split(sep).join("/");
      if (stats.isDirectory()) {
        // The Roku installer's unzip fails ("Invalid or corrupt zip archive") unless
        // every directory has its own explicit "dir/" entry, as `zip -r` produces.
        files[`${entryName}/`] = new Uint8Array();
        walk(fullPath);
        continue;
      }
      totalBytes += stats.size;
      if (totalBytes > MAX_PACKAGE_BYTES) {
        throw new PackageError(`Channel package exceeds ${MAX_PACKAGE_BYTES} bytes`);
      }
      files[entryName] = readFileSync(fullPath);
    }
  };

  walk(channelDir);
  if (!("manifest" in files)) {
    throw new PackageError(`No manifest found at the root of ${channelDir}`);
  }
  return zipSync(files, { level: 6, mtime: new Date("2020-01-01T00:00:00Z") });
}
