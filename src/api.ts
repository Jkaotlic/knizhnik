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
}

export type BookInput = Partial<Omit<Book, "id" | "added_at" | "updated_at" | "availability" | "lent_to">> & {
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
  source: string;
}

export interface Stats {
  total: number;
  by_status: [string, number][];
  pages_read: number;
  top_genres: [string, number][];
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
  locationBreadcrumb: (shelf_id: number) => invoke<string>("location_breadcrumb", { shelfId: shelf_id }),
  bookCreate: (input: BookInput) => invoke<Book>("book_create", { input }),
  bookUpdate: (id: number, input: BookInput) => invoke<Book>("book_update", { id, input }),
  bookDelete: (id: number) => invoke<void>("book_delete", { id }),
  bookSetShelf: (id: number, shelf_id: number | null) => invoke<void>("book_set_shelf", { id, shelfId: shelf_id }),
  bookSetAvailability: (id: number, availability: string, lent_to: string | null) =>
    invoke<void>("book_set_availability", { id, availability, lentTo: lent_to }),
  booksOnShelf: (shelf_id: number) => invoke<Book[]>("books_on_shelf", { shelfId: shelf_id }),
  booksSearch: (query: string) => invoke<BookHit[]>("books_search", { query }),
  metadataLookupIsbn: (isbn: string) => invoke<Candidate[]>("metadata_lookup_isbn", { isbn }),
  metadataLookupTitle: (title: string) => invoke<Candidate[]>("metadata_lookup_title", { title }),
  capture: (shelf_id: number, isbn: string) => invoke<CaptureResult>("capture", { shelfId: shelf_id, isbn }),
  statsSummary: () => invoke<Stats>("stats_summary"),
  exportCsv: () => invoke<string>("export_csv"),
};
