import { useId, useState, type ReactNode } from "react";
import { styles } from "./styles";

export interface SidePanelTab {
  id: string;
  label: string;
  /** Receives whether the tab is visible (e.g. to pause polling). */
  render: (active: boolean) => ReactNode;
}

interface SidePanelProps {
  tabs: SidePanelTab[];
}

/**
 * Tabbed tool panel. Every tab stays mounted (hidden when inactive) so the
 * console keeps its history while another tool is open.
 */
export function SidePanel({ tabs }: SidePanelProps) {
  const [activeId, setActiveId] = useState(tabs[0]?.id ?? "");
  const baseId = useId();

  return (
    <section className={styles.panel}>
      <div role="tablist" aria-label="Ferramentas" className={styles.tabList}>
        {tabs.map((tab) => {
          const active = tab.id === activeId;
          return (
            <button
              key={tab.id}
              type="button"
              role="tab"
              id={`${baseId}-tab-${tab.id}`}
              aria-selected={active}
              aria-controls={`${baseId}-panel-${tab.id}`}
              className={`${styles.tab.base} ${active ? styles.tab.active : styles.tab.idle}`}
              onClick={() => setActiveId(tab.id)}
            >
              {tab.label}
            </button>
          );
        })}
      </div>
      {tabs.map((tab) => (
        <div
          key={tab.id}
          role="tabpanel"
          id={`${baseId}-panel-${tab.id}`}
          aria-labelledby={`${baseId}-tab-${tab.id}`}
          hidden={tab.id !== activeId}
          className={styles.tabPanel}
        >
          {tab.render(tab.id === activeId)}
        </div>
      ))}
    </section>
  );
}
