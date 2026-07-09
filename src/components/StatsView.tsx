import { useEffect, useState } from "react";
import { api, Stats } from "../api";

const statusRu: Record<string, string> = {
  want: "хочу прочитать",
  reading: "читаю",
  read: "прочитано",
  "не указан": "не указан",
};

export function StatsView() {
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => { api.statsSummary().then(setStats); }, []);

  const exportCsv = async () => {
    const csv = await api.exportCsv();
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "knizhnik.csv";
    a.click();
    URL.revokeObjectURL(url);
  };

  if (!stats) return <p className="muted">Загрузка…</p>;

  return (
    <div>
      <h2>Статистика</h2>
      <p>Всего книг: {stats.total}</p>
      <p>Страниц прочитано: {stats.pages_read}</p>
      <h3>По статусам</h3>
      <ul>{stats.by_status.map(([k, n]) => <li key={k}>{statusRu[k] ?? k}: {n}</li>)}</ul>
      <h3>Топ жанров</h3>
      <ul>{stats.top_genres.map(([g, n]) => <li key={g}>{g}: {n}</li>)}</ul>
      <button onClick={exportCsv}>Экспорт в CSV</button>
    </div>
  );
}
