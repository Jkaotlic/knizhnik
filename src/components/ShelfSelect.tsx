import { useCallback, useEffect, useState } from "react";
import { api, Location } from "../api";
import { Icon } from "./Icon";

// Полку заводят редко, но вспоминают о ней всегда в одном и том же месте —
// когда собрались положить туда книгу. Раньше за этим приходилось уходить
// в дерево локаций и искать нужный шкаф среди кнопок переименования и удаления.
// Теперь полка создаётся прямо здесь, не уводя с экрана.
//
// «Дом» и «Комната» тут не упоминаются вовсе: их поднимает бэкенд.

const NEW = "__new__";

export function ShelfSelect({
  value,
  onChange,
  allowNone = false,
  noneLabel = "— без полки —",
  withAddButton = false,
}: {
  value: number | null;
  onChange: (id: number | null) => void;
  allowNone?: boolean;
  noneLabel?: string;
  /** Показать отдельную кнопку «+ Полка» рядом с выбором. */
  withAddButton?: boolean;
}) {
  const [shelves, setShelves] = useState<Location[]>([]);
  const [bookcases, setBookcases] = useState<Location[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [name, setName] = useState("");
  const [label, setLabel] = useState("");
  const [bookcaseId, setBookcaseId] = useState<number | null>(null);
  const [newBookcase, setNewBookcase] = useState("");

  const load = useCallback(async () => {
    try {
      const all = await api.locationsAll();
      const sh = all.filter((l) => l.kind === "shelf");
      const bc = all.filter((l) => l.kind === "bookcase");
      setShelves(sh);
      setBookcases(bc);
      setBookcaseId((cur) => cur ?? bc[0]?.id ?? null);
      // Пустая библиотека: показываем форму сразу, а не пустой список,
      // из которого некуда деться.
      if (sh.length === 0) setCreating(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoaded(true);
    }
  }, []);
  useEffect(() => { load(); }, [load]);

  const create = async () => {
    if (!name.trim()) { setError("Впиши название полки."); return; }
    setBusy(true);
    setError(null);
    try {
      let target = bookcaseId;
      if (target === null) {
        const title = newBookcase.trim();
        if (!title) { setError("Впиши название шкафа."); return; }
        target = (await api.bookcaseCreate(title)).id;
      }
      const shelf = await api.shelfCreate(target, name.trim(), label.trim() || null);
      await load();
      onChange(shelf.id);
      setCreating(false);
      setName("");
      setLabel("");
      setNewBookcase("");
      setBookcaseId(shelf.parent_id);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!loaded) return <span className="muted small">Загрузка…</span>;

  return (
    <>
      {!creating && (
        <select
          className="select"
          value={value ?? ""}
          onChange={(e) => {
            if (e.target.value === NEW) { setCreating(true); return; }
            onChange(e.target.value ? Number(e.target.value) : null);
          }}
        >
          {allowNone && <option value="">{noneLabel}</option>}
          {!allowNone && value === null && <option value="">— выбери полку —</option>}
          {shelves.map((s) => (
            <option key={s.id} value={s.id}>{s.name}{s.label ? ` · ${s.label}` : ""}</option>
          ))}
          <option value={NEW}>+ Новая полка…</option>
        </select>
      )}
      {!creating && withAddButton && (
        <button className="btn btn--brass btn--sm" onClick={() => setCreating(true)}>
          <Icon name="plus" size={15} /> Полка
        </button>
      )}

      {creating && (
        <div className="editor" style={{ marginTop: 0, width: "100%" }}>
          <h3>Новая полка</h3>
          <div className="field">
            <span className="label">Название</span>
            <input
              className="input"
              autoFocus
              value={name}
              placeholder="напр. Верхняя"
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && create()}
            />
          </div>
          <div className="field">
            <span className="label">Код (необязательно)</span>
            <input
              className="input mono"
              value={label}
              placeholder="напр. А-3"
              onChange={(e) => setLabel(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && create()}
            />
          </div>
          <div className="field">
            <span className="label">Шкаф</span>
            <select
              className="select"
              value={bookcaseId ?? NEW}
              onChange={(e) =>
                setBookcaseId(e.target.value === NEW ? null : Number(e.target.value))
              }
            >
              {bookcases.map((b) => (
                <option key={b.id} value={b.id}>{b.name}</option>
              ))}
              <option value={NEW}>+ Новый шкаф…</option>
            </select>
          </div>
          {bookcaseId === null && (
            <div className="field">
              <span className="label">Название шкафа</span>
              <input
                className="input"
                value={newBookcase}
                placeholder="напр. Шкаф у окна"
                onChange={(e) => setNewBookcase(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && create()}
              />
            </div>
          )}

          {error && <p className="error-note" style={{ marginTop: 8 }}>{error}</p>}

          <div className="btn-row" style={{ marginTop: 12 }}>
            <button className="btn btn--primary" onClick={create} disabled={busy}>
              <Icon name="check" size={16} /> {busy ? "Создаю…" : "Создать полку"}
            </button>
            {shelves.length > 0 && (
              <button
                className="btn btn--ghost"
                onClick={() => { setCreating(false); setError(null); }}
              >
                Отмена
              </button>
            )}
          </div>
        </div>
      )}

      {!creating && error && <p className="error-note">{error}</p>}
    </>
  );
}
