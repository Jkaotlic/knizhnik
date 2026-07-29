import { useEffect, useState } from "react";
import { api, Book, BookInput, Candidate, Location } from "../api";
import { Icon } from "./Icon";
import { useDialog } from "./Dialog";

export function BookEditor({
  book,
  onDone,
  onSaved,
}: {
  book: Book;
  onDone: () => void;
  /** Обновлённая книга — чтобы вызывающий экран мог освежить свою карточку. */
  onSaved?: (updated: Book) => void;
}) {
  const dlg = useDialog();
  const [shelves, setShelves] = useState<Location[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.locationsAll()
      .then((locs) => setShelves(locs.filter((l) => l.kind === "shelf")))
      .catch((e) => setError(String(e)));
  }, []);

  const [form, setForm] = useState<BookInput>({
    title: book.title,
    authors: book.authors ?? undefined,
    isbn: book.isbn ?? undefined,
    year: book.year ?? undefined,
    publisher: book.publisher ?? undefined,
    pages: book.pages ?? undefined,
    language: book.language ?? undefined,
    genre: book.genre ?? undefined,
    annotation: book.annotation ?? undefined,
    cover_url: book.cover_url ?? undefined,
    shelf_id: book.shelf_id ?? undefined,
    status: book.status ?? undefined,
    rating: book.rating ?? undefined,
    notes: book.notes ?? undefined,
    started_at: book.started_at ?? undefined,
    finished_at: book.finished_at ?? undefined,
  });

  // Ставим «дочитал = сегодня» одним кликом, когда статус меняют на «прочитано».
  const today = () => new Date().toISOString().slice(0, 10);

  const set = <K extends keyof BookInput>(k: K, v: BookInput[K] | "") =>
    setForm((f) => ({ ...f, [k]: v === "" ? undefined : v }));

  const applyCandidate = (c: Candidate) =>
    setForm((f) => ({
      ...f,
      title: c.title || f.title,
      authors: c.authors ?? f.authors,
      year: c.year ?? f.year,
      publisher: c.publisher ?? f.publisher,
      pages: c.pages ?? f.pages,
      language: c.language ?? f.language,
      cover_url: c.cover_url ?? f.cover_url,
    }));

  const lookup = async () => {
    if (!form.isbn) return;
    setBusy(true);
    try {
      const cands = await api.metadataLookupIsbn(form.isbn);
      if (cands[0]) applyCandidate(cands[0]);
      else dlg.alert("Метаданные не найдены");
    } catch (e) {
      dlg.alert(String(e));
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    if (!form.title.trim()) { setError("У книги должно быть название."); return; }
    setError(null);
    try {
      const updated = await api.bookUpdate(book.id, form);
      onSaved?.(updated);
      onDone();
    } catch (e) {
      setError(String(e)); // раньше правка молча не сохранялась
    }
  };

  const remove = async () => {
    if (!(await dlg.confirm("Удалить книгу?", { danger: true, okLabel: "Удалить" }))) return;
    try {
      await api.bookDelete(book.id);
      onDone();
    } catch (e) {
      setError(String(e));
    }
  };

  const field = (label: string, key: keyof BookInput, type: "text" | "number" = "text") => (
    <div className="field">
      <span className="label">{label}</span>
      <input
        className="input"
        type={type}
        value={(form[key] as string | number | undefined) ?? ""}
        onChange={(e) => {
          const v = e.target.value;
          set(key, (type === "number" ? (v === "" ? undefined : Number(v)) : v) as never);
        }}
      />
    </div>
  );

  return (
    <div className="editor">
      <h3>Правка книги</h3>
      {field("Название", "title")}
      {field("Авторы", "authors")}
      <div className="field">
        <span className="label">ISBN</span>
        <input className="input mono" value={form.isbn ?? ""} onChange={(e) => set("isbn", e.target.value)} />
        <button className="btn btn--brass btn--sm" onClick={lookup} disabled={busy || !form.isbn}>
          <Icon name="globe" size={15} /> {busy ? "Ищу…" : "Подтянуть"}
        </button>
      </div>
      {field("Год", "year", "number")}
      {field("Издатель", "publisher")}
      {field("Страниц", "pages", "number")}
      {field("Язык", "language")}
      {field("Жанр", "genre")}
      <div className="field">
        <span className="label">Полка</span>
        <select
          className="select"
          value={form.shelf_id ?? ""}
          onChange={(e) => set("shelf_id", (e.target.value ? Number(e.target.value) : undefined) as never)}
        >
          <option value="">— без полки —</option>
          {shelves.map((s) => (
            <option key={s.id} value={s.id}>{s.name}{s.label ? ` · ${s.label}` : ""}</option>
          ))}
        </select>
      </div>
      <div className="field">
        <span className="label">Статус</span>
        <select
          className="select"
          value={form.status ?? ""}
          onChange={(e) => {
            const next = (e.target.value || undefined) as BookInput["status"];
            setForm((f) => ({
              ...f,
              status: next,
              // проставляем дату сами, но не затираем уже введённую
              started_at: next === "reading" && !f.started_at ? today() : f.started_at,
              finished_at: next === "read" && !f.finished_at ? today() : f.finished_at,
            }));
          }}
        >
          <option value="">не указан</option>
          <option value="want">хочу прочитать</option>
          <option value="reading">читаю</option>
          <option value="read">прочитано</option>
        </select>
      </div>
      <div className="field">
        <span className="label">Начал читать</span>
        <input
          className="input"
          type="date"
          value={form.started_at ?? ""}
          onChange={(e) => set("started_at", e.target.value)}
        />
      </div>
      <div className="field">
        <span className="label">Дочитал</span>
        <input
          className="input"
          type="date"
          value={form.finished_at ?? ""}
          onChange={(e) => set("finished_at", e.target.value)}
        />
      </div>
      <div className="field">
        <span className="label">Оценка (0–5)</span>
        <input
          className="input"
          type="number"
          min={0}
          max={5}
          value={form.rating ?? ""}
          onChange={(e) => {
            const v = e.target.value;
            set("rating", v === "" ? "" : Math.min(5, Math.max(0, Number(v))));
          }}
        />
      </div>
      {field("Заметки", "notes")}
      {error && <p className="error-note" style={{ marginTop: 10 }}>{error}</p>}
      <div className="btn-row" style={{ marginTop: 14 }}>
        <button className="btn btn--primary" onClick={save}><Icon name="check" size={16} /> Сохранить</button>
        <button className="btn btn--ghost" onClick={onDone}>Отмена</button>
        <button className="btn btn--ghost btn--danger" onClick={remove} style={{ marginLeft: "auto" }}>
          <Icon name="trash" size={16} /> Удалить
        </button>
      </div>
    </div>
  );
}
