import { useCallback, useEffect, useMemo, useState } from "react";
import type { SceneInspection, SceneNode } from "@roku-emu/protocol";
import { NodeTreeItem, nodeLabel } from "@/components/NodeTreeItem";
import { styles } from "./styles";

/** Bounds of the selected node, for the highlight drawn over the screen. */
export interface NodeHighlight {
  bounds: number[];
  width: number;
  height: number;
}

interface NodeInspectorProps {
  inspect: () => Promise<SceneInspection | null>;
  /** Polls only while the tab is visible. */
  active: boolean;
  onHighlight: (highlight: NodeHighlight | null) => void;
}

const POLL_MS = 1_000;
const MAX_VALUE_CHARS = 300;

function nodeAt(roots: SceneNode[], path: string): SceneNode | null {
  const [root, ...rest] = path.split("/").map(Number);
  let node = roots[root!] ?? null;
  for (const index of rest) node = node?.children[index] ?? null;
  return node;
}

/** Paths of nodes whose type or id contains `filter`, plus their ancestors. */
function visiblePaths(roots: SceneNode[], filter: string): Set<string> {
  const needle = filter.toLowerCase();
  const paths = new Set<string>();
  const visit = (node: SceneNode, path: string): boolean => {
    const { type, id } = nodeLabel(node);
    let match = type.toLowerCase().includes(needle) || id.toLowerCase().includes(needle);
    node.children.forEach((child, index) => {
      if (visit(child, `${path}/${index}`)) match = true;
    });
    if (match) paths.add(path);
    return match;
  };
  roots.forEach((root, index) => visit(root, String(index)));
  return paths;
}

function formatValue(value: unknown): string {
  const text = typeof value === "string" ? JSON.stringify(value) : (JSON.stringify(value) ?? "");
  return text.length > MAX_VALUE_CHARS ? `${text.slice(0, MAX_VALUE_CHARS)}…` : text;
}

/** Live SceneGraph tree of the running channel with the selected node's fields. */
export function NodeInspector({ inspect, active, onHighlight }: NodeInspectorProps) {
  const [scene, setScene] = useState<SceneInspection | null>(null);
  const [error, setError] = useState("");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(["0"]));
  const [filter, setFilter] = useState("");

  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    let timer = 0;
    const poll = async () => {
      try {
        const next = await inspect();
        if (cancelled) return;
        setScene(next);
        setError("");
      } catch (reason) {
        if (!cancelled) setError(String((reason as Error).message ?? reason));
      }
      if (!cancelled) timer = window.setTimeout(() => void poll(), POLL_MS);
    };
    void poll();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [active, inspect]);

  const roots = useMemo(() => scene?.roots ?? [], [scene]);
  const selected = selectedPath ? nodeAt(roots, selectedPath) : null;
  const visible = useMemo(
    () => (filter.trim() ? visiblePaths(roots, filter.trim()) : null),
    [roots, filter],
  );

  useEffect(() => {
    onHighlight(
      active && selected && scene
        ? { bounds: selected.bounds, width: scene.width ?? 1280, height: scene.height ?? 720 }
        : null,
    );
  }, [active, selected, scene, onHighlight]);

  const toggle = useCallback((path: string) => {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (!next.delete(path)) next.add(path);
      return next;
    });
  }, []);

  const fields = selected
    ? Object.entries(selected.fields).sort(([a], [b]) => a.localeCompare(b))
    : [];

  return (
    <div className={styles.inspector}>
      <div className={styles.toolbar}>
        <input
          type="search"
          className={styles.filter}
          placeholder="Filtrar por tipo ou id"
          aria-label="Filtrar nodes"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
        />
      </div>
      {error && <p className={styles.error}>Inspeção indisponível: {error}</p>}
      {roots.length === 0 ? (
        <p className={styles.empty}>Nenhuma cena em exibição.</p>
      ) : (
        <>
          <ul role="tree" aria-label="Árvore de nodes" className={styles.tree}>
            {roots.map((root, index) => (
              <NodeTreeItem
                key={index}
                node={root}
                path={String(index)}
                depth={0}
                selectedPath={selectedPath}
                expanded={expanded}
                visible={visible}
                onToggle={toggle}
                onSelect={setSelectedPath}
              />
            ))}
          </ul>
          <div className={styles.details} aria-label="Campos do node">
            {selected ? (
              <>
                <p className={styles.detailsTitle}>
                  {selected.ancestry.join(" › ")}
                  <span className={styles.bounds}>
                    bounds [{selected.bounds.map((n) => Math.round(n)).join(", ")}]
                  </span>
                </p>
                <table className={styles.table}>
                  <tbody>
                    {fields.map(([name, value]) => (
                      <tr key={name}>
                        <th scope="row" className={styles.fieldName}>
                          {name}
                        </th>
                        <td className={styles.fieldValue}>{formatValue(value)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </>
            ) : (
              <p className={styles.empty}>Selecione um node para ver os campos.</p>
            )}
          </div>
        </>
      )}
    </div>
  );
}
