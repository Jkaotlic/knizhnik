import { useEffect, useState } from "react";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
// `check` уже занято локальной проверкой ключа Google
import { check as checkForUpdate } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { api } from "../api";
import { Icon } from "./Icon";
import { useDialog } from "./Dialog";
import { Note, useNote } from "./Note";
import { LocationTree } from "./LocationTree";
import { ShelfSelect } from "./ShelfSelect";

export function SettingsView({ onOpenShelf }: { onOpenShelf: (id: number) => void }) {
  const dlg = useDialog();
  const [key, setKey] = useState("");
  const note = useNote();
  const [busy, setBusy] = useState(false);

  const backup = useNote();
  const cache = useNote();
  const [cached, setCached] = useState<number | null>(null);

  useEffect(() => {
    api.getGoogleKey().then(setKey).catch((e) => note.fail(String(e)));
    api.cacheSize().then(setCached).catch(() => setCached(null));
    getVersion().then(setVersion).catch(() => setVersion(""));
  }, []);

  const covers = useNote();
  const [coversBusy, setCoversBusy] = useState(false);

  const upd = useNote();
  const [updBusy, setUpdBusy] = useState(false);
  const [version, setVersion] = useState("");

  const checkUpdates = async () => {
    setUpdBusy(true);
    upd.clear();
    try {
      const found = await checkForUpdate();
      if (!found) {
        upd.ok("Установлена последняя версия.");
        return;
      }
      const ok = await dlg.confirm(
        `Доступна версия ${found.version}. Скачать и установить? Приложение перезапустится.`,
        { okLabel: "Обновить" }
      );
      if (!ok) return;
      upd.ok("Качаю обновление…");
      await found.downloadAndInstall();
      await relaunch();
    } catch (e) {
      // Самая частая причина — обновления ещё не настроены (нет ключа подписи).
      upd.fail(`Не удалось проверить обновления: ${String(e)}`);
    } finally {
      setUpdBusy(false);
    }
  };

  const imp = useNote();
  const [impBusy, setImpBusy] = useState(false);
  const [impShelf, setImpShelf] = useState<number | null>(null);

  // Импорт ДОПОЛНЯЕТ каталог (в отличие от восстановления из копии),
  // поэтому спрашиваем подтверждение с точным числом найденных книг.
  const importCsv = async () => {
    imp.clear();
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "CSV", extensions: ["csv", "tsv", "txt"] }],
      });
      if (typeof picked !== "string") return;

      setImpBusy(true);
      const found = await api.importCsvPreview(picked);
      if (found === 0) {
        imp.fail("В файле не нашлось ни одной книги.");
        return;
      }
      // Полку могли создать прямо в селекте — путь спрашиваем у бэкенда.
      const where = impShelf ? await api.locationBreadcrumb(impShelf).catch(() => "") : "";
      const ok = await dlg.confirm(
        `Нашлось книг: ${found}. Добавить их ${where ? `на полку «${where}»` : "без полки"}? ` +
          `Существующие книги не пострадают, а совпадения по ISBN будут пропущены.`,
        { okLabel: "Добавить" }
      );
      if (!ok) return;

      const r = await api.importCsvApply(picked, impShelf);
      const parts = [`Добавлено: ${r.added}`];
      if (r.skipped_duplicates > 0) parts.push(`пропущено дублей: ${r.skipped_duplicates}`);
      if (r.skipped_invalid > 0) parts.push(`не удалось: ${r.skipped_invalid}`);
      const summary = parts.join(", ") + ".";
      if (r.added > 0) imp.ok(summary + " Загляни на полку.");
      else imp.fail(summary);
      if (r.problems.length > 0) imp.fail(`${summary} ${r.problems.join("; ")}`);
    } catch (e) {
      imp.fail(String(e));
    } finally {
      setImpBusy(false);
    }
  };

  const downloadCovers = async () => {
    setCoversBusy(true);
    covers.clear();
    try {
      const n = await api.coversCacheAll();
      covers.ok(
        n > 0
          ? `Скачано обложек: ${n}. Теперь они видны и без интернета.`
          : "Всё, что можно было скачать, уже лежит локально."
      );
    } catch (e) {
      covers.fail(String(e));
    } finally {
      setCoversBusy(false);
    }
  };

  const clearCache = async () => {
    cache.clear();
    try {
      const n = await api.cacheClear();
      setCached(0);
      cache.ok(n > 0 ? `Кэш очищен, удалено записей: ${n}.` : "Кэш и так был пуст.");
    } catch (e) {
      cache.fail(String(e));
    }
  };

  const stamp = () => new Date().toISOString().slice(0, 10);

  // Раньше файл «сохранялся» через blob + `<a download>`: в WKWebView на macOS
  // это ничего не создавало, но сообщение об успехе всё равно показывалось.
  const exportBackup = async () => {
    backup.clear();
    try {
      const path = await saveDialog({
        defaultPath: `knizhnik-backup-${stamp()}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return; // пользователь закрыл диалог
      await api.backupExportTo(path);
      backup.ok(`Копия сохранена: ${path}`);
    } catch (e) {
      backup.fail(`Не удалось сохранить: ${String(e)}`);
    }
  };

  const importBackup = async () => {
    backup.clear();
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (typeof picked !== "string") return;
      if (!(await dlg.confirm("Восстановить из копии? Текущий каталог (полки и книги) будет заменён.", { okLabel: "Восстановить", danger: true }))) return;
      const s = await api.backupImportFrom(picked);
      backup.ok(`Восстановлено: полок ${s.locations}, книг ${s.books}. Переключись на другие вкладки, чтобы увидеть данные.`);
    } catch (e) {
      backup.fail(`Не удалось: ${String(e)}`);
    }
  };

  const save = async () => {
    try {
      await api.setGoogleKey(key);
      note.ok("Ключ сохранён локально.");
    } catch (e) {
      note.fail(`Не удалось сохранить: ${String(e)}`);
    }
  };

  const check = async () => {
    setBusy(true);
    note.clear();
    try {
      await api.setGoogleKey(key);
      const cands = await api.metadataLookupIsbn("9785171183660");
      note.ok(
        cands.length > 0
          ? `Google отвечает — нашлась книга «${cands[0].title}». Ключ работает.`
          : "Запрос прошёл без ошибки, но книга не найдена. Ключ, похоже, рабочий."
      );
    } catch (e) {
      note.fail(`Не сработало: ${String(e)}`);
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
            <button
              className="btn btn--ghost btn--danger"
              onClick={async () => {
                try {
                  await api.setGoogleKey("");
                  setKey("");
                  note.ok("Ключ удалён.");
                } catch (e) {
                  note.fail(`Не удалось удалить: ${String(e)}`);
                }
              }}
            >
              Удалить ключ
            </button>
          )}
        </div>

        <Note note={note.note} style={{ marginTop: 10 }} />

        <p className="small muted" style={{ marginTop: 16, lineHeight: 1.6 }}>
          Где взять: <span className="mono">console.cloud.google.com</span> → создать проект →
          включить <b>Books API</b> → «Credentials» → «Create API key». Ограничь ключ на Books API.
        </p>
      </div>

      <div className="editor" style={{ marginTop: 18 }}>
        <h3>Устройство библиотеки</h3>
        <p className="small muted" style={{ marginTop: 4, marginBottom: 14, lineHeight: 1.5 }}>
          Полки заводятся там, где ты кладёшь на них книги — прямо в выборе полки.
          Сюда стоит заходить, только чтобы переименовать шкаф, перенести полку
          или разложить библиотеку по нескольким комнатам.
        </p>
        <LocationTree onOpenShelf={onOpenShelf} />
      </div>

      <div className="editor" style={{ marginTop: 18 }}>
        <h3>Импорт из Goodreads или LiveLib</h3>
        <p className="small muted" style={{ marginTop: 4, lineHeight: 1.5 }}>
          Если список книг уже где-то ведётся — выгрузи оттуда <span className="mono">.csv</span> и
          залей сюда. Это быстрее, чем сканировать: сотни книг с авторами, оценками и датами
          прочтения приезжают за один заход. Импорт <b>дополняет</b> каталог и пропускает
          книги, чей ISBN уже есть.
        </p>
        <div className="btn-row" style={{ marginTop: 14 }}>
          <span className="label muted">на полку</span>
          <ShelfSelect value={impShelf} onChange={setImpShelf} allowNone />
          <button className="btn btn--primary" onClick={importCsv} disabled={impBusy}>
            <Icon name="plus" size={16} /> {impBusy ? "Читаю…" : "Выбрать CSV"}
          </button>
        </div>
        <p className="small muted" style={{ marginTop: 12, lineHeight: 1.5 }}>
          Goodreads: <span className="mono">My Books → Import and export → Export Library</span>.
          Понимаются и запятые, и точки с запятой, и русские заголовки колонок.
        </p>
        <Note note={imp.note} style={{ marginTop: 8 }} />
      </div>

      <div className="editor" style={{ marginTop: 18 }}>
        <h3>Кэш метаданных</h3>
        <p className="small muted" style={{ marginTop: 4, lineHeight: 1.5 }}>
          Каждый ISBN запрашивается у источников <b>один раз за всё время</b> — дальше
          берётся из локального кэша. Поэтому дневной лимит Google упирается в то,
          сколько новых книг ты добавил, а не в размер библиотеки.
          Чистить стоит только если метаданные в источнике поправили.
        </p>
        <div className="btn-row" style={{ marginTop: 14 }}>
          <span className="chip mono">{cached === null ? "…" : `${cached} ISBN в кэше`}</span>
          <button className="btn btn--ghost" onClick={clearCache} disabled={cached === 0}>
            Очистить кэш
          </button>
        </div>
        <Note note={cache.note} style={{ marginTop: 8 }} />
      </div>

      <div className="editor" style={{ marginTop: 18 }}>
        <h3>Обложки</h3>
        <p className="small muted" style={{ marginTop: 4, lineHeight: 1.5 }}>
          Обложки новых книг скачиваются на компьютер сами. Эта кнопка догоняет
          те, что добавлены раньше и до сих пор подтягиваются из интернета —
          после неё полка выглядит как надо и без сети.
        </p>
        <div className="btn-row" style={{ marginTop: 14 }}>
          <button className="btn btn--brass" onClick={downloadCovers} disabled={coversBusy}>
            <Icon name="download" size={16} /> {coversBusy ? "Качаю…" : "Скачать все обложки"}
          </button>
        </div>
        <Note note={covers.note} style={{ marginTop: 8 }} />
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
          <button className="btn btn--brass" onClick={importBackup}>
            <Icon name="globe" size={16} /> Восстановить из файла
          </button>
        </div>

        <p className="small muted" style={{ marginTop: 12, lineHeight: 1.5 }}>
          Восстановление <b>заменяет</b> текущий каталог данными из файла (id и связи книга↔полка сохраняются).
        </p>
        <Note note={backup.note} style={{ marginTop: 8 }} />
      </div>

      <div className="editor" style={{ marginTop: 18 }}>
        <h3>Обновления</h3>
        <div className="btn-row" style={{ marginTop: 10 }}>
          <span className="chip mono">версия {version || "…"}</span>
          <button className="btn btn--ghost" onClick={checkUpdates} disabled={updBusy}>
            {updBusy ? "Проверяю…" : "Проверить обновления"}
          </button>
        </div>
        <Note note={upd.note} style={{ marginTop: 8 }} />
      </div>
    </div>
  );
}
