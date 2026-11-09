/**
 * Canvas2D renderer for the engine's display list. Draws in design
 * coordinates scaled to the surface (the device also scales the channel's
 * ui_resolution to the output mode).
 *
 * Provisional fidelity (F4 refines it against device screenshots): system
 * font metrics, Poster blendColor and image filtering are approximations.
 */
import { layoutLines, parseDisplayList, rgbaToCss, type DisplayList } from "./displayList";

/** Fallback stack: the Roku system font is not redistributable. */
const FONT_FAMILY = "system-ui, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif";

type ImageEntry =
  { state: "loading" } | { state: "ready"; bitmap: ImageBitmap } | { state: "failed" };

export class CanvasRenderer {
  private readonly context: OffscreenCanvasRenderingContext2D;
  private readonly images = new Map<string, ImageEntry>();
  private list: DisplayList | null = null;
  private framePending = false;

  /**
   * @param readPackageFile resolves `pkg:/` URIs from the running channel.
   */
  constructor(
    private readonly canvas: OffscreenCanvas,
    private readonly readPackageFile: (
      uri: string,
    ) => Uint8Array<ArrayBuffer> | null | Promise<Uint8Array<ArrayBuffer> | null>,
  ) {
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) throw new Error("Canvas2D is not available in this worker");
    this.context = context;
    this.clear();
  }

  /** Black screen (no scene shown / channel exited). Drops cached images. */
  clear(): void {
    this.list = null;
    for (const entry of this.images.values()) {
      if (entry.state === "ready") entry.bitmap.close();
    }
    this.images.clear();
    this.context.fillStyle = "#000";
    this.context.fillRect(0, 0, this.canvas.width, this.canvas.height);
  }

  show(json: string): void {
    this.list = parseDisplayList(json);
    this.requestFrame();
  }

  private requestFrame(): void {
    if (this.framePending) return;
    this.framePending = true;
    const draw = () => {
      this.framePending = false;
      this.draw();
    };
    if (typeof requestAnimationFrame === "function") requestAnimationFrame(draw);
    else setTimeout(draw, 16);
  }

  private image(uri: string): ImageBitmap | null {
    const entry = this.images.get(uri);
    if (entry) return entry.state === "ready" ? entry.bitmap : null;
    this.images.set(uri, { state: "loading" });
    void this.loadImage(uri).then(
      (bitmap) => {
        this.images.set(uri, { state: "ready", bitmap });
        this.requestFrame();
      },
      () => this.images.set(uri, { state: "failed" }),
    );
    return null;
  }

  private async loadImage(uri: string): Promise<ImageBitmap> {
    let blob: Blob;
    if (uri.startsWith("pkg:/")) {
      const bytes = await this.readPackageFile(uri);
      if (!bytes) throw new Error(`missing ${uri}`);
      blob = new Blob([bytes]);
    } else if (/^https?:\/\//i.test(uri)) {
      // Direct fetch; servers without CORS fail until the local proxy lands (F2).
      const response = await fetch(uri, { credentials: "omit" });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      blob = await response.blob();
    } else {
      throw new Error(`unsupported uri ${uri}`);
    }
    return createImageBitmap(blob);
  }

  private draw(): void {
    const list = this.list;
    const context = this.context;
    if (!list) return;
    const scaleX = this.canvas.width / list.w;
    const scaleY = this.canvas.height / list.h;

    context.save();
    context.setTransform(scaleX, 0, 0, scaleY, 0, 0);
    context.fillStyle = "#000";
    context.fillRect(0, 0, list.w, list.h);
    context.fillStyle = rgbaToCss(list.bg);
    context.fillRect(0, 0, list.w, list.h);

    for (const item of list.items) {
      switch (item[0]) {
        case "r": {
          const [, x, y, w, h, color, opacity] = item;
          context.fillStyle = rgbaToCss(color, opacity);
          context.fillRect(x, y, w, h);
          break;
        }
        case "i": {
          const [, x, y, w, h, uri, opacity] = item;
          const bitmap = this.image(uri);
          if (!bitmap) break;
          // Width/height 0 means "bitmap size" on the device.
          const width = w > 0 ? w : bitmap.width;
          const height = h > 0 ? h : bitmap.height;
          context.globalAlpha = opacity;
          context.drawImage(bitmap, x, y, width, height);
          context.globalAlpha = 1;
          break;
        }
        case "t":
          this.drawText(item);
          break;
        case "c": {
          const [, x, y, w, h] = item;
          context.save();
          context.beginPath();
          context.rect(x, y, w, h);
          context.clip();
          break;
        }
        case "C":
          context.restore();
          break;
      }
    }
    context.restore();
  }

  private drawText(item: Extract<DisplayList["items"][number], { 0: "t" }>): void {
    const [, x, y, w, h, text, size, bold, color, opacity, horizAlign, vertAlign, wrap, maxLines] =
      item;
    if (!text) return;
    const context = this.context;
    context.font = `${bold ? "bold " : ""}${size}px ${FONT_FAMILY}`;
    context.fillStyle = rgbaToCss(color, opacity);
    context.textBaseline = "top";
    const lineHeight = Math.round((size * 46) / 45);
    const lines = layoutLines(text, w, wrap, maxLines, (value) => context.measureText(value).width);
    const blockHeight = lines.length * lineHeight;
    let top = y;
    if (vertAlign === "center") top = y + (h - blockHeight) / 2;
    else if (vertAlign === "bottom") top = y + h - blockHeight;

    context.save();
    context.beginPath();
    context.rect(x, y, w, Math.max(h, blockHeight));
    context.clip();
    lines.forEach((line, index) => {
      let left = x;
      if (horizAlign === "center") {
        context.textAlign = "center";
        left = x + w / 2;
      } else if (horizAlign === "right") {
        context.textAlign = "right";
        left = x + w;
      } else {
        context.textAlign = "left";
      }
      // Center glyphs vertically in the line box, like the device's metrics.
      context.fillText(line, left, top + index * lineHeight + (lineHeight - size) / 2);
    });
    context.restore();
  }
}
