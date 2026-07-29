import { useCallback, useEffect, useState } from "react";
import { ShelfView } from "./components/ShelfView";
import { ShelfPicker } from "./components/ShelfPicker";
import { UnshelvedView } from "./components/UnshelvedView";
import { CaptureMode } from "./components/CaptureMode";
import { SearchView } from "./components/SearchView";
import { AddBookView } from "./components/AddBookView";
import { StatsView } from "./components/StatsView";
import { SettingsView } from "./components/SettingsView";
import { Icon } from "./components/Icon";
import { api } from "./api";
import "./App.css";

type Tab =
  | "shelf"
  | "unshelved"
  | "capture"
  | "add"
  | "search"
  | "stats"
  | "settings";

const NAV: { id: Tab; label: string; icon: Parameters<typeof Icon>[0]["name"] }[] = [
  { id: "shelf", label: "Полка", icon: "shelf" },
  { id: "unshelved", label: "Без полки", icon: "book" },
  { id: "capture", label: "Сканирование", icon: "capture" },
  { id: "add", label: "Добавить", icon: "addbook" },
  { id: "search", label: "Поиск", icon: "search" },
  { id: "stats", label: "Статистика", icon: "stats" },
  { id: "settings", label: "Настройки", icon: "settings" },
];

// Выбранная полка держалась только в памяти, поэтому после каждого перезапуска
// вкладка «Полка» снова оказывалась недоступной. Запоминаем между сеансами.
const LAST_SHELF = "knizhnik.lastShelf";

function readLastShelf(): number | null {
  const raw = localStorage.getItem(LAST_SHELF);
  const id = raw ? Number(raw) : NaN;
  return Number.isFinite(id) && id > 0 ? id : null;
}

export default function App() {
  const [tab, setTab] = useState<Tab>("shelf");
  const [activeShelf, setActiveShelf] = useState<number | null>(readLastShelf);
  const [unshelved, setUnshelved] = useState(0);

  const remember = useCallback((id: number) => {
    setActiveShelf(id);
    localStorage.setItem(LAST_SHELF, String(id));
  }, []);

  const openShelf = useCallback(
    (id: number) => {
      remember(id);
      setTab("shelf");
    },
    [remember]
  );

  // Счётчик на вкладке — единственный намёк, что книги вообще где-то потерялись.
  const refreshUnshelved = useCallback(() => {
    api.booksWithoutShelf().then((b) => setUnshelved(b.length)).catch(() => {});
  }, []);
  useEffect(refreshUnshelved, [refreshUnshelved, tab]);

  // Полка могла быть удалена в прошлый раз — тогда сохранённый id ведёт в пустоту.
  useEffect(() => {
    if (activeShelf === null) return;
    api.locationsAll()
      .then((ls) => {
        if (!ls.some((l) => l.id === activeShelf && l.kind === "shelf")) {
          setActiveShelf(null);
          localStorage.removeItem(LAST_SHELF);
        }
      })
      .catch(() => {});
  }, [activeShelf]);

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
            >
              <Icon name={n.icon} />
              <span>{n.label}</span>
              {n.id === "unshelved" && unshelved > 0 && (
                <span className="chip chip--brass" style={{ marginLeft: "auto" }}>{unshelved}</span>
              )}
            </button>
          ))}
        </div>
        <div className="rail__foot">Домашняя библиотека · офлайн</div>
      </nav>

      <main className="main">
        <div className="view" key={tab === "shelf" ? `shelf-${activeShelf}` : tab}>
          {/* Полка не выбрана — предлагаем выбрать, а не упираемся в серую кнопку */}
          {tab === "shelf" &&
            (activeShelf ? <ShelfView shelfId={activeShelf} onOpenShelf={openShelf} /> : <ShelfPicker onOpenShelf={openShelf} />)}
          {tab === "unshelved" && <UnshelvedView onOpenShelf={openShelf} />}
          {tab === "capture" && <CaptureMode onShelfUsed={remember} />}
          {tab === "add" && <AddBookView onOpenShelf={openShelf} onShelfUsed={remember} />}
          {tab === "search" && <SearchView onOpenShelf={openShelf} onShelfUsed={remember} />}
          {tab === "stats" && <StatsView />}
          {tab === "settings" && <SettingsView onOpenShelf={openShelf} />}
        </div>
      </main>
    </div>
  );
}
