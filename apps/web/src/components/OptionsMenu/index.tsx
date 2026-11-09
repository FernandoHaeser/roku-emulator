import { Icon, type IconName } from "@/components/Icon";
import { styles } from "./styles";

export interface OptionsMenuItem {
  id: string;
  label: string;
  icon: IconName;
}

interface OptionsMenuProps {
  title: string;
  items: readonly OptionsMenuItem[];
  focusedIndex: number;
  onSelect: (item: OptionsMenuItem) => void;
}

/**
 * Side panel opened with the Options key on a tile. Focus is driven by the
 * parent (remote Up/Down/OK/Back); clicks select directly.
 */
export function OptionsMenu({ title, items, focusedIndex, onSelect }: OptionsMenuProps) {
  return (
    <div className={styles.backdrop}>
      <div className={styles.panel} role="menu" aria-label={title}>
        <p className={styles.title}>{title}</p>
        {items.map((item, index) => (
          <button
            key={item.id}
            type="button"
            role="menuitem"
            tabIndex={-1}
            className={`${styles.item.base} ${index === focusedIndex ? styles.item.focused : ""}`}
            onClick={() => onSelect(item)}
          >
            <Icon name={item.icon} size="screen" />
            {item.label}
          </button>
        ))}
      </div>
    </div>
  );
}
