import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { Icon } from "./Icon";
import { useDialog } from "./Dialog";

export function SettingsView() {
  const dlg = useDialog();
  const [key, setKey] = useState("");
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [backupNote, setBackupNote] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => { api.getGoogleKey().then(setKey); }, []);

  const stamp = () => new Date().toISOString().slice(0, 10);

  const exportBackup = async () => {
    const json = await api.backupExport();
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `knizhnik-backup-${stamp()}.json`;
    a.click();
    URL.revokeObjectURL(url);
    setBackupNote("Резервная копия сохранена.");
  };

  const importBackup = async (file: File) => {
    if (!(await dlg.confirm("Восстановить из копии? Текущий каталог (полки и книги) будет заменён.", { okLabel: "Восстановить" }))) return;
    try {
      const text = await file.text();
      const s = await api.backupImport(text);
      setBackupNote(`Восстановлено: полок ${s.locations}, книг ${s.books}. Переключись на другие вкладки, чтобы увидеть данные.`);
    } catch (e) {
      setBackupNote(`Не удалось: ${String(e)}`);
    }
  };

  const save = async () => {
    await api.setGoogleKey(key);
    setNote("Ключ сохранён локально.");
  };

  const check = async () => {
    setBusy(true);
    setNote(null);
    try {
      await api.setGoogleKey(key);
      const cands = await api.metadataLookupIsbn("9785171183660");
      setNote(
        cands.length > 0
          ? `Google отвечает — нашлась книга «${cands[0].title}». Ключ работает.`
          : "Запрос прошёл без ошибки, но книга не найдена. Ключ, похоже, рабочий."
      );
    } catch (e) {
      setNote(`Не сработало: ${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <div className="eyebrow">Настройки</div>
      <h2 className="page-title" style={{ marginBottom: 18 }}>Источники метаданных</h2>

      <div className="editor" style={{ marginTop: 0 }}>
        <h3>Google Books API-ключ</h3>
        <p className="small muted" style={{ marginTop: 4, lineHeight: 1.5 }}>
          Без ключа Google быстро упирается в дневной лимит (429), и русские книги часто не подтягиваются.
          Бесплатный ключ (1000 запросов/день) снимает лимит и заметно улучшает покрытие.
          Хранится только локально, на этом компьютере.
        </p>

        <div className="field" style={{ marginTop: 14 }}>
          <span className="label">Ключ</span>
          <input
            className="input mono"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder="AIza…"
            spellCheck={false}
          />
        </div>

        <div className="btn-row" style={{ marginTop: 12 }}>
          <button className="btn btn--primary" onClick={save}><Icon name="check" size={16} /> Сохранить</button>
          <button className="btn btn--brass" onClick={check} disabled={busy}>
            <Icon name="globe" size={16} /> {busy ? "Проверяю…" : "Проверить"}
          </button>
          {key && (
            <button className="btn btn--ghost btn--danger" onClick={() => { setKey(""); api.setGoogleKey(""); setNote("Ключ удалён."); }}>
              Удалить ключ
            </button>
          )}
        </div>

        {note && <p className="small" style={{ color: "var(--green)", marginTop: 10 }}>{note}</p>}

        <p className="small muted" style={{ marginTop: 16, lineHeight: 1.6 }}>
          Где взять: <span className="mono">console.cloud.google.com</span> → создать проект →
          включить <b>Books API</b> → «Credentials» → «Create API key». Ограничь ключ на Books API.
        </p>
      </div>

      <div className="editor" style={{ marginTop: 18 }}>
        <h3>Резервная копия</h3>
        <p className="small muted" style={{ marginTop: 4, lineHeight: 1.5 }}>
          Данные и так сохраняются между обновлениями приложения. Резервная копия нужна, чтобы
          перенести каталог на другой компьютер или подстраховаться. Файл <span className="mono">.json</span> хранит
          все полки и книги (без API-ключа).
        </p>

        <div className="btn-row" style={{ marginTop: 14 }}>
          <button className="btn btn--primary" onClick={exportBackup}>
            <Icon name="download" size={16} /> Сохранить копию
          </button>
          <button className="btn btn--brass" onClick={() => fileRef.current?.click()}>
            <Icon name="globe" size={16} /> Восстановить из файла
          </button>
          <input
            ref={fileRef}
            type="file"
            accept="application/json,.json"
            style={{ display: "none" }}
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) importBackup(f);
              e.target.value = "";
            }}
          />
        </div>

        <p className="small muted" style={{ marginTop: 12, lineHeight: 1.5 }}>
          Восстановление <b>заменяет</b> текущий каталог данными из файла (id и связи книга↔полка сохраняются).
        </p>
        {backupNote && <p className="small" style={{ color: "var(--green)", marginTop: 8 }}>{backupNote}</p>}
      </div>
    </div>
  );
}
