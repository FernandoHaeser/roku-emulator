/**
 * Output surface of the reference device (profiles/H302X-16.0.4.json,
 * `ui-resolution: 720p`). The device composes every channel at its UI
 * resolution and scales the channel's design resolution (FHD/HD) to it, so
 * screenshots are 1280x720 even for FHD channels. Rendering at the same size
 * keeps text rasterization and edge rounding comparable with device goldens.
 */
export const UI_RESOLUTIONS = {
  "720p": { width: 1280, height: 720 },
  "1080p": { width: 1920, height: 1080 },
} as const;

export const DEVICE_UI_RESOLUTION = "720p" satisfies keyof typeof UI_RESOLUTIONS;

export const SURFACE = UI_RESOLUTIONS[DEVICE_UI_RESOLUTION];
