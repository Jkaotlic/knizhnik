import { useEffect, useState } from "react";
import { api } from "../api";
import { Icon } from "./Icon";

export function SettingsView() {
  const [key, setKey] = useState("");
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => { api.getGoogleKey().then(setKey); }, []);

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
    </div>
  );
}
