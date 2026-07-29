import { useCallback, useEffect, useState } from "react";
import { api, Book } from "../api";
import { spineColor } from "../theme";
import { BookEditor } from "./BookEditor";
import { Icon } from "./Icon";
import { useDialog } from "./Dialog";
import { Cover, useCoversDir } from "./Cover";
import { ShelfSelect } from "./ShelfSelect";

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

export function ShelfView({
  shelfId,
  onOpenShelf,
}: {
  shelfId: number;
  onOpenShelf: (id: number) => void;
}) {
  const [crumb, setCrumb] = useState("");
  const [label, setLabel] = useState<string | null>(null);
  const [books, setBooks] = useState<Book[]>([]);
  const [editing, setEditing] = useState<Book | null>(null);
  const [error, setError] = useState<string | null>(null);
  const dlg = useDialog();
  const coversDir = useCoversDir();

  const reload = useCallback(() => {
    setError(null);
    Promise.all([
      api.locationBreadcrumb(shelfId).then(setCrumb),
      api.booksOnShelf(shelfId).then(setBooks),
      api.locationsAll().then((ls) => setLabel(ls.find((l) => l.id === shelfId)?.label ?? null)),
    ]).catch((e) => setError(String(e)));
  }, [shelfId]);
  useEffect(reload, [reload]);

  const lend = async (b: Book) => {
    const who = await dlg.prompt("Кому выдана? (пусто — вернуть на полку)", { defaultValue: b.lent_to ?? "" });
    if (who === null) return;
    const to = who.trim();
    let due: string | null = null;
    if (to) {
      const answer = await dlg.prompt("Когда ждём назад? ГГГГ-ММ-ДД (можно пусто)", {
        defaultValue: b.due_at ?? "",
        placeholder: "напр. 2026-09-01",
      });
      if (answer === null) return;
      due = answer.trim() || null;
    }
    try {
      await api.bookSetAvailability(b.id, to ? "lent" : "on_shelf", to || null, due);
      reload();
    } catch (e) {
      dlg.alert(String(e));
    }
  };

  const today = new Date().toISOString().slice(0, 10);
  const isOverdue = (b: Book) => b.availability === "lent" && !!b.due_at && b.due_at < today;

  // (id * 37) % 37 всегда 0 — все корешки выходили одной высоты.
  const spineHeight = (b: Book) => 60 + ((b.id * 37) % 29);

  return (
    <div>
      <div className="page-head">
        <div>
          <div className="eyebrow">Полка</div>
          <Breadcrumb path={crumb} label={label} />
        </div>
        <span className="muted small mono">{books.length} кн.</span>
      </div>

      {/* Отдельной строкой, а не в шапке: форма создания полки раскрывается
          прямо здесь, и ей нужна вся ширина. Заодно отсюда переключаются
          между полками — раньше, попав на полку, уйти на соседнюю было нельзя. */}
      <div className="btn-row" style={{ marginBottom: 18, alignItems: "flex-start" }}>
        <span className="label muted">полка</span>
        <ShelfSelect
          value={shelfId}
          onChange={(id) => id !== null && onOpenShelf(id)}
          withAddButton
        />
      </div>

      {error && <p className="error-note">{error}</p>}

      <div className="shelf">
        {books.length === 0 ? (
          <div className="shelf__empty">Полка пустует — добавь книги в разделе «Сканирование».</div>
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
            <Cover book={b} dir={coversDir} className="book-card__cover" />
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
                {b.due_at && b.availability === "lent" && (
                  <span className={`chip ${isOverdue(b) ? "chip--rust" : ""}`}>
                    {isOverdue(b) ? "просрочена с " : "ждём до "}{b.due_at}
                  </span>
                )}
                {b.finished_at && <span className="chip chip--green">дочитана {b.finished_at}</span>}
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
