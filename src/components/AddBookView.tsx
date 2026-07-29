import { useState } from "react";
import { api, BookInput, Candidate } from "../api";
import { Icon } from "./Icon";
import { ShelfSelect } from "./ShelfSelect";
import { sourceLabel } from "../theme";
import { Note, useNote } from "./Note";

const empty: BookInput = { title: "" };

export function AddBookView({
  onOpenShelf,
  onShelfUsed,
}: {
  onOpenShelf: (id: number) => void;
  onShelfUsed: (id: number) => void;
}) {
  const [form, setForm] = useState<BookInput>(empty);

  const [q, setQ] = useState("");
  const [candidates, setCandidates] = useState<Candidate[] | null>(null);
  const [busy, setBusy] = useState(false);
  const note = useNote();

  const set = <K extends keyof BookInput>(k: K, v: BookInput[K] | "") =>
    setForm((f) => ({ ...f, [k]: v === "" ? undefined : v }));

  const digits = q.replace(/\D/g, "");
  const byIsbn = digits.length >= 10;

  const lookup = async () => {
    if (q.trim().length < 2) return;
    setBusy(true);
    note.clear();
    try {
      const cands = byIsbn ? await api.metadataLookupIsbn(q.trim()) : await api.metadataLookupTitle(q.trim());
      setCandidates(cands);
      if (cands.length === 0) note.ok("Ничего не нашлось в Open Library и Google.");
    } catch (e) {
      setCandidates([]);
      note.fail(String(e));
    } finally {
      setBusy(false);
    }
  };

  const pick = (c: Candidate) => {
    setForm((f) => ({
      ...f,
      title: c.title || f.title,
      authors: c.authors ?? f.authors,
      isbn: c.isbn ?? (byIsbn ? digits : f.isbn),
      year: c.year ?? f.year,
      publisher: c.publisher ?? f.publisher,
      pages: c.pages ?? f.pages,
      language: c.language ?? f.language,
      cover_url: c.cover_url ?? f.cover_url,
    }));
    setCandidates(null);

    // Перечисляем, что реально приехало: иначе непонятно, то ли источник
    // беден, то ли кнопка не сработала.
    const filled = [
      c.authors && "автор",
      c.year && "год",
      c.publisher && "издатель",
      c.pages && "страницы",
      c.isbn && "ISBN",
      c.language && "язык",
      c.cover_url && "обложка",
    ].filter(Boolean);
    note.ok(
      filled.length > 0
        ? `Из «${sourceLabel(c.source)}» заполнено: ${filled.join(", ")}. Проверь, выбери полку и добавь.`
        : `«${c.title}» — кроме названия источник ничего не знает, дозаполни руками.`
    );
  };

  const save = async () => {
    if (!form.title || !form.title.trim()) { note.fail("Впиши хотя бы название."); return; }
    try {
      const book = await api.bookCreate(form);
      api.coverCache(book.id).catch(() => {});
      // Путь спрашиваем у бэкенда: полка могла быть создана только что,
      // прямо в селекте, и любой локальный список уже устарел.
      let where = " (без полки)";
      if (form.shelf_id) {
        onShelfUsed(form.shelf_id); // теперь вкладка «Полка» ведёт куда надо
        where = ` на полку ${await api.locationBreadcrumb(form.shelf_id).catch(() => "")}`.trimEnd();
      }
      note.ok(`«${book.title}» добавлена${where}.`);
      // Полку намеренно НЕ сбрасываем: подряд обычно заносят несколько книг
      // на одну и ту же, да и кнопка «Открыть полку» иначе исчезала сразу
      // после добавления — ровно когда она и нужна.
      setForm({ ...empty, shelf_id: form.shelf_id });
      setQ("");
    } catch (e) {
      note.fail(`Не удалось добавить: ${String(e)}`);
    }
  };

  const field = (label: string, key: keyof BookInput, type: "text" | "number" = "text") => (
    <div className="field">
      <span className="label">{label}</span>
      <input
        className={key === "isbn" ? "input mono" : "input"}
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
    <div>
      <div className="eyebrow">Добавить книгу</div>
      <h2 className="page-title" style={{ marginBottom: 18 }}>Найди или впиши вручную</h2>

      <div className="search__field">
        <Icon name="search" size={18} />
        <input
          className="input search__input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && lookup()}
          placeholder="Название или ISBN — найдём в интернете"
        />
      </div>
      <div className="btn-row" style={{ marginBottom: 8 }}>
        <button className="btn btn--brass" onClick={lookup} disabled={busy || q.trim().length < 2}>
          <Icon name="globe" size={16} /> {busy ? "Ищу…" : byIsbn ? "Найти по ISBN" : "Найти по названию"}
        </button>
        <span className="muted small">или заполни поля ниже руками</span>
      </div>

      <div className="stack" style={{ marginBottom: candidates && candidates.length ? 18 : 0 }}>
        {candidates?.map((c, i) => (
          <div key={i} className="candidate">
            {c.cover_url ? (
              <img className="candidate__cover" src={c.cover_url} alt="" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
            ) : (
              <div className="candidate__cover candidate__cover--ph"><Icon name="book" size={22} /></div>
            )}
            <div className="candidate__body">
              <div className="candidate__title">{c.title}</div>
              <div className="book-card__meta">
                {c.authors && <span className="muted small">{c.authors}</span>}
                {c.year && <span className="chip mono">{c.year}</span>}
                {c.publisher && <span className="muted small">{c.publisher}</span>}
                <span className="chip">{sourceLabel(c.source)}</span>
              </div>
              <div className="candidate__add">
                <button className="btn btn--primary btn--sm" onClick={() => pick(c)}>
                  <Icon name="check" size={15} /> Взять эти данные
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>

      <div className="editor" style={{ marginTop: 4 }}>
        <h3>Книга</h3>
        {field("Название", "title")}
        {field("Авторы", "authors")}
        {field("ISBN", "isbn")}
        {field("Год", "year", "number")}
        {field("Издатель", "publisher")}
        {field("Страниц", "pages", "number")}
        {field("Язык", "language")}
        {field("Жанр", "genre")}
        <div className="field">
          <span className="label">Полка</span>
          <ShelfSelect
            value={form.shelf_id ?? null}
            onChange={(id) => set("shelf_id", (id ?? undefined) as never)}
            allowNone
          />
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

        <Note note={note.note} style={{ marginTop: 8 }} />

        <div className="btn-row" style={{ marginTop: 14 }}>
          <button className="btn btn--primary" onClick={save}><Icon name="plus" size={16} /> Добавить книгу</button>
          {form.shelf_id && (
            <button className="btn btn--ghost" onClick={() => onOpenShelf(form.shelf_id!)}>Открыть полку</button>
          )}
          <button className="btn btn--ghost" onClick={() => { setForm(empty); setQ(""); setCandidates(null); note.clear(); }} style={{ marginLeft: "auto" }}>
            Очистить
          </button>
        </div>
      </div>
    </div>
  );
}
