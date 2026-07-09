// Тонкие линейные иконки, наследуют currentColor. Без внешних зависимостей.
import type { ReactNode } from "react";

type Name =
  | "locations"
  | "shelf"
  | "capture"
  | "search"
  | "stats"
  | "book"
  | "plus"
  | "pencil"
  | "move"
  | "trash"
  | "hand"
  | "download"
  | "globe"
  | "check"
  | "settings";

const paths: Record<Name, ReactNode> = {
  locations: (
    <>
      <path d="M4 5h6M4 12h9M4 19h5" />
      <path d="M4 5v14" />
    </>
  ),
  shelf: (
    <>
      <rect x="4" y="4" width="3.4" height="13" rx="0.5" />
      <rect x="8.6" y="4" width="3.4" height="13" rx="0.5" />
      <path d="M14.5 6.2l3.3-.9 2.4 12.6-3.3.9z" />
      <path d="M3 17.5h18" />
    </>
  ),
  capture: (
    <>
      <path d="M4 7V5.5A1.5 1.5 0 0 1 5.5 4H8M16 4h2.5A1.5 1.5 0 0 1 20 5.5V7M20 17v1.5a1.5 1.5 0 0 1-1.5 1.5H16M8 20H5.5A1.5 1.5 0 0 1 4 18.5V17" />
      <path d="M7 8v8M10 8v8M13 8v8M16 8v8" />
    </>
  ),
  search: (
    <>
      <circle cx="11" cy="11" r="6" />
      <path d="M20 20l-4.3-4.3" />
    </>
  ),
  stats: (
    <>
      <path d="M5 20V10M12 20V4M19 20v-7" />
      <path d="M3 20h18" />
    </>
  ),
  book: (
    <>
      <path d="M6 4h11a1 1 0 0 1 1 1v15H7a2 2 0 0 1-2-2V5a1 1 0 0 1 1-1z" />
      <path d="M9 4v14" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  pencil: (
    <>
      <path d="M4 20l1-4 11-11 3 3-11 11z" />
      <path d="M14 7l3 3" />
    </>
  ),
  move: <path d="M12 3v18M12 3l-3 3M12 3l3 3M12 21l-3-3M12 21l3-3M3 12h18M3 12l3-3M3 12l3 3M21 12l-3-3M21 12l-3 3" />,
  trash: (
    <>
      <path d="M4 7h16M9 7V5h6v2M6 7l1 13h10l1-13" />
    </>
  ),
  hand: (
    <>
      <path d="M8 11V6a1.4 1.4 0 0 1 2.8 0v4M10.8 10V5a1.4 1.4 0 0 1 2.8 0v5M13.6 10.5V7a1.4 1.4 0 0 1 2.8 0v6.5a5 5 0 0 1-5 5H10a4 4 0 0 1-3-1.4L4.5 14a1.5 1.5 0 0 1 2.3-2L8 13.5" />
    </>
  ),
  download: (
    <>
      <path d="M12 4v11M8 11l4 4 4-4" />
      <path d="M5 20h14" />
    </>
  ),
  globe: (
    <>
      <circle cx="12" cy="12" r="8" />
      <path d="M4 12h16M12 4c2.5 2.2 2.5 13.8 0 16M12 4c-2.5 2.2-2.5 13.8 0 16" />
    </>
  ),
  check: <path d="M5 12l4 4 10-10" />,
  settings: (
    <>
      <circle cx="12" cy="12" r="3.2" />
      <path d="M12 3.5v2.2M12 18.3v2.2M4.6 7.5l1.9 1.1M17.5 15.4l1.9 1.1M4.6 16.5l1.9-1.1M17.5 8.6l1.9-1.1" />
    </>
  ),
};

export function Icon({ name, size = 18 }: { name: Name; size?: number }) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths[name]}
    </svg>
  );
}
