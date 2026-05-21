import type { TabInfo } from "../../types/editor";

interface Props {
  tabs: TabInfo[];
  activeTabId: string | null;
  onSwitchTab: (id: string) => void;
  onCloseTab: (id: string) => void;
}

export default function TabBar({ tabs, activeTabId, onSwitchTab, onCloseTab }: Props) {
  if (tabs.length === 0) return null;

  return (
    <div className="tabbar">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          className={`tabbar-tab${tab.id === activeTabId ? " tabbar-tab-active" : ""}`}
          onClick={() => onSwitchTab(tab.id)}
        >
          <span className="tabbar-tab-name">
            {tab.dirty ? "\u2022 " : ""}
            {tab.file_name}
          </span>
          <span
            className="tabbar-tab-close"
            onClick={(e) => {
              e.stopPropagation();
              onCloseTab(tab.id);
            }}
          >
            &times;
          </span>
        </div>
      ))}
    </div>
  );
}
