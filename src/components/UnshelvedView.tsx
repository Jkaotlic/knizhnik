import { useCallback, useEffect, useState } from "react";
import { api, Book, Location } from "../api";
import { spineColor } from "../theme";
import { BookEditor } from "./BookEditor";
import { Icon } from "./Icon";
import { Cover, useCoversDir } from "./Cover";
import { ShelfSelect } from "./ShelfSelect";

// Книга, добавленная без полки, раньше исчезала из виду: на полках её нет,
// в дереве локаций тоже. Здесь она и находится — и отсюда же раскладывается.

export function UnshelvedView({ onOpenShelf }: { onOpenShelf: (id: number) => void }) {
  const [books, setBooks] = useState<Book[]>([]);
  const [shelves, setShelves] = useState<Location[]>([]);
  const [editing, setEditing] = useState<Book | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const coversDir = useCoversDir();

  const reload = useCallback(() => {
    setError(null);
    Promise.all([
      api.booksWithoutShelf().then(setBooks),
      api.locationsAll().then((ls) => setShelves(ls.filter((l) => l.kind === "shelf"))),
    ])
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);
  useEffect(reload, [reload]);

  const putOnShelf = async (b: Book, shelfId: number) => {
    try {
      await api.bookSetShelf(b.id, shelfId);
      reload();
    } catch (e) {
      setError(String(e));
    }
  };

  if (loading) return <p className="muted">Загрузка…</p>;

  return (
    <div>
      <div className="page-head">
        <div>
          <div className="eyebrow">Без полки</div>
          <h2 className="page-title">Разложить по местам</h2>
        </div>
        <span className="muted small mono">{books.length} кн.</span>
      </div>

      {error && <p className="error-note">{error}</p>}

      {books.length === 0 ? (
        <div className="empty">Все книги разложены по полкам.</div>
      ) : (
        <>
          <p className="small muted" style={{ marginBottom: 14, lineHeight: 1.5 }}>
            Эти книги в каталоге есть, но неизвестно, где стоят. Выбери полку в строке
            книги — и она уедет туда.
          </p>
          {/* Полок нет вообще — разложить некуда, поэтому даём завести первую прямо здесь */}
          {shelves.length === 0 && (
            <div style={{ marginBottom: 16 }}>
              <ShelfSelect value={null} onChange={() => reload()} />
            </div>
          )}
          <div className="books">
            {books.map((b, i) => (
              <div key={b.id} className="book-card" style={{ animationDelay: `${Math.min(i, 12) * 35}ms` }}>
                <div className="book-card__spine" style={{ background: spineColor(b.id) }} />
                <Cover book={b} dir={coversDir} className="book-card__cover" />
                <div className="book-card__body">
                  <button className="book-card__title" onClick={() => setEditing(b)}>{b.title}</button>
                  <div className="book-card__meta">
                    {b.authors && <span className="muted small">{b.authors}</span>}
                    {b.isbn && <span className="chip mono">{b.isbn}</span>}
                    {b.year && <span className="chip mono">{b.year}</span>}
                  </div>
                </div>
                <div className="book-card__actions">
                  {shelves.length === 0 ? (
                    <span className="muted small">Заведи полку выше</span>
                  ) : (
                    <select
                      className="select"
                      defaultValue=""
                      onChange={(e) => e.target.value && putOnShelf(b, Number(e.target.value))}
                    >
                      <option value="" disabled>На полку…</option>
                      {shelves.map((s) => (
                        <option key={s.id} value={s.id}>{s.name}{s.label ? ` · ${s.label}` : ""}</option>
                      ))}
                    </select>
                  )}
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {shelves.length > 0 && books.length > 0 && (
        <div className="btn-row" style={{ marginTop: 16 }}>
          <button className="btn btn--ghost" onClick={() => onOpenShelf(shelves[0].id)}>
            <Icon name="shelf" size={16} /> Открыть полку «{shelves[0].name}»
          </button>
        </div>
      )}

      {editing && <BookEditor book={editing} onDone={() => { setEditing(null); reload(); }} />}
    </div>
  );
}
