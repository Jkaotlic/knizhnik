import { useCallback, useEffect, useState } from "react";
import { api, Book } from "../api";
import { spineColor } from "../theme";
import { BookEditor } from "./BookEditor";
import { Icon } from "./Icon";

function Breadcrumb({ path, label }: { path: string; label: string | null }) {
  const parts = path.split(" › ").filter(Boolean);
  return (
    <div className="breadcrumb">
      <span className="breadcrumb__path">
        {parts.map((p, i) => (
          <span key={i}>
            {i > 0 && <span className="breadcrumb__sep"> › </span>}
            {p}
          </span>
        ))}
      </span>
      {label && <span className="shelf-tag">{label}</span>}
    </div>
  );
}

export function ShelfView({ shelfId }: { shelfId: number }) {
  const [crumb, setCrumb] = useState("");
  const [label, setLabel] = useState<string | null>(null);
  const [books, setBooks] = useState<Book[]>([]);
  const [editing, setEditing] = useState<Book | null>(null);

  const reload = useCallback(() => {
    api.locationBreadcrumb(shelfId).then(setCrumb);
    api.booksOnShelf(shelfId).then(setBooks);
    api.locationsAll().then((ls) => setLabel(ls.find((l) => l.id === shelfId)?.label ?? null));
  }, [shelfId]);
  useEffect(reload, [reload]);

  const lend = async (b: Book) => {
    const who = prompt("Кому выдана? (пусто — вернуть на полку)", b.lent_to ?? "");
    if (who === null) return;
    await api.bookSetAvailability(b.id, who ? "lent" : "on_shelf", who || null);
    reload();
  };

  const spineHeight = (b: Book) => 60 + ((b.id * 37) % 37);

  return (
    <div>
      <div className="page-head">
        <div>
          <div className="eyebrow">Полка</div>
          <Breadcrumb path={crumb} label={label} />
        </div>
        <span className="muted small mono">{books.length} кн.</span>
      </div>

      <div className="shelf">
        {books.length === 0 ? (
          <div className="shelf__empty">Полка пустует — занеси книги в режиме «Капчур».</div>
        ) : (
          <div className="shelf__spines">
            {books.map((b) => (
              <button
                key={b.id}
                className="spine"
                title={b.title}
                onClick={() => setEditing(b)}
                style={{ height: spineHeight(b), background: spineColor(b.id) }}
              />
            ))}
          </div>
        )}
        <div className="shelf__ledge" />
      </div>

      <div className="books">
        {books.map((b, i) => (
          <div key={b.id} className="book-card" style={{ animationDelay: `${Math.min(i, 12) * 35}ms` }}>
            <div className="book-card__spine" style={{ background: spineColor(b.id) }} />
            {b.cover_url && (
              <img className="book-card__cover" src={b.cover_url} alt="" onError={(e) => (e.currentTarget.style.display = "none")} />
            )}
            <div className="book-card__body">
              <button className="book-card__title" onClick={() => setEditing(b)}>{b.title}</button>
              <div className="book-card__meta">
                {b.authors && <span className="muted small">{b.authors}</span>}
                {b.isbn && <span className="chip mono">{b.isbn}</span>}
                {b.availability === "on_shelf" ? (
                  <span className="chip chip--green">на полке</span>
                ) : (
                  <span className="chip chip--rust">{b.availability === "lent" ? "выдана" : "не на месте"}{b.lent_to ? `: ${b.lent_to}` : ""}</span>
                )}
              </div>
            </div>
            <div className="book-card__actions">
              <button className="btn btn--ghost btn--sm" onClick={() => lend(b)} title="Выдать / вернуть">
                <Icon name="hand" size={16} /> Выдача
              </button>
            </div>
          </div>
        ))}
      </div>

      {editing && <BookEditor book={editing} onDone={() => { setEditing(null); reload(); }} />}
    </div>
  );
}
