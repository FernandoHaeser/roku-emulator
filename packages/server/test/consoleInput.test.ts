import { expect, it } from "vitest";
import { ConsoleInput } from "../src/consoleInput";

it("frames split UTF-8, CRLF and Ctrl+C without extra commands", () => {
  const commands: string[] = [];
  const input = new ConsoleInput((command) => commands.push(command));
  const bytes = Buffer.from("print café\r\nvar\r\n\x03;\r\nc\n");
  for (const byte of bytes) input.push(Buffer.from([byte]));
  expect(commands).toEqual(["print café", "var", "\x03", "c"]);
});

it("drops oversized lines and telnet option negotiation, then recovers", () => {
  const commands: string[] = [];
  const input = new ConsoleInput((command) => commands.push(command));
  input.push(Buffer.from([255, 253]));
  input.push(Buffer.from([1]));
  input.push(Buffer.from(`${"x".repeat(4097)}\nbt\n`));
  expect(commands).toEqual(["bt"]);
});
