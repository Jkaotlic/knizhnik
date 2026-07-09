import { useEffect, useState } from "react";
import { api, Location, Kind } from "../api";
import { kindLabel } from "../theme";
import { Icon } from "./Icon";
import { useDialog } from "./Dialog";

const childKind: Record<Kind, Kind | null> = {
  root: "room",
  room: "bookcase",
  bookcase: "shelf",
  shelf: null,
};
const parentKind: Record<Kind, Kind | null> = {
  root: null,
  room: "root",
  bookcase: "room",
  shelf: "bookcase",
};

export function LocationTree({ onOpenShelf }: { onOpenShelf: (id: number) => void }) {
  const [locs, setLocs] = useState<Location[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [movingId, setMovingId] = useState<number | null>(null);
  const dlg = useDialog();

  const reload = () => api.locationsAll().then(setLocs).catch((e) => setError(String(e)));
  useEffect(() => { reload(); }, []);

  const roots = locs.filter((l) => l.parent_id === null);
  const childrenOf = (id: number) => locs.filter((l) => l.parent_id === id);

  const addChild = async (parent: Location) => {
    const kind = childKind[parent.kind];
    if (!kind) return;
    const name = await dlg.prompt(`Название · ${kindLabel[kind]}`);
    if (!name) return;
    const label = kind === "shelf" ? await dlg.prompt("Код полки, напр. A-3 (необязательно)") : null;
    await api.locationCreate(parent.id, name, kind, label || null);
    reload();
  };

  const addRoot = async () => {
    const name = await dlg.prompt("Название дома / корня", { placeholder: "напр. Квартира" });
    if (!name) return;
    await api.locationCreate(null, name, "root", null);
    reload();
  };

  const del = async (l: Location) => {
    try { await api.locationDelete(l.id); reload(); } catch (e) { dlg.alert(String(e)); }
  };

  const rename = async (l: Location) => {
    const newName = await dlg.prompt("Новое название", { defaultValue: l.name });
    if (!newName || newName === l.name) return;
    try { await api.locationUpdate(l.id, newName, null); reload(); } catch (e) { dlg.alert(String(e)); }
  };

  const moveCandidates = (l: Location) => {
    const pk = parentKind[l.kind];
    if (!pk) return [];
    return locs.filter((c) => c.kind === pk && c.id !== l.parent_id && c.id !== l.id);
  };

  const startMove = (l: Location) => {
    if (moveCandidates(l).length === 0) { dlg.alert("Нет подходящих родителей"); return; }
    setMovingId((cur) => (cur === l.id ? null : l.id));
  };

  const move = async (l: Location, newParentId: number) => {
    try { await api.locationMove(l.id, newParentId); setMovingId(null); reload(); } catch (e) { dlg.alert(String(e)); }
  };

  const node = (l: Location) => (
    <div key={l.id} className="tree__node">
      <div className="tree__row">
        <span className="tree__kind">{kindLabel[l.kind]}</span>
        <span className="tree__name">{l.name}</span>
        {l.label && <span className="shelf-tag">{l.label}</span>}

        <span className="tree__tools">
          {l.kind === "shelf" && (
            <button className="btn btn--ghost btn--sm" onClick={() => onOpenShelf(l.id)}><Icon name="shelf" size={15} /> Открыть</button>
          )}
          {childKind[l.kind] && (
            <button className="btn btn--ghost btn--sm" onClick={() => addChild(l)} title={`Добавить ${kindLabel[childKind[l.kind]!].toLowerCase()}`}>
              <Icon name="plus" size={15} /> {kindLabel[childKind[l.kind]!]}
            </button>
          )}
          <button className="btn btn--ghost btn--sm" onClick={() => rename(l)} title="Переименовать"><Icon name="pencil" size={15} /></button>
          {parentKind[l.kind] && (
            <button className="btn btn--ghost btn--sm" onClick={() => startMove(l)} title="Перенести"><Icon name="move" size={15} /></button>
          )}
          <button className="btn btn--ghost btn--sm btn--danger" onClick={() => del(l)} title="Удалить"><Icon name="trash" size={15} /></button>
        </span>

        {movingId === l.id && (
          <select className="select" defaultValue="" onChange={(e) => e.target.value && move(l, Number(e.target.value))}>
            <option value="" disabled>Перенести в…</option>
            {moveCandidates(l).map((c) => (
              <option key={c.id} value={c.id}>{c.name}</option>
            ))}
          </select>
        )}
      </div>
      {childrenOf(l.id).length > 0 && <div className="tree__kids">{childrenOf(l.id).map(node)}</div>}
    </div>
  );

  return (
    <div>
      <div className="page-head">
        <div>
          <div className="eyebrow">Локации</div>
          <h2 className="page-title">Комнаты, шкафы, полки</h2>
        </div>
        <button className="btn btn--primary" onClick={addRoot}><Icon name="plus" size={16} /> Дом</button>
      </div>
      {error && <p className="error-note">{error}</p>}
      {roots.length === 0 && <div className="empty">Пока пусто. Начни с «+ Дом», затем комнаты, шкафы и полки.</div>}
      <div className="tree">{roots.map(node)}</div>
    </div>
  );
}
