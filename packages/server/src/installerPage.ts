/**
 * HTML answers of the developer installer. The device reports results with
 * `Shell.create('Roku.Message')` calls; roku-deploy and our oracle parse them.
 */

export type InstallerMessageType = "success" | "error" | "info";

export interface InstallerMessage {
  type: InstallerMessageType;
  text: string;
}

/** Escapes text for a single-quoted JS string inside an HTML script. */
function jsString(text: string): string {
  return text.replace(
    /[\\'"<>&\n\r]/g,
    (char) => `\\u${char.charCodeAt(0).toString(16).padStart(4, "0")}`,
  );
}

export function installerPage(messages: readonly InstallerMessage[]): string {
  const triggers = messages
    .map(
      (message) =>
        `      Shell.create('Roku.Message').trigger('Set message type', '${message.type}').trigger('Set message content', '${jsString(message.text)}').trigger('Render', node);`,
    )
    .join("\n");
  return `<!DOCTYPE html>
<html>
  <head><meta charset="utf-8"><title>Roku Emulator - Development Application Installer</title></head>
  <body>
    <div id="root"></div>
    <script type="text/javascript">
      var node = document.getElementById('root');
${triggers}
    </script>
  </body>
</html>
`;
}
