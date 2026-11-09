/**
 * Bundled substitute for the Roku system font, which is not redistributable:
 * Inter (variable weight, SIL OFL 1.1). Shipping it instead of `system-ui`
 * makes rendering identical on every OS, so device screenshot diffs are
 * reproducible in CI. Layout never depends on it (device metrics drive line
 * breaks and widths); only glyph shapes do.
 */
import latinExtUrl from "@fontsource-variable/inter/files/inter-latin-ext-wght-normal.woff2?url";
import latinUrl from "@fontsource-variable/inter/files/inter-latin-wght-normal.woff2?url";

export const SUBSTITUTE_FAMILY = "RokuEmuSans";

/** Ranges from @fontsource-variable/inter/index.css. */
const FACES = [
  {
    url: latinUrl,
    unicodeRange:
      "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD",
  },
  {
    url: latinExtUrl,
    unicodeRange:
      "U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF",
  },
];

/** Registers the faces in this worker; resolves even if loading fails (fallback stack). */
export async function loadSubstituteFont(): Promise<boolean> {
  const fonts = (globalThis as { fonts?: FontFaceSet }).fonts;
  if (!fonts || typeof FontFace === "undefined") return false;
  try {
    await Promise.all(
      FACES.map(async ({ url, unicodeRange }) => {
        const face = new FontFace(SUBSTITUTE_FAMILY, `url(${url})`, {
          weight: "100 900",
          unicodeRange,
        });
        fonts.add(await face.load());
      }),
    );
    return true;
  } catch {
    return false;
  }
}
