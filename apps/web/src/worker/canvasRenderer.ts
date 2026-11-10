/**
 * Canvas2D renderer for the engine's display list. Draws in design
 * coordinates scaled to the surface (the device also scales the channel's
 * ui_resolution to the output mode).
 *
 * Fidelity: rectangle edges and text metrics follow device measurements
 * (fidelity/scenegraph/render-rects-*, label-metrics, font-*). Glyph shapes
 * come from a substitute font; Poster blendColor and image filtering are
 * still approximations.
 */
import { parseDisplayList, rgbaToCss, type DisplayList } from "./displayList";
import { SUBSTITUTE_FAMILY } from "./substituteFont";

/** Absorbs float error in design * scale products (102 * 2/3 must stay 68). */
const SNAP_EPSILON = 1e-4;

/** Device edge rule for filled boxes: floor the scaled start and the scaled size. */
export function snapToOutputPixels(
  x: number,
  y: number,
  w: number,
  h: number,
  scaleX: number,
  scaleY: number,
): { x: number; y: number; width: number; height: number } {
  const snap = (value: number) => Math.floor(value + SNAP_EPSILON);
  return {
    x: snap(x * scaleX),
    y: snap(y * scaleY),
    width: Math.max(0, snap(w * scaleX)),
    height: Math.max(0, snap(h * scaleY)),
  };
}

/** Bundled substitute first; the generic stack only covers a failed load. */
const FONT_FAMILY = `${SUBSTITUTE_FAMILY}, system-ui, sans-serif`;

/**
 * Substitute glyph calibration against device screenshots
 * (fidelity/scenegraph/label-metrics): stroke weights matching the device's
 * ink density, and the glyph top offset in output pixels.
 */
const TEXT_WEIGHT = { regular: 350, bold: 570 };
const GLYPH_TOP_OUTPUT_PX = -1;

type ImageEntry =
  { state: "loading" } | { state: "ready"; bitmap: ImageBitmap } | { state: "failed" };

export class CanvasRenderer {
  private readonly context: OffscreenCanvasRenderingContext2D;
  private fontGeneration = 0;
  private readonly packageFonts = new Map<string, string | null>();
  private readonly images = new Map<string, ImageEntry>();
  private list: DisplayList | null = null;
  private framePending = false;
  /** Output px per design px (vertical), set at the start of each frame. */
  private outputScaleY = 1;
  /** Open rotations; pixel snapping only applies to axis-aligned drawing. */
  private rotationDepth = 0;
  private readonly patches = new Map<string, [number, number, number, number]>();
  private readonly tinted = new Map<string, OffscreenCanvas>();

  /**
   * @param readPackageFile resolves `pkg:/` URIs from the running channel.
   */
  constructor(
    private readonly canvas: OffscreenCanvas,
    private readonly readPackageFile: (
      uri: string,
    ) => Uint8Array<ArrayBuffer> | null | Promise<Uint8Array<ArrayBuffer> | null>,
  ) {
    // Opaque contexts permit LCD subpixel text on Linux (colored fringes).
    // Use grayscale antialiasing across hosts; draw() still fills the whole
    // surface with the scene background, so channel output remains opaque.
    const context = canvas.getContext("2d", { alpha: true });
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
    this.fontGeneration++;
    this.packageFonts.clear();
    this.tinted.clear();
    this.patches.clear();
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
    this.outputScaleY = scaleY;

    context.save();
    context.setTransform(scaleX, 0, 0, scaleY, 0, 0);
    context.fillStyle = "#000";
    context.fillRect(0, 0, list.w, list.h);
    context.fillStyle = rgbaToCss(list.bg);
    context.fillRect(0, 0, list.w, list.h);

    this.rotationDepth = 0;
    for (const item of list.items) {
      switch (item[0]) {
        case "r": {
          const [, x, y, w, h, color, opacity] = item;
          context.fillStyle = rgbaToCss(color, opacity);
          // The device fills whole output pixels without antialiasing:
          // start = floor(x * scale), size = floor(w * scale)
          // (fidelity/scenegraph/render-rects-{fhd,hd}).
          if (this.rotationDepth > 0) {
            context.fillRect(x, y, w, h);
            break;
          }
          const box = snapToOutputPixels(x, y, w, h, scaleX, scaleY);
          context.save();
          context.setTransform(1, 0, 0, 1, 0, 0);
          context.fillRect(box.x, box.y, box.width, box.height);
          context.restore();
          break;
        }
        case "i": {
          const [, x, y, w, h, uri, opacity, blend] = item;
          const bitmap = this.image(uri);
          if (!bitmap) break;
          const source = this.blend(uri, bitmap, blend);
          // The engine sends the bitmap size (design px) when width/height are 0;
          // 0 here only means the load result has not reached the engine yet.
          const width = w > 0 ? w : bitmap.width / scaleX;
          const height = h > 0 ? h : bitmap.height / scaleY;
          context.save();
          context.globalAlpha = opacity;
          if (this.rotationDepth > 0) {
            this.drawImage(uri, source, bitmap, x, y, width, height, scaleX, scaleY);
          } else {
            const box = snapToOutputPixels(x, y, width, height, scaleX, scaleY);
            context.setTransform(1, 0, 0, 1, 0, 0);
            this.drawImage(uri, source, bitmap, box.x, box.y, box.width, box.height, 1, 1);
          }
          context.restore();
          break;
        }
        case "R": {
          const [, pivotX, pivotY, radians] = item;
          context.save();
          context.translate(pivotX, pivotY);
          // SceneGraph rotation is counterclockwise; canvas y points down.
          context.rotate(-radians);
          context.translate(-pivotX, -pivotY);
          this.rotationDepth += 1;
          break;
        }
        case "Q":
          context.restore();
          this.rotationDepth -= 1;
          break;
        case "t":
          this.drawText(item);
          break;
        case "v": {
          // Punch through to the video plane under the canvas (the shell
          // positions the player there).
          const [, x, y, w, h] = item;
          context.clearRect(x, y, w, h);
          break;
        }
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

  /**
   * Poster blendColor: the device multiplies each texel by the color
   * (fidelity/scenegraph/render-composite). White (the default) is a no-op.
   */
  private blend(uri: string, bitmap: ImageBitmap, rgba: number): CanvasImageSource {
    const unsigned = rgba >>> 0;
    if ((unsigned | 0xff) >>> 0 === 0xffffffff) return bitmap;
    const key = `${uri}|${unsigned}`;
    const cached = this.tinted.get(key);
    if (cached) return cached;
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d")!;
    context.drawImage(bitmap, 0, 0);
    context.globalCompositeOperation = "multiply";
    context.fillStyle = rgbaToCss(unsigned | 0xff);
    context.fillRect(0, 0, bitmap.width, bitmap.height);
    // Multiply fills transparent texels too; restore the bitmap's alpha.
    context.globalCompositeOperation = "destination-in";
    context.drawImage(bitmap, 0, 0);
    this.tinted.set(key, canvas);
    return canvas;
  }

  private drawImage(
    uri: string,
    source: CanvasImageSource,
    bitmap: ImageBitmap,
    x: number,
    y: number,
    width: number,
    height: number,
    scaleX: number,
    scaleY: number,
  ): void {
    const context = this.context;
    if (
      !uri.split(/[?#]/)[0]!.toLowerCase().endsWith(".9.png") ||
      bitmap.width < 3 ||
      bitmap.height < 3
    ) {
      context.drawImage(source, x, y, width, height);
      return;
    }
    const sw = bitmap.width - 2,
      sh = bitmap.height - 2;
    if (width * scaleX < sw || height * scaleY < sh) {
      context.drawImage(source, 1, 1, sw, sh, x, y, width, height);
      return;
    }
    let patch = this.patches.get(uri);
    if (!patch) {
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const pixels = canvas.getContext("2d")!;
      pixels.drawImage(bitmap, 0, 0);
      const data = pixels.getImageData(0, 0, bitmap.width, bitmap.height).data;
      const marked = (px: number, py: number) => {
        const at = (py * bitmap.width + px) * 4;
        return data[at + 3]! > 127 && data[at]! < 32 && data[at + 1]! < 32 && data[at + 2]! < 32;
      };
      const range = (length: number, horizontal: boolean): [number, number] => {
        const marks = Array.from({ length }, (_, i) => i).filter((i) =>
          horizontal ? marked(i + 1, 0) : marked(0, i + 1),
        );
        return marks.length ? [marks[0]!, marks[marks.length - 1]! + 1] : [0, length];
      };
      patch = [...range(sw, true), ...range(sh, false)];
      this.patches.set(uri, patch);
    }
    const [left, right, top, bottom] = patch;
    const sx = [0, left, right, sw],
      sy = [0, top, bottom, sh];
    const dx = [0, left / scaleX, width - (sw - right) / scaleX, width];
    const dy = [0, top / scaleY, height - (sh - bottom) / scaleY, height];
    for (let row = 0; row < 3; row++)
      for (let col = 0; col < 3; col++) {
        const sourceWidth = sx[col + 1]! - sx[col]!,
          sourceHeight = sy[row + 1]! - sy[row]!;
        if (sourceWidth > 0 && sourceHeight > 0)
          context.drawImage(
            source,
            1 + sx[col]!,
            1 + sy[row]!,
            sourceWidth,
            sourceHeight,
            x + dx[col]!,
            y + dy[row]!,
            dx[col + 1]! - dx[col]!,
            dy[row + 1]! - dy[row]!,
          );
      }
  }

  private fontFamily(uri: string): string {
    if (!uri) return FONT_FAMILY;
    const loaded = this.packageFonts.get(uri);
    if (loaded) return loaded;
    if (!this.packageFonts.has(uri)) {
      this.packageFonts.set(uri, null);
      const generation = this.fontGeneration;
      void Promise.resolve(this.readPackageFile(uri))
        .then(async (bytes) => {
          if (!bytes || typeof FontFace === "undefined") return;
          const family = `RokuPackage${generation}_${Math.random().toString(36).slice(2)}`;
          const face = await new FontFace(family, bytes).load();
          const fonts =
            (globalThis as unknown as { fonts?: FontFaceSet; document?: Document }).fonts ??
            globalThis.document?.fonts;
          fonts?.add(face);
          if (this.fontGeneration !== generation) return;
          this.packageFonts.set(uri, family);
          this.requestFrame();
        })
        .catch(() => {
          /* Invalid fonts retain the substitute. */
        });
    }
    return FONT_FAMILY;
  }

  private drawText(item: Extract<DisplayList["items"][number], { 0: "t" }>): void {
    const [
      ,
      x,
      y,
      w,
      h,
      lines,
      size,
      bold,
      color,
      opacity,
      horizAlign,
      vertAlign,
      lineBox,
      lineSpacing,
      fontUri,
      ascent,
    ] = item;
    if (!lines.length) return;
    const context = this.context;
    const family = this.fontFamily(fontUri);
    const packaged = !!fontUri && !!this.packageFonts.get(fontUri);
    context.font = `${packaged ? 400 : bold ? TEXT_WEIGHT.bold : TEXT_WEIGHT.regular} ${size}px ${family}`;
    context.fillStyle = rgbaToCss(color, opacity);
    context.textBaseline = packaged ? "alphabetic" : "top";
    context.textAlign = "left";
    const blockHeight = lines.length * lineBox + Math.max(0, lines.length - 1) * lineSpacing;
    let top = y;
    if (vertAlign === "center") top += (h - blockHeight) / 2;
    else if (vertAlign === "bottom") top += h - blockHeight;
    context.save();
    context.beginPath();
    context.rect(x, y, w, h);
    context.clip();
    lines.forEach(([line, deviceWidth], index) => {
      const drawnWidth = context.measureText(line).width;
      let left = x;
      if (horizAlign === "center") left += (w - deviceWidth) / 2;
      else if (horizAlign === "right") left += w - deviceWidth;
      const lineTop =
        top +
        index * (lineBox + lineSpacing) +
        (packaged ? ascent : (lineBox - size) / 2 + GLYPH_TOP_OUTPUT_PX / this.outputScaleY);
      context.save();
      context.translate(left, lineTop);
      if (drawnWidth > 0 && deviceWidth > 0) context.scale(deviceWidth / drawnWidth, 1);
      context.fillText(line, 0, 0);
      context.restore();
    });
    context.restore();
  }
}
