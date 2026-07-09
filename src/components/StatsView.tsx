import { useEffect, useState } from "react";
import { api, Stats } from "../api";
import { statusRu } from "../theme";
import { Icon } from "./Icon";

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
      <div className="page-head">
        <div>
          <div className="eyebrow">Статистика</div>
          <h2 className="page-title">Твоя библиотека в цифрах</h2>
        </div>
        <button className="btn" onClick={exportCsv}><Icon name="download" size={16} /> Экспорт в CSV</button>
      </div>

      <div className="stat-grid">
        <div className="stat">
          <div className="stat__num">{stats.total}</div>
          <div className="stat__lbl">всего книг</div>
        </div>
        <div className="stat" style={{ animationDelay: "70ms" }}>
          <div className="stat__num">{stats.pages_read.toLocaleString("ru-RU")}</div>
          <div className="stat__lbl">страниц прочитано</div>
        </div>
      </div>

      <div className="section-label">по статусам</div>
      <Bars data={stats.by_status.map(([k, n]) => [statusRu[k] ?? k, n])} />

      <div className="section-label">топ жанров</div>
      <Bars data={stats.top_genres} />
    </div>
  );
}
