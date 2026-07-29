import { useEffect, useRef, useState } from "react";
import { api, Book, CaptureResult } from "../api";
import { spineColor } from "../theme";
import { Icon } from "./Icon";
import { ShelfSelect } from "./ShelfSelect";
import { BookEditor } from "./BookEditor";

interface FeedItem {
  title: string;
  color: string;
  /** Где эта книга уже лежит — по всему каталогу, не только на этой полке. */
  duplicateAt: string[];
  noMeta: boolean;
  error: boolean;
  /** Сбой сети: книга на полке, но метаданные не подтянулись не потому, что их нет. */
  note: string | null;
  /** Сама книга — чтобы дозаполнить её прямо из ленты, не уходя искать на полке. */
  book: Book | null;
}

export function CaptureMode({ onShelfUsed }: { onShelfUsed: (id: number) => void }) {
  const [shelfId, setShelfId] = useState<number | null>(null);
  const [feed, setFeed] = useState<FeedItem[]>([]);
  const [count, setCount] = useState(0);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const [editing, setEditing] = useState<Book | null>(null);

  // Пока открыта форма дозаполнения, фокус не отбираем — иначе набранный
  // текст улетал бы в поле сканера.
  const refocus = () => inputRef.current?.focus();
  useEffect(() => {
    if (!editing) refocus();
  }, [shelfId, feed, editing]);

  const onKey = async (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "Enter" || !shelfId || busy) return;
    const isbn = e.currentTarget.value.trim();
    e.currentTarget.value = "";
    if (!isbn) return;
    setBusy(true);
    try {
      const r: CaptureResult = await api.capture(shelfId, isbn);
      setFeed((f) => [
        {
          title: r.book.title,
          color: spineColor(r.book.id),
          duplicateAt: r.duplicate_at,
          noMeta: r.source === "none",
          error: false,
          note: r.note,
          book: r.book,
        },
        ...f,
      ]);
      setCount((c) => c + 1);
      onShelfUsed(shelfId);
      // забираем обложку к себе фоном — сканирование ждать её не должно
      api.coverCache(r.book.id).catch(() => {});
    } catch (err) {
      setFeed((f) => [
        {
          title: String(err),
          color: "var(--rust)",
          duplicateAt: [],
          noMeta: false,
          error: true,
          note: null,
          book: null,
        },
        ...f,
      ]);
    } finally {
      setBusy(false);
      if (!editing) refocus();
    }
  };

  // После ручного дозаполнения строка в ленте должна перестать врать.
  const applyEdited = (updated: Book) =>
    setFeed((f) =>
      f.map((item) =>
        item.book?.id === updated.id
          ? { ...item, title: updated.title, noMeta: false, book: updated }
          : item
      )
    );

  return (
    <div>
      <div className="page-head">
        <div>
          <div className="eyebrow">Сканирование на полку</div>
          <h2 className="page-title">Добавляй книги сканером</h2>
        </div>
        <div className="counter">
          <span className="counter__num">{count}</span>
          <span className="counter__lbl">за сессию</span>
        </div>
      </div>

      <div className="capture__bar">
        <span className="label muted">Полка</span>
        <ShelfSelect value={shelfId} onChange={setShelfId} />
      </div>

      <div className={`scan${shelfId ? " is-armed" : ""}`}>
        <span className="scan__dot" />
        <input
          ref={inputRef}
          className="scan__input"
          placeholder={shelfId ? "Пикни штрихкод или введи ISBN и нажми Enter" : "Сначала выбери полку"}
          disabled={!shelfId}
          onKeyDown={onKey}
          autoFocus
        />
        <span className="scan__hint">{busy ? "ищу…" : "ISBN + Enter"}</span>
      </div>

      {/* Форма прямо под сканером: книга уже на полке, ей не хватает только
          данных — уходить за этим на полку и искать её там незачем. */}
      {editing && (
        <BookEditor
          book={editing}
          onSaved={applyEdited}
          onDone={() => { setEditing(null); refocus(); }}
        />
      )}

      <ul className="feed">
        {feed.map((item, i) => (
          <li key={feed.length - i} className={`feed__item${item.error ? " is-error" : ""}`} style={{ borderLeftColor: item.color }}>
            <span className="feed__check"><Icon name={item.error ? "search" : "check"} size={14} /></span>
            <span className="feed__title">{item.title}</span>
            {item.duplicateAt.length > 0 && (
              <span className="chip chip--brass" title={item.duplicateAt.join("\n")}>
                уже есть: {item.duplicateAt.slice(0, 2).join(", ")}
                {item.duplicateAt.length > 2 ? ` и ещё ${item.duplicateAt.length - 2}` : ""}
              </span>
            )}
            {item.noMeta && item.book && (
              <>
                <span className="chip chip--brass">не нашлась</span>
                <button
                  className="btn btn--primary btn--sm"
                  onClick={() => setEditing(item.book)}
                >
                  <Icon name="pencil" size={14} /> Заполнить руками
                </button>
              </>
            )}
            {item.note && <span className="chip chip--rust" title={item.note}>сеть: {item.note}</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}
