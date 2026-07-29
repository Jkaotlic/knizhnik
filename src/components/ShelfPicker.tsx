import { useEffect, useState } from "react";
import { api, Location } from "../api";
import { Icon } from "./Icon";
import { ShelfSelect } from "./ShelfSelect";

// Вкладка «Полка» раньше была просто отключена, пока полку не откроют через
// «Локации», — и человек, добавивший книгу, упирался в серую кнопку.
// Теперь вкладка всегда живая, а выбрать полку можно прямо здесь.

export function ShelfPicker({ onOpenShelf }: { onOpenShelf: (id: number) => void }) {
  const [shelves, setShelves] = useState<Location[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.locationsAll()
      .then((ls) => setShelves(ls.filter((l) => l.kind === "shelf")))
      .catch((e) => setError(String(e)));
  }, []);

  if (error) return <p className="error-note">{error}</p>;
  if (!shelves) return <p className="muted">Загрузка…</p>;

  return (
    <div>
      <div className="eyebrow">Полка</div>
      <h2 className="page-title" style={{ marginBottom: 18 }}>Какую полку показать?</h2>

      {shelves.length > 0 && (
        <div className="stack" style={{ marginBottom: 18 }}>
          {shelves.map((s) => (
            <button
              key={s.id}
              className="btn btn--ghost"
              style={{ justifyContent: "flex-start" }}
              onClick={() => onOpenShelf(s.id)}
            >
              <Icon name="shelf" size={16} /> {s.name}
              {s.label && <span className="shelf-tag" style={{ marginLeft: 8 }}>{s.label}</span>}
            </button>
          ))}
        </div>
      )}

      {/* Это первый экран при запуске: на пустой библиотеке отсюда и заводится
          самая первая полка, без похода в другой раздел. */}
      <ShelfSelect value={null} onChange={(id) => id !== null && onOpenShelf(id)} />
    </div>
  );
}
