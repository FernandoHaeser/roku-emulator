const api = acquireVsCodeApi();
const frame = document.querySelector("iframe");
for (const button of document.querySelectorAll("button[data-key]"))
  button.addEventListener("click", () =>
    api.postMessage({ type: "remote", key: button.dataset.key }),
  );
window.addEventListener("message", (event) => {
  if (event.source !== frame.contentWindow || event.origin !== new URL(frame.src).origin) return;
  if (event.data?.type === "roku-emulator-ready")
    api.postMessage({
      type: "ready",
      isolated: event.data.isolated,
      serverRuntime: event.data.serverRuntime,
    });
});
