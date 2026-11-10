import type { SceneNode } from "@roku-emu/protocol";
import { Icon } from "@/components/Icon";
import { styles } from "./styles";

interface NodeTreeItemProps {
  node: SceneNode;
  /** Child indices from the root, joined with "/" (stable key per position). */
  path: string;
  depth: number;
  selectedPath: string | null;
  expanded: ReadonlySet<string>;
  /** Paths that match the filter (or contain a match); null = no filter. */
  visible: ReadonlySet<string> | null;
  onToggle: (path: string) => void;
  onSelect: (path: string) => void;
}

/** Node label: type plus `id` when set, like the device's app-ui listing. */
export function nodeLabel(node: SceneNode): { type: string; id: string } {
  const id = node.fields.id;
  return { type: node.type, id: typeof id === "string" ? id : "" };
}

export function NodeTreeItem(props: NodeTreeItemProps) {
  const { node, path, depth, selectedPath, expanded, visible, onToggle, onSelect } = props;
  if (visible && !visible.has(path)) return null;
  const hasChildren = node.children.length > 0;
  // A filter opens every branch that leads to a match.
  const open = hasChildren && (visible !== null || expanded.has(path));
  const selected = path === selectedPath;
  const { type, id } = nodeLabel(node);

  return (
    <li
      role="treeitem"
      aria-selected={selected}
      aria-expanded={hasChildren ? open : undefined}
      aria-level={depth + 1}
    >
      <div
        className={`${styles.row.base} ${selected ? styles.row.selected : styles.row.idle}`}
        style={{ paddingLeft: `${depth * 12 + 4}px` }}
      >
        {hasChildren ? (
          <button
            type="button"
            className={styles.toggle}
            aria-label={open ? "Recolher" : "Expandir"}
            onClick={() => onToggle(path)}
          >
            <Icon name={open ? "chevronDown" : "chevronRight"} />
          </button>
        ) : (
          <span className={styles.spacer} />
        )}
        <button type="button" className={styles.label} onClick={() => onSelect(path)}>
          <span className={styles.type}>{type}</span>
          {id && <span className={styles.id}>{id}</span>}
          {node.focused && <span className={styles.focus}>foco</span>}
        </button>
      </div>
      {open && (
        <ul role="group" className={styles.group}>
          {node.children.map((child, index) => (
            <NodeTreeItem
              key={`${path}/${index}`}
              {...props}
              node={child}
              path={`${path}/${index}`}
              depth={depth + 1}
            />
          ))}
        </ul>
      )}
    </li>
  );
}
