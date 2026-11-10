import { afterEach, expect, it, vi } from "vitest";
import { HotReload } from "../src/hotReload";
afterEach(() => vi.useRealTimers());
it("coalesces edits, serializes deployments and cancels pending work", async () => {
  vi.useFakeTimers();
  let finish: () => void = () => {};
  const deploy = vi.fn(
    (_signal: AbortSignal) =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const reload = new HotReload(deploy, vi.fn(), 50);
  reload.notify();
  reload.notify();
  await vi.advanceTimersByTimeAsync(50);
  expect(deploy).toHaveBeenCalledTimes(1);
  reload.notify();
  reload.notify();
  await vi.advanceTimersByTimeAsync(100);
  expect(deploy).toHaveBeenCalledTimes(1);
  finish();
  await vi.advanceTimersByTimeAsync(50);
  expect(deploy).toHaveBeenCalledTimes(2);
  reload.notify();
  reload.dispose();
  finish();
  await vi.advanceTimersByTimeAsync(100);
  expect(deploy).toHaveBeenCalledTimes(2);
  expect(deploy.mock.calls[0]![0].aborted).toBe(true);
});
it("reports deployment errors and accepts later changes", async () => {
  vi.useFakeTimers();
  const error = vi.fn();
  const deploy = vi
    .fn()
    .mockRejectedValueOnce(new Error("broken manifest"))
    .mockResolvedValue(undefined);
  const reload = new HotReload(deploy, error, 10);
  reload.notify();
  await vi.advanceTimersByTimeAsync(10);
  expect(error).toHaveBeenCalledTimes(1);
  reload.notify();
  await vi.advanceTimersByTimeAsync(10);
  expect(deploy).toHaveBeenCalledTimes(2);
  reload.dispose();
});
