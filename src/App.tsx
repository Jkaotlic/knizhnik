import { useState } from "react";
import { LocationTree } from "./components/LocationTree";
import { ShelfView } from "./components/ShelfView";
import { CaptureMode } from "./components/CaptureMode";
import { SearchView } from "./components/SearchView";
import { StatsView } from "./components/StatsView";
import "./App.css";

type Tab = "locations" | "shelf" | "capture" | "search" | "stats";

export default function App() {
  const [tab, setTab] = useState<Tab>("locations");
  const [activeShelf, setActiveShelf] = useState<number | null>(null);

  const openShelf = (id: number) => {
    setActiveShelf(id);
    setTab("shelf");
  };

  return (
    <div className="app">
      <nav className="tabs">
        <button onClick={() => setTab("locations")} className={tab === "locations" ? "on" : ""}>Локации</button>
        <button onClick={() => setTab("shelf")} className={tab === "shelf" ? "on" : ""} disabled={!activeShelf}>Полка</button>
        <button onClick={() => setTab("capture")} className={tab === "capture" ? "on" : ""}>Капчур</button>
        <button onClick={() => setTab("search")} className={tab === "search" ? "on" : ""}>Поиск</button>
        <button onClick={() => setTab("stats")} className={tab === "stats" ? "on" : ""}>Статистика</button>
      </nav>
      <main className="content">
        {tab === "locations" && <LocationTree onOpenShelf={openShelf} />}
        {tab === "shelf" && activeShelf && <ShelfView shelfId={activeShelf} />}
        {tab === "capture" && <CaptureMode />}
        {tab === "search" && <SearchView onOpenShelf={openShelf} />}
        {tab === "stats" && <StatsView />}
      </main>
    </div>
  );
}
