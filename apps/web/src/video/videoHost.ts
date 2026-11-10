/**
 * Video planes of the web shell. The engine's Video nodes run in external
 * mode (crates/brs-core/src/sg/video.rs): the worker forwards their
 * commands, this host plays them in <video> elements under the channel
 * canvas (which clears those areas) and reports player events back.
 *
 * Progressive files (MP4, package media as blob URLs) play natively; HLS and
 * DASH go through Shaka Player with networking over the shell's HTTP proxy.
 * Protected contents pass their license server to Shaka (EME); a browser
 * without that CDM (e.g. open-source Chromium has no Widevine) reports
 * `unsupported` and the engine keeps the event sequence on its own clock.
 */
import type { VideoCommand, VideoDrm, VideoEvent, VideoRect } from "@roku-emu/protocol";
import muxjs from "mux.js";
import shaka from "shaka-player/dist/shaka-player.compiled.js";
import { proxyScheme } from "./proxyScheme";

let shakaReady = false;

function setupShaka(): void {
  if (shakaReady) return;
  shakaReady = true;
  // Shaka transmuxes MPEG-TS HLS segments with mux.js, found on window.
  (window as unknown as { muxjs: unknown }).muxjs = muxjs;
  shaka.polyfill.installAll();
  for (const scheme of ["http", "https"])
    shaka.net.NetworkingEngine.registerScheme(
      scheme,
      proxyScheme,
      shaka.net.NetworkingEngine.PluginPriority.APPLICATION,
    );
}

/** Adaptive streams need Shaka; everything else is a progressive file. */
export function isAdaptive(url: string, format: string): boolean {
  const kind = format.toLowerCase();
  if (kind === "hls" || kind === "dash" || kind === "smooth") return true;
  if (kind === "mp4" || kind === "mkv" || kind === "mp3") return false;
  const path = url.split(/[?#]/)[0]!.toLowerCase();
  return path.endsWith(".m3u8") || path.endsWith(".mpd");
}

/** EME key system for a Roku `drmParams.keySystem`. */
export function emeKeySystem(keySystem: string): string | null {
  switch (keySystem.toLowerCase()) {
    case "widevine":
      return "com.widevine.alpha";
    case "playready":
      return "com.microsoft.playready";
    default:
      return null;
  }
}

interface ShakaErrorLike {
  category?: number;
  code?: number;
  data?: unknown[];
}

/** HTTP status from a Shaka error (BAD_HTTP_STATUS carries it in data[1]). */
function errorStatus(error: unknown): number {
  const status = (error as ShakaErrorLike).data?.[1];
  return typeof status === "number" ? status : -1;
}

/** Maps a Shaka load error to a player event. */
export function shakaErrorEvent(
  id: number,
  error: unknown,
): Extract<VideoEvent, { type: "error" | "unsupported" }> {
  const { category, code, data } = error as ShakaErrorLike;
  const Code = shaka.util.Error.Code;
  if (
    code === Code.REQUESTED_KEY_SYSTEM_CONFIG_UNAVAILABLE ||
    code === Code.NO_RECOGNIZED_KEY_SYSTEMS
  )
    return { id, type: "unsupported" };
  const message = String((error as Error).message ?? error);
  if (category === shaka.util.Error.Category.DRM) {
    // LICENSE_REQUEST_FAILED wraps the network error of the license request.
    const status = code === Code.LICENSE_REQUEST_FAILED ? errorStatus(data?.[0]) : -1;
    return { id, type: "error", status, message, category: "drm" };
  }
  return { id, type: "error", status: errorStatus(error), message };
}

class Player {
  readonly element: HTMLVideoElement;
  private shakaPlayer: shaka.Player | null = null;
  private finished = false;

  constructor(
    readonly id: number,
    private readonly emit: (event: VideoEvent) => void,
  ) {
    const video = document.createElement("video");
    video.playsInline = true;
    video.style.position = "absolute";
    video.style.objectFit = "contain";
    video.style.background = "black";
    video.style.display = "none";
    video.addEventListener("waiting", () => this.send({ id, type: "buffering" }));
    video.addEventListener("playing", () => this.send({ id, type: "playing" }));
    video.addEventListener("seeked", () => {
      if (!video.paused) this.send({ id, type: "playing" });
    });
    video.addEventListener("durationchange", () => {
      if (Number.isFinite(video.duration))
        this.send({ id, type: "duration", value: video.duration });
    });
    video.addEventListener("timeupdate", () =>
      this.send({ id, type: "position", value: video.currentTime }),
    );
    video.addEventListener("ended", () => {
      this.send({ id, type: "ended" });
      this.finished = true;
    });
    video.addEventListener("error", () => {
      // Native playback errors carry no HTTP status.
      if (!this.shakaPlayer)
        this.fail({
          id,
          type: "error",
          status: -1,
          message: video.error?.message ?? "media error",
        });
    });
    this.element = video;
  }

  private send(event: VideoEvent): void {
    if (!this.finished) this.emit(event);
  }

  private fail(event: Extract<VideoEvent, { type: "error" }>): void {
    this.send(event);
    this.finished = true;
  }

  /** No CDM: the engine takes over the timing; the plane stays black. */
  private unsupported(): void {
    this.send({ id: this.id, type: "unsupported" });
    this.finished = true;
    void this.shakaPlayer?.destroy();
    this.shakaPlayer = null;
  }

  async load(url: string, format: string, drm?: VideoDrm): Promise<void> {
    const video = this.element;
    if (drm || isAdaptive(url, format)) {
      setupShaka();
      this.shakaPlayer = new shaka.Player();
      const keySystem = drm ? emeKeySystem(drm.keySystem) : null;
      if (drm && !keySystem) {
        this.unsupported();
        return;
      }
      if (drm && keySystem)
        this.shakaPlayer.configure({ drm: { servers: { [keySystem]: drm.licenseServerURL } } });
      try {
        await this.shakaPlayer.attach(video);
        await this.shakaPlayer.load(url);
      } catch (error) {
        const event = shakaErrorEvent(this.id, error);
        if (event.type === "unsupported") this.unsupported();
        else this.fail(event);
        return;
      }
    } else {
      video.src = url;
    }
    await this.play();
  }

  async play(): Promise<void> {
    try {
      await this.element.play();
    } catch (error) {
      // Autoplay policies block audible playback without a user gesture;
      // a TV has none, so retry muted rather than stalling.
      if ((error as DOMException).name === "NotAllowedError" && !this.element.muted) {
        this.element.muted = true;
        await this.element.play().catch(() => undefined);
      }
    }
  }

  command(command: Exclude<VideoCommand, { op: "reset" }>): void {
    switch (command.op) {
      case "load":
        void this.load(command.url, command.format, command.drm);
        break;
      case "pause":
        this.element.pause();
        break;
      case "resume":
        void this.play();
        break;
      case "seek":
        this.element.currentTime = command.position;
        break;
      case "stop":
        this.destroy();
        break;
    }
  }

  destroy(): void {
    this.finished = true;
    void this.shakaPlayer?.destroy();
    this.shakaPlayer = null;
    this.element.pause();
    this.element.removeAttribute("src");
    this.element.load();
    this.element.remove();
  }
}

export class VideoHost {
  private readonly players = new Map<number, Player>();
  private layout: { width: number; height: number; rects: VideoRect[] } = {
    width: 0,
    height: 0,
    rects: [],
  };

  constructor(
    private readonly container: HTMLElement,
    private readonly emit: (event: VideoEvent) => void,
  ) {}

  command(command: VideoCommand): void {
    if (command.op === "reset") {
      this.reset();
      return;
    }
    let player = this.players.get(command.id);
    if (command.op === "load") {
      player?.destroy();
      player = new Player(command.id, this.emit);
      this.players.set(command.id, player);
      this.container.append(player.element);
      this.place(player);
    }
    player?.command(command);
    if (command.op === "stop") this.players.delete(command.id);
  }

  setLayout(width: number, height: number, rects: VideoRect[]): void {
    this.layout = { width, height, rects };
    for (const player of this.players.values()) this.place(player);
  }

  /** Positions a player's element over its plane (percent of the screen). */
  private place(player: Player): void {
    const { width, height, rects } = this.layout;
    const rect = rects.find((r) => r.id === player.id);
    const style = player.element.style;
    if (!rect || width <= 0 || height <= 0) {
      style.display = "none";
      return;
    }
    style.display = "block";
    style.left = `${(rect.x / width) * 100}%`;
    style.top = `${(rect.y / height) * 100}%`;
    style.width = `${(rect.w / width) * 100}%`;
    style.height = `${(rect.h / height) * 100}%`;
  }

  reset(): void {
    for (const player of this.players.values()) player.destroy();
    this.players.clear();
  }
}
