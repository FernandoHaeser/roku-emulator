/** Debounces source edits and serializes package/deploy work. */
export class HotReload {
  private timer: ReturnType<typeof setTimeout> | undefined;
  private dirty = false;
  private running = false;
  private disposed = false;
  private readonly controller = new AbortController();
  constructor(
    private readonly deploy: (signal: AbortSignal) => Promise<void>,
    private readonly onError: (error: unknown) => void,
    private readonly delayMs = 400,
  ) {}
  notify() {
    if (this.disposed) return;
    this.dirty = true;
    if (this.running) return;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.run(), this.delayMs);
    this.timer.unref?.();
  }
  private async run() {
    if (this.disposed || this.running || !this.dirty) return;
    this.running = true;
    this.dirty = false;
    try {
      await this.deploy(this.controller.signal);
    } catch (error) {
      if (!this.disposed) this.onError(error);
    } finally {
      this.running = false;
      if (this.dirty) this.notify();
    }
  }
  dispose() {
    this.disposed = true;
    this.dirty = false;
    clearTimeout(this.timer);
    this.controller.abort();
  }
}
