import { useEffect, useState } from "react";
import { api, BookInput, Candidate, Location } from "../api";
import { Icon } from "./Icon";

const empty: BookInput = { title: "" };

export function AddBookView({ onOpenShelf }: { onOpenShelf: (id: number) => void }) {
  const [shelves, setShelves] = useState<Location[]>([]);
  const [form, setForm] = useState<BookInput>(empty);

  const [q, setQ] = useState("");
  const [candidates, setCandidates] = useState<Candidate[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    api.locationsAll().then((ls) => setShelves(ls.filter((l) => l.kind === "shelf")));
  }, []);

  const set = <K extends keyof BookInput>(k: K, v: BookInput[K] | "") =>
    setForm((f) => ({ ...f, [k]: v === "" ? undefined : v }));

  const digits = q.replace(/\D/g, "");
  const byIsbn = digits.length >= 10;

  const lookup = async () => {
    if (q.trim().length < 2) return;
    setBusy(true);
    setNote(null);
    try {
      const cands = byIsbn ? await api.metadataLookupIsbn(q.trim()) : await api.metadataLookupTitle(q.trim());
      setCandidates(cands);
      if (cands.length === 0) setNote("Ничего не нашлось в Open Library и Google.");
    } catch (e) {
      setCandidates([]);
      setNote(String(e));
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
    setNote(`Поля заполнены из «${c.title}». Проверь, выбери полку и добавь.`);
  };

  const save = async () => {
    if (!form.title || !form.title.trim()) { setNote("Впиши хотя бы название."); return; }
    const book = await api.bookCreate(form);
    const shelf = shelves.find((s) => s.id === form.shelf_id);
    setNote(`«${book.title}» добавлена${shelf ? ` на полку ${shelf.name}${shelf.label ? ` · ${shelf.label}` : ""}` : " (без полки)"}.`);
    setForm(empty);
    setQ("");
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
                <span className="chip">{c.source === "openlibrary" ? "Open Library" : c.source === "google" ? "Google Books" : c.source}</span>
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

        {note && <p className="small" style={{ color: "var(--green)", marginTop: 8 }}>{note}</p>}

        <div className="btn-row" style={{ marginTop: 14 }}>
          <button className="btn btn--primary" onClick={save}><Icon name="plus" size={16} /> Добавить книгу</button>
          {form.shelf_id && (
            <button className="btn btn--ghost" onClick={() => onOpenShelf(form.shelf_id!)}>Открыть полку</button>
          )}
          <button className="btn btn--ghost" onClick={() => { setForm(empty); setQ(""); setCandidates(null); setNote(null); }} style={{ marginLeft: "auto" }}>
            Очистить
          </button>
        </div>
      </div>
    </div>
  );
}
