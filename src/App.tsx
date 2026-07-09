import { useState } from "react";
import { LocationTree } from "./components/LocationTree";
import { ShelfView } from "./components/ShelfView";
import { CaptureMode } from "./components/CaptureMode";
import { SearchView } from "./components/SearchView";
import { StatsView } from "./components/StatsView";
import { Icon } from "./components/Icon";
import "./App.css";

type Tab = "locations" | "shelf" | "capture" | "search" | "stats";

const NAV: { id: Tab; label: string; icon: Parameters<typeof Icon>[0]["name"] }[] = [
  { id: "locations", label: "Локации", icon: "locations" },
  { id: "shelf", label: "Полка", icon: "shelf" },
  { id: "capture", label: "Капчур", icon: "capture" },
  { id: "search", label: "Поиск", icon: "search" },
  { id: "stats", label: "Статистика", icon: "stats" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("locations");
  const [activeShelf, setActiveShelf] = useState<number | null>(null);

  const openShelf = (id: number) => {
    setActiveShelf(id);
    setTab("shelf");
  };

  return (
    <div className="app">
      <nav className="rail">
        <div className="rail__brand">
          <span className="rail__glyph"><Icon name="book" size={18} /></span>
          <span className="rail__word">Книж<b>ник</b></span>
        </div>
        <div className="rail__nav">
          {NAV.map((n) => (
            <button
              key={n.id}
              className={`rail__item${tab === n.id ? " is-active" : ""}`}
              onClick={() => setTab(n.id)}
              disabled={n.id === "shelf" && !activeShelf}
            >
              <Icon name={n.icon} />
              <span>{n.label}</span>
            </button>
          ))}
        </div>
        <div className="rail__foot">Домашняя библиотека · офлайн</div>
      </nav>

      <main className="main">
        <div className="view" key={tab === "shelf" ? `shelf-${activeShelf}` : tab}>
          {tab === "locations" && <LocationTree onOpenShelf={openShelf} />}
          {tab === "shelf" && activeShelf && <ShelfView shelfId={activeShelf} />}
          {tab === "capture" && <CaptureMode />}
          {tab === "search" && <SearchView onOpenShelf={openShelf} />}
          {tab === "stats" && <StatsView />}
        </div>
      </main>
    </div>
  );
}
