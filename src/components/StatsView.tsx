import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, Stats } from "../api";
import { statusRu } from "../theme";
import { Icon } from "./Icon";
import { Note, useNote } from "./Note";

function Bars({ data }: { data: [string, number][] }) {
  if (data.length === 0) return <p className="muted small">Пока нет данных</p>;
  const max = Math.max(...data.map(([, n]) => n));
  return (
    <div className="bars">
      {data.map(([name, n], i) => (
        <div className="bar" key={name}>
          <span className="bar__name">{name}</span>
          <span className="bar__track">
            <span className="bar__fill" style={{ width: `${(n / max) * 100}%`, animationDelay: `${i * 60}ms` }} />
          </span>
          <span className="bar__val">{n}</span>
        </div>
      ))}
    </div>
  );
}

export function StatsView() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const note = useNote();

  useEffect(() => {
    api.statsSummary().then(setStats).catch((e) => setError(String(e)));
  }, []);

  const exportCsv = async () => {
    note.clear();
    try {
      const path = await save({
        defaultPath: `knizhnik-${new Date().toISOString().slice(0, 10)}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) return; // пользователь закрыл диалог
      await api.exportCsvTo(path);
      note.ok(`Сохранено: ${path}`);
    } catch (e) {
      note.fail(`Не удалось сохранить: ${String(e)}`);
    }
  };

  if (error) return <p className="error-note">{error}</p>;
  if (!stats) return <p className="muted">Загрузка…</p>;

  return (
    <div>
      <div className="page-head">
        <div>
          <div className="eyebrow">Статистика</div>
          <h2 className="page-title">Твоя библиотека в цифрах</h2>
        </div>
        <button className="btn" onClick={exportCsv}><Icon name="download" size={16} /> Экспорт в CSV</button>
      </div>

      <Note note={note.note} style={{ marginBottom: 14 }} />

      <div className="stat-grid">
        <div className="stat">
          <div className="stat__num">{stats.total}</div>
          <div className="stat__lbl">всего книг</div>
        </div>
        <div className="stat" style={{ animationDelay: "70ms" }}>
          <div className="stat__num">{stats.pages_read.toLocaleString("ru-RU")}</div>
          <div className="stat__lbl">страниц прочитано</div>
        </div>
        <div className="stat" style={{ animationDelay: "140ms" }}>
          <div className="stat__num">{stats.lent_out}</div>
          <div className="stat__lbl">на руках</div>
        </div>
        <div className="stat" style={{ animationDelay: "210ms" }}>
          <div className="stat__num" style={{ color: stats.overdue > 0 ? "var(--rust)" : undefined }}>
            {stats.overdue}
          </div>
          <div className="stat__lbl">просрочено</div>
        </div>
      </div>

      <div className="section-label">прочитано по годам</div>
      <Bars data={stats.by_year} />

      <div className="section-label">по статусам</div>
      <Bars data={stats.by_status.map(([k, n]) => [statusRu[k] ?? k, n])} />

      <div className="section-label">топ жанров</div>
      <Bars data={stats.top_genres} />
    </div>
  );
}
