import { useState } from "react";
import { api, Book, BookInput, Candidate } from "../api";

export function BookEditor({ book, onDone }: { book: Book; onDone: () => void }) {
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

  const save = async () => {
    await api.bookUpdate(book.id, form);
    onDone();
  };
  const remove = async () => {
    if (confirm("Удалить книгу?")) {
      await api.bookDelete(book.id);
      onDone();
    }
  };

  const field = (label: string, key: keyof BookInput, type: "text" | "number" = "text") => (
    <div className="row">
      <label style={{ width: 110 }}>{label}</label>
      <input
        type={type}
        value={(form[key] as string | number | undefined) ?? ""}
        onChange={(e) =>
          set(key, (type === "number" ? Number(e.target.value) || undefined : e.target.value) as never)
        }
      />
    </div>
  );

  return (
    <div style={{ border: "1px solid #ccc", padding: 12, marginTop: 8 }}>
      <h3>Правка книги</h3>
      {field("Название", "title")}
      {field("Авторы", "authors")}
      <div className="row">
        <label style={{ width: 110 }}>ISBN</label>
        <input value={form.isbn ?? ""} onChange={(e) => set("isbn", e.target.value)} />
        <button onClick={lookup}>Подтянуть по ISBN</button>
      </div>
      {field("Год", "year", "number")}
      {field("Издатель", "publisher")}
      {field("Страниц", "pages", "number")}
      {field("Язык", "language")}
      {field("Жанр", "genre")}
      <div className="row">
        <label style={{ width: 110 }}>Статус</label>
        <select
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
      <div className="row">
        <button onClick={save}>Сохранить</button>
        <button onClick={onDone}>Отмена</button>
        <button onClick={remove}>Удалить</button>
      </div>
    </div>
  );
}
