import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, Book } from "../api";

// Папка обложек одна на всё приложение — спрашиваем её один раз и делимся
// между всеми карточками, а не дёргаем бэкенд на каждую книгу.
let dirPromise: Promise<string> | null = null;
function coversDir(): Promise<string> {
  if (!dirPromise) dirPromise = api.coversDir();
  return dirPromise;
}

export function useCoversDir(): string | null {
  const [dir, setDir] = useState<string | null>(null);
  useEffect(() => {
    coversDir().then(setDir).catch(() => setDir(null));
  }, []);
  return dir;
}

/**
 * Показывает скачанную обложку, а если её нет — сетевую.
 * Порядок деградации намеренный: локальный файл работает офлайн, сетевой —
 * запасной, а если не открылось ни то ни другое, картинка просто исчезает.
 */
export function Cover({
  book,
  dir,
  className,
  hideOnError = "display",
}: {
  book: Pick<Book, "cover_path" | "cover_url">;
  dir: string | null;
  className?: string;
  hideOnError?: "display" | "visibility";
}) {
  const local = book.cover_path && dir ? convertFileSrc(`${dir}/${book.cover_path}`) : null;
  const [src, setSrc] = useState<string | null>(null);
  // Сдались или нет — состояние React, а не инлайновый стиль на узле. Стиль,
  // выставленный из onError, React обратно не убирает: книга, у которой
  // обложка появилась позже, так и оставалась невидимой.
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setSrc(local ?? book.cover_url ?? null);
    setFailed(false);
  }, [local, book.cover_url]);

  if (!src) return null;

  return (
    <img
      className={className}
      src={src}
      alt=""
      style={failed ? (hideOnError === "visibility" ? { visibility: "hidden" } : { display: "none" }) : undefined}
      onError={() => {
        // локальный файл не открылся — пробуем сетевой, потом сдаёмся
        if (local && src === local && book.cover_url) {
          setSrc(book.cover_url);
          return;
        }
        setFailed(true);
      }}
    />
  );
}
