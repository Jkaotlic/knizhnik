import { useRef, useState } from "react";
import { api, BookHit, Candidate } from "../api";
import { spineColor } from "../theme";
import { Icon } from "./Icon";
import { sourceLabel } from "../theme";
import { Note, useNote } from "./Note";
import { Cover, useCoversDir } from "./Cover";
import { ShelfSelect } from "./ShelfSelect";

export function SearchView({
  onOpenShelf,
  onShelfUsed,
}: {
  onOpenShelf: (id: number) => void;
  onShelfUsed: (id: number) => void;
}) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<BookHit[]>([]);

  const [target, setTarget] = useState<number | null>(null);
  const [candidates, setCandidates] = useState<Candidate[] | null>(null);
  const [onlineBusy, setOnlineBusy] = useState(false);
  const note = useNote();
  const coversDir = useCoversDir();

  // Запросы летят на каждый символ и возвращаются в произвольном порядке —
  // без этого счётчика ответ на «дю» мог перезаписать ответ на «дюна».
  const seq = useRef(0);

  const search = async (value: string) => {
    const mine = ++seq.current;
    try {
      const found = await api.booksSearch(value);
      if (mine === seq.current) setHits(found);
    } catch (e) {
      if (mine === seq.current) { setHits([]); note.fail(String(e)); }
    }
  };

  const run = async (value: string) => {
    setQ(value);
    setCandidates(null);
    note.clear();
    if (value.trim().length < 2) { seq.current++; setHits([]); return; }
    await search(value.trim());
  };

  const digits = q.replace(/\D/g, "");
  const looksIsbn = digits.length >= 10;

  const lookupOnline = async () => {
    setOnlineBusy(true);
    note.clear();
    try {
      const cands = await api.metadataLookupIsbn(q.trim());
      setCandidates(cands);
      if (cands.length === 0) note.ok("По этому ISBN ничего не нашлось в Open Library и Google.");
    } catch (e) {
      setCandidates([]);
      note.fail(String(e));
    } finally {
      setOnlineBusy(false);
    }
  };

  const add = async (c: Candidate) => {
    if (!target) { note.fail("Выбери полку, куда добавить книгу."); return; }
    try {
      const created = await api.bookCreate({
        title: c.title,
        authors: c.authors ?? undefined,
        isbn: c.isbn ?? (digits || undefined),
        year: c.year ?? undefined,
        publisher: c.publisher ?? undefined,
        pages: c.pages ?? undefined,
        language: c.language ?? undefined,
        cover_url: c.cover_url ?? undefined,
        shelf_id: target,
      });
      api.coverCache(created.id).catch(() => {});
      onShelfUsed(target);
      // Полка могла быть создана прямо в селекте — путь берём у бэкенда.
      const where = await api.locationBreadcrumb(target).catch(() => "");
      note.ok(`«${c.title}» добавлена на полку ${where}.`);
      setCandidates(null);
      if (q.trim().length >= 2) await search(q.trim());
    } catch (e) {
      note.fail(`Не удалось добавить: ${String(e)}`);
    }
  };

  return (
    <div>
      <div className="eyebrow">Поиск</div>
      <h2 className="page-title" style={{ marginBottom: 18 }}>Где эта книга?</h2>

      <div className="search__field">
        <Icon name="search" size={18} />
        <input
          className="input search__input"
          value={q}
          onChange={(e) => run(e.target.value)}
          placeholder="Название, автор, ISBN или жанр"
          autoFocus
        />
      </div>

      {q.trim().length >= 2 && (
        <>
          <div className="section-label">в каталоге · {hits.length}</div>
          {hits.length === 0 && <div className="empty">Ничего не нашлось в твоём каталоге</div>}
          <div className="books">
            {hits.map((h, i) => (
              <div key={h.book.id} className="book-card" style={{ animationDelay: `${Math.min(i, 12) * 35}ms` }}>
                <div className="book-card__spine" style={{ background: spineColor(h.book.id) }} />
                <Cover book={h.book} dir={coversDir} className="book-card__cover" />
                <div className="book-card__body">
                  <div className="book-card__title" style={{ cursor: "default" }}>{h.book.title}</div>
                  <div className="book-card__meta">
                    {h.book.authors && <span className="muted small">{h.book.authors}</span>}
                    {h.book.isbn && <span className="chip mono">{h.book.isbn}</span>}
                    {h.breadcrumb && h.book.shelf_id && (
                      <span className="chip is-link" onClick={() => onOpenShelf(h.book.shelf_id!)}>{h.breadcrumb}</span>
                    )}
                    {h.off_shelf && <span className="chip chip--rust">не на полке{h.book.lent_to ? `: ${h.book.lent_to}` : ""}</span>}
                  </div>
                </div>
              </div>
            ))}
          </div>

          <div className="section-label">в интернете по ISBN</div>
          <div className="btn-row" style={{ marginBottom: 12 }}>
            <button className="btn btn--brass" onClick={lookupOnline} disabled={!looksIsbn || onlineBusy}>
              <Icon name="globe" size={16} /> {onlineBusy ? "Ищу…" : "Найти по ISBN"}
            </button>
            {!looksIsbn && <span className="muted small">Введи ISBN (10–13 цифр), чтобы искать онлайн</span>}
            {candidates && candidates.length > 0 && (
              <>
                <span className="label muted" style={{ marginLeft: "auto" }}>на полку</span>
                <ShelfSelect value={target} onChange={setTarget} />
              </>
            )}
          </div>

          <Note note={note.note} />

          <div className="stack">
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
                    <button className="btn btn--primary btn--sm" onClick={() => add(c)} disabled={!target}>
                      <Icon name="plus" size={15} /> Добавить на полку
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
