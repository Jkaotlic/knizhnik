// Дизайн-словарь «Библиотека при лампе».

// Палитра корешков — приглушённые «переплётные» цвета. Цвет книги стабилен по id.
export const SPINES = [
  "#3B5A46", // зелёное сукно
  "#7E3B32", // оксблад
  "#2E3E52", // тёмно-синий
  "#A67C2E", // горчица
  "#2F5A55", // тил
  "#5A3A52", // слива
  "#3E5236", // хвоя
  "#9C5A3C", // терракота
  "#4A5560", // сланец
  "#8A6A2E", // охра
];

export function spineColor(seed: number): string {
  const n = ((seed % SPINES.length) + SPINES.length) % SPINES.length;
  return SPINES[n];
}

export const statusRu: Record<string, string> = {
  want: "хочу прочитать",
  reading: "читаю",
  read: "прочитано",
  "не указан": "не указан",
};

export const availabilityRu: Record<string, string> = {
  on_shelf: "на полке",
  lent: "выдана",
  away: "не на месте",
};

export const kindLabel = {
  root: "Дом",
  room: "Комната",
  bookcase: "Шкаф",
  shelf: "Полка",
} as const;

// Человеческие названия источников метаданных.
const SOURCE_LABELS: Record<string, string> = {
  openlibrary: "Open Library",
  google: "Google Books",
  dnb: "Нем. нацбиблиотека",
  loc: "Library of Congress",
  wildberries: "Wildberries",
};

// Данные по одной книге склеиваются из нескольких каталогов, и бэкенд
// присылает источник как «openlibrary+dnb» — показываем все, а не сырую строку.
export function sourceLabel(source: string): string {
  return source
    .split("+")
    .map((s) => SOURCE_LABELS[s] ?? s)
    .join(" + ");
}
