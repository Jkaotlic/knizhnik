import { useEffect, useRef, useState } from "react";
import { api, Location, CaptureResult } from "../api";
import { spineColor } from "../theme";
import { Icon } from "./Icon";

interface FeedItem {
  title: string;
  color: string;
  duplicate: boolean;
  noMeta: boolean;
  error: boolean;
}

export function CaptureMode() {
  const [shelves, setShelves] = useState<Location[]>([]);
  const [shelfId, setShelfId] = useState<number | null>(null);
  const [feed, setFeed] = useState<FeedItem[]>([]);
  const [count, setCount] = useState(0);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    api.locationsAll().then((ls) => setShelves(ls.filter((l) => l.kind === "shelf")));
  }, []);

  const refocus = () => inputRef.current?.focus();
  useEffect(refocus, [shelfId, feed]);

  const onKey = async (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "Enter" || !shelfId || busy) return;
    const isbn = e.currentTarget.value.trim();
    e.currentTarget.value = "";
    if (!isbn) return;
    setBusy(true);
    try {
      const r: CaptureResult = await api.capture(shelfId, isbn);
      setFeed((f) => [
        { title: r.book.title, color: spineColor(r.book.id), duplicate: r.is_possible_duplicate, noMeta: r.source === "none", error: false },
        ...f,
      ]);
      setCount((c) => c + 1);
    } catch (err) {
      setFeed((f) => [{ title: String(err), color: "var(--rust)", duplicate: false, noMeta: false, error: true }, ...f]);
    } finally {
      setBusy(false);
      refocus();
    }
  };

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
        <select className="select" value={shelfId ?? ""} onChange={(e) => setShelfId(Number(e.target.value) || null)}>
          <option value="">— выбери полку —</option>
          {shelves.map((s) => (
            <option key={s.id} value={s.id}>{s.name}{s.label ? ` · ${s.label}` : ""}</option>
          ))}
        </select>
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

      <ul className="feed">
        {feed.map((item, i) => (
          <li key={feed.length - i} className={`feed__item${item.error ? " is-error" : ""}`} style={{ borderLeftColor: item.color }}>
            <span className="feed__check"><Icon name={item.error ? "search" : "check"} size={14} /></span>
            <span className="feed__title">{item.title}</span>
            {item.duplicate && <span className="chip chip--brass">возможно дубль</span>}
            {item.noMeta && <span className="chip">без метаданных — дозаполни</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}
