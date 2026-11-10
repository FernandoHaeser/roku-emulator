import type { SceneInspection, SceneNode } from "@roku-emu/protocol";

export type { SceneInspection, SceneNode };
const escape = (text: unknown) =>
  String(text ?? "").replace(/[<>&"']/g, (c) => `&#${c.charCodeAt(0)};`);
export function appUiXml(scene: SceneInspection, title: string): string {
  const node = (value: SceneNode, index: number): string => {
    const attrs: Record<string, unknown> = {
      rcid: 0,
      bounds: `{${value.bounds.join(", ")}}`,
      index,
    };
    for (const [key, name] of [
      ["id", "name"],
      ["text", "text"],
      ["uri", "uri"],
    ] as const)
      if (value.fields[key]) attrs[name] = value.fields[key];
    if (value.focused) attrs.focused = true;
    const attributes = Object.entries(attrs)
      .map(([k, v]) => `${k}="${escape(v)}"`)
      .join(" ");
    return `<${escape(value.type)} ${attributes}>${value.children.map(node).join("")}</${escape(value.type)}>`;
  };
  return `<?xml version="1.0" encoding="UTF-8"?><app-ui><status>OK</status><topscreen><plugin id="dev" name="${escape(title)}"/><screen type="SGScreen" focused="true">${scene.roots.map(node).join("")}</screen></topscreen></app-ui>`;
}
export function sceneNodes(scene: SceneInspection): SceneNode[] {
  const nodes: SceneNode[] = [];
  const visit = (n: SceneNode) => {
    nodes.push(n);
    for (const child of n.children) visit(child);
  };
  for (const root of scene.roots) visit(root);
  return nodes;
}
