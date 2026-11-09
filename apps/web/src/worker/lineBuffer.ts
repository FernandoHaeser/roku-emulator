/**
 * Splits streamed console text into complete lines. The engine writes PRINT
 * output in pieces (items, padding, newline); the console shows whole lines.
 */
export class LineBuffer {
  private partial = "";

  /** Appends text and returns the lines it completed. */
  push(text: string): string[] {
    const parts = (this.partial + text).split("\n");
    this.partial = parts.pop() ?? "";
    return parts;
  }

  /** Returns the unterminated tail, if any (e.g. the debugger prompt). */
  flush(): string[] {
    const tail = this.partial;
    this.partial = "";
    return tail === "" ? [] : [tail];
  }
}
