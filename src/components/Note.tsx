import { useState } from "react";

// Раньше сообщения об ошибках красились в тот же зелёный, что и «сохранено» —
// сбой выглядел как успех. Успех и провал теперь различимы на глаз.

export type NoteState = { text: string; bad: boolean } | null;

export function useNote() {
  const [note, setNote] = useState<NoteState>(null);
  return {
    note,
    ok: (text: string) => setNote({ text, bad: false }),
    fail: (text: string) => setNote({ text, bad: true }),
    clear: () => setNote(null),
  };
}

export function Note({ note, style }: { note: NoteState; style?: React.CSSProperties }) {
  if (!note) return null;
  return (
    <p className="small" style={{ color: note.bad ? "var(--rust)" : "var(--green)", ...style }}>
      {note.text}
    </p>
  );
}
