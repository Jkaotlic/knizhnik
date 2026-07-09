import { useEffect, useRef, useState } from "react";
import { api, Location, CaptureResult } from "../api";

interface FeedItem {
  title: string;
  duplicate: boolean;
  source: string;
  noMeta: boolean;
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
        { title: r.book.title, duplicate: r.is_possible_duplicate, source: r.source, noMeta: r.source === "none" },
        ...f,
      ]);
      setCount((c) => c + 1);
    } catch (err) {
      setFeed((f) => [{ title: `Ошибка: ${String(err)}`, duplicate: false, source: "error", noMeta: false }, ...f]);
    } finally {
      setBusy(false);
      refocus();
    }
  };

  return (
    <div>
      <h2>Капчур на полку</h2>
      <div className="row">
        <label>Полка:</label>
        <select value={shelfId ?? ""} onChange={(e) => setShelfId(Number(e.target.value) || null)}>
          <option value="">— выбери полку —</option>
          {shelves.map((s) => (
            <option key={s.id} value={s.id}>{s.name}{s.label ? ` [${s.label}]` : ""}</option>
          ))}
        </select>
        <span className="muted">Добавлено за сессию: {count}</span>
      </div>
      <div className="row">
        <input
          ref={inputRef}
          placeholder={shelfId ? "Сканируй ISBN…" : "Сначала выбери полку"}
          disabled={!shelfId}
          onKeyDown={onKey}
          autoFocus
        />
      </div>
      <ul>
        {feed.map((item, i) => (
          <li key={i}>
            ✓ {item.title}
            {item.duplicate && <span className="muted"> — возможно дубль</span>}
            {item.noMeta && <span className="muted"> — метаданные не найдены, дозаполни вручную</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}
