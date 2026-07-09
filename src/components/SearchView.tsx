import { useState } from "react";
import { api, BookHit } from "../api";

export function SearchView({ onOpenShelf }: { onOpenShelf: (id: number) => void }) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<BookHit[]>([]);

  const run = async (value: string) => {
    setQ(value);
    if (value.trim().length < 2) {
      setHits([]);
      return;
    }
    setHits(await api.booksSearch(value.trim()));
  };

  return (
    <div>
      <h2>Поиск</h2>
      <input value={q} onChange={(e) => run(e.target.value)} placeholder="Название, автор, ISBN, жанр" autoFocus />
      {q.trim().length >= 2 && hits.length === 0 && <p className="muted">Ничего не найдено</p>}
      {hits.map((h) => (
        <div key={h.book.id} className="row">
          <span>{h.book.title}{h.book.authors ? ` — ${h.book.authors}` : ""}</span>
          {h.breadcrumb && (
            <button className="muted" onClick={() => h.book.shelf_id && onOpenShelf(h.book.shelf_id)}>
              {h.breadcrumb}
            </button>
          )}
          {h.off_shelf && <span className="muted">не на полке{h.book.lent_to ? `: ${h.book.lent_to}` : ""}</span>}
        </div>
      ))}
    </div>
  );
}
