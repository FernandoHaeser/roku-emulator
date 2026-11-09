/** Bounded TCP command framing. CRLF may span chunks; Ctrl+C interrupts now. */
import { StringDecoder } from "node:string_decoder";

export class ConsoleInput {
  private readonly decoder = new StringDecoder("utf8");
  private pending = "";
  private discard = false;
  private afterCr = false;
  private afterInterrupt = false;
  private telnet = 0;

  constructor(private readonly command: (text: string) => void) {}

  push(chunk: Buffer): void {
    const text: number[] = [];
    for (const byte of chunk) {
      if (this.telnet) {
        this.telnet = this.telnet === 1 && byte >= 251 && byte <= 254 ? 2 : 0;
      } else if (byte === 255) this.telnet = 1;
      else text.push(byte);
    }
    for (const char of this.decoder.write(Buffer.from(text))) {
      if (char === "\x03") {
        this.pending = "";
        this.discard = false;
        this.afterInterrupt = true;
        this.command("\x03");
        continue;
      }
      // RokuCommunity sends Ctrl+C followed by ';' and a newline.
      if (this.afterInterrupt && char === ";") continue;
      if (char === "\r" || char === "\n") {
        if (!(char === "\n" && this.afterCr) && !this.discard && !this.afterInterrupt) {
          this.command(this.pending);
        }
        this.pending = "";
        this.discard = false;
        this.afterInterrupt = false;
        this.afterCr = char === "\r";
        continue;
      }
      this.afterCr = false;
      this.afterInterrupt = false;
      if (!this.discard) {
        this.pending += char;
        if (this.pending.length > 4096) {
          this.pending = "";
          this.discard = true;
        }
      }
    }
  }
}
