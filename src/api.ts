import { invoke } from "@tauri-apps/api/core";

export type Kind = "root" | "room" | "bookcase" | "shelf";

export interface Location {
  id: number;
  parent_id: number | null;
  name: string;
  kind: Kind;
  label: string | null;
  position: number;
}

export interface Book {
  id: number;
  title: string;
  authors: string | null;
  isbn: string | null;
  year: number | null;
  publisher: string | null;
  pages: number | null;
  language: string | null;
  genre: string | null;
  annotation: string | null;
  cover_url: string | null;
  shelf_id: number | null;
  status: "want" | "reading" | "read" | null;
  rating: number | null;
  notes: string | null;
  availability: "on_shelf" | "lent" | "away";
  lent_to: string | null;
  added_at: string;
  updated_at: string;
  started_at: string | null;   // ГГГГ-ММ-ДД, начал читать
  finished_at: string | null;  // ГГГГ-ММ-ДД, дочитал
  lent_at: string | null;      // когда отдал
  due_at: string | null;       // когда ждём назад
  cover_path: string | null;   // локальный файл обложки
}

export type BookInput = Partial<
  Omit<Book, "id" | "added_at" | "updated_at" | "availability" | "lent_to" | "lent_at" | "due_at" | "cover_path">
> & {
  title: string;
};

export interface BookHit {
  book: Book;
  breadcrumb: string;
  off_shelf: boolean;
}

export interface Candidate {
  title: string;
  authors: string | null;
  isbn: string | null;
  year: number | null;
  publisher: string | null;
  pages: number | null;
  language: string | null;
  cover_url: string | null;
  source: string;
}

export interface CaptureResult {
  book: Book;
  is_possible_duplicate: boolean;
  /** Где ещё в каталоге лежит книга с этим ISBN. */
  duplicate_at: string[];
  source: string;
  /** Почему метаданные не подтянулись, если дело не в «книги нет в базах». */
  note: string | null;
}

/** Что зацепит удаление локации: вложенные локации и книги внутри. */
export interface SubtreeInfo {
  locations: number;
  books: number;
}

export interface ImportSummary {
  locations: number;
  books: number;
}

export interface ImportReport {
  added: number;
  skipped_duplicates: number;
  skipped_invalid: number;
  problems: string[];
}

export interface Stats {
  total: number;
  by_status: [string, number][];
  pages_read: number;
  top_genres: [string, number][];
  by_year: [string, number][];
  lent_out: number;
  overdue: number;
}

export const api = {
  locationsAll: () => invoke<Location[]>("locations_all"),
  locationCreate: (parent_id: number | null, name: string, kind: Kind, label: string | null) =>
    invoke<Location>("location_create", { parentId: parent_id, name, kind, label }),
  locationUpdate: (id: number, name: string | null, label: string | null) =>
    invoke<Location>("location_update", { id, name, label }),
  locationMove: (id: number, new_parent_id: number | null) =>
    invoke<void>("location_move", { id, newParentId: new_parent_id }),
  locationDelete: (id: number) => invoke<void>("location_delete", { id }),
  bookcaseCreate: (name: string) => invoke<Location>("bookcase_create", { name }),
  shelfCreate: (bookcase_id: number, name: string, label: string | null) =>
    invoke<Location>("shelf_create", { bookcaseId: bookcase_id, name, label }),
  locationSubtreeInfo: (id: number) => invoke<SubtreeInfo>("location_subtree_info", { id }),
  locationBreadcrumb: (shelf_id: number) => invoke<string>("location_breadcrumb", { shelfId: shelf_id }),
  bookCreate: (input: BookInput) => invoke<Book>("book_create", { input }),
  bookUpdate: (id: number, input: BookInput) => invoke<Book>("book_update", { id, input }),
  bookDelete: (id: number) => invoke<void>("book_delete", { id }),
  bookSetShelf: (id: number, shelf_id: number | null) => invoke<void>("book_set_shelf", { id, shelfId: shelf_id }),
  bookSetAvailability: (id: number, availability: string, lent_to: string | null, due_at: string | null) =>
    invoke<void>("book_set_availability", { id, availability, lentTo: lent_to, dueAt: due_at }),
  bookDuplicates: (isbn: string) => invoke<string[]>("book_duplicates", { isbn }),
  booksOnShelf: (shelf_id: number) => invoke<Book[]>("books_on_shelf", { shelfId: shelf_id }),
  booksWithoutShelf: () => invoke<Book[]>("books_without_shelf"),
  booksSearch: (query: string) => invoke<BookHit[]>("books_search", { query }),
  metadataLookupIsbn: (isbn: string) => invoke<Candidate[]>("metadata_lookup_isbn", { isbn }),
  metadataLookupTitle: (title: string) => invoke<Candidate[]>("metadata_lookup_title", { title }),
  capture: (shelf_id: number, isbn: string) => invoke<CaptureResult>("capture", { shelfId: shelf_id, isbn }),
  statsSummary: () => invoke<Stats>("stats_summary"),
  getGoogleKey: () => invoke<string>("settings_get_google_key"),
  setGoogleKey: (key: string) => invoke<void>("settings_set_google_key", { key }),
  // Файлы пишет и читает Rust: в WKWebView на macOS скачивание блоба
  // через `<a download>` молча не срабатывало.
  exportCsvTo: (path: string) => invoke<void>("export_csv_to", { path }),
  backupExportTo: (path: string) => invoke<void>("backup_export_to", { path }),
  backupImportFrom: (path: string) => invoke<ImportSummary>("backup_import_from", { path }),
  importCsvPreview: (path: string) => invoke<number>("import_csv_preview", { path }),
  importCsvApply: (path: string, shelf_id: number | null) =>
    invoke<ImportReport>("import_csv_apply", { path, shelfId: shelf_id }),
  coversDir: () => invoke<string>("covers_dir"),
  coverCache: (id: number) => invoke<string | null>("cover_cache", { id }),
  coversCacheAll: () => invoke<number>("covers_cache_all"),
  cacheSize: () => invoke<number>("cache_size"),
  cacheClear: () => invoke<number>("cache_clear"),
};
