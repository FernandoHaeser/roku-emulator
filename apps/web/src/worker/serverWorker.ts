/** Worker-shaped transport for the runtime owned by the VS Code/Node host. */
import {
  BRIDGE_PATH,
  BRIDGE_HEADER,
  type ShellToWorkerMessage,
  type WorkerToShellMessage,
} from "@roku-emu/protocol";
export class ServerWorker {
  onmessage: ((event: MessageEvent<WorkerToShellMessage>) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  private readonly events = new EventSource(`${BRIDGE_PATH}/events`);
  private queue = Promise.resolve();
  constructor() {
    this.events.onmessage = (event) => {
      const value = JSON.parse(event.data) as { type: string; message: WorkerToShellMessage };
      if (value.type === "runtime")
        this.onmessage?.({ data: value.message } as MessageEvent<WorkerToShellMessage>);
    };
  }
  postMessage(message: ShellToWorkerMessage, _transfer: Transferable[] = []) {
    if (message.type === "init") return;
    this.queue = this.queue
      .then(async () => {
        let body: BodyInit;
        let metadata: unknown = message;
        if ("zip" in message) {
          const { zip, ...rest } = message;
          metadata = rest;
          body = zip;
        } else body = "";
        const response = await fetch(`${BRIDGE_PATH}/runtime`, {
          method: "POST",
          headers: {
            [BRIDGE_HEADER]: "1",
            "x-roku-emu-message": encodeURIComponent(JSON.stringify(metadata)),
          },
          body,
        });
        if (!response.ok) throw new Error(`Runtime transport: ${response.status}`);
      })
      .catch((error: unknown) => this.onerror?.({ message: String(error) } as ErrorEvent));
  }
  terminate() {
    this.events.close();
  }
}
