import { useEffect, useState } from "react";
import { api, Book } from "../api";
import { BookEditor } from "./BookEditor";

export function ShelfView({ shelfId }: { shelfId: number }) {
  const [crumb, setCrumb] = useState("");
  const [books, setBooks] = useState<Book[]>([]);
  const [editing, setEditing] = useState<Book | null>(null);

  const reload = () => {
    api.locationBreadcrumb(shelfId).then(setCrumb);
    api.booksOnShelf(shelfId).then(setBooks);
  };
  useEffect(reload, [shelfId]);

  const lend = async (b: Book) => {
    const who = prompt("Кому выдана?");
    if (who === null) return;
    await api.bookSetAvailability(b.id, who ? "lent" : "on_shelf", who || null);
    reload();
  };

  return (
    <div>
      <h2>{crumb}</h2>
      {books.length === 0 && <p className="muted">На полке пока нет книг</p>}
      {books.map((b) => (
        <div key={b.id} className="row">
          <button onClick={() => setEditing(b)}>{b.title}{b.authors ? ` — ${b.authors}` : ""}</button>
          {b.availability !== "on_shelf" && (
            <span className="muted">не на полке{b.lent_to ? `: ${b.lent_to}` : ""}</span>
          )}
          <button onClick={() => lend(b)}>Выдача</button>
        </div>
      ))}
      {editing && <BookEditor book={editing} onDone={() => { setEditing(null); reload(); }} />}
    </div>
  );
}
