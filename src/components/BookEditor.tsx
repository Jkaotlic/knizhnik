import { useEffect, useState } from "react";
import { api, Book, BookInput, Candidate, Location } from "../api";
import { Icon } from "./Icon";

export function BookEditor({ book, onDone }: { book: Book; onDone: () => void }) {
  const [shelves, setShelves] = useState<Location[]>([]);
  useEffect(() => {
    api.locationsAll().then((locs) => setShelves(locs.filter((l) => l.kind === "shelf")));
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
  });

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
    try {
      const cands = await api.metadataLookupIsbn(form.isbn);
      if (cands[0]) applyCandidate(cands[0]);
      else alert("Метаданные не найдены");
    } catch (e) {
      alert(String(e));
    }
  };

  const save = async () => { await api.bookUpdate(book.id, form); onDone(); };
  const remove = async () => { if (confirm("Удалить книгу?")) { await api.bookDelete(book.id); onDone(); } };

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
        <button className="btn btn--brass btn--sm" onClick={lookup}><Icon name="globe" size={15} /> Подтянуть</button>
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
          onChange={(e) => set("status", (e.target.value || undefined) as BookInput["status"])}
        >
          <option value="">не указан</option>
          <option value="want">хочу прочитать</option>
          <option value="reading">читаю</option>
          <option value="read">прочитано</option>
        </select>
      </div>
      {field("Оценка (0–5)", "rating", "number")}
      {field("Заметки", "notes")}
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
