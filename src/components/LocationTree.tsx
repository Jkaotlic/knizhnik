import { useEffect, useState } from "react";
import { api, Location, Kind } from "../api";

const childKind: Record<Kind, Kind | null> = {
  root: "room",
  room: "bookcase",
  bookcase: "shelf",
  shelf: null,
};
const kindLabel: Record<Kind, string> = {
  root: "Дом",
  room: "Комната",
  bookcase: "Шкаф",
  shelf: "Полка",
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

  const reload = () => api.locationsAll().then(setLocs).catch((e) => setError(String(e)));
  useEffect(() => { reload(); }, []);

  const roots = locs.filter((l) => l.parent_id === null);
  const childrenOf = (id: number) => locs.filter((l) => l.parent_id === id);

  const addChild = async (parent: Location) => {
    const kind = childKind[parent.kind];
    if (!kind) return;
    const name = prompt(`Название (${kindLabel[kind]})`);
    if (!name) return;
    const label = kind === "shelf" ? prompt("Код полки (необязательно)") : null;
    await api.locationCreate(parent.id, name, kind, label || null);
    reload();
  };

  const addRoot = async () => {
    const name = prompt("Название дома/корня");
    if (!name) return;
    await api.locationCreate(null, name, "root", null);
    reload();
  };

  const del = async (l: Location) => {
    try {
      await api.locationDelete(l.id);
      reload();
    } catch (e) {
      alert(String(e));
    }
  };

  const rename = async (l: Location) => {
    const newName = prompt("Новое название", l.name);
    if (!newName || newName === l.name) return;
    try {
      await api.locationUpdate(l.id, newName, null);
      reload();
    } catch (e) {
      alert(String(e));
    }
  };

  const moveCandidates = (l: Location) => {
    const pk = parentKind[l.kind];
    if (!pk) return [];
    return locs.filter((c) => c.kind === pk && c.id !== l.parent_id && c.id !== l.id);
  };

  const startMove = (l: Location) => {
    const candidates = moveCandidates(l);
    if (candidates.length === 0) {
      alert("Нет подходящих родителей");
      return;
    }
    setMovingId(l.id);
  };

  const move = async (l: Location, newParentId: number) => {
    try {
      await api.locationMove(l.id, newParentId);
      setMovingId(null);
      reload();
    } catch (e) {
      alert(String(e));
    }
  };

  const node = (l: Location) => (
    <div key={l.id} className="tree-node">
      <div className="row">
        <span>{kindLabel[l.kind]}: {l.name}{l.label ? ` [${l.label}]` : ""}</span>
        {l.kind === "shelf" && <button onClick={() => onOpenShelf(l.id)}>Открыть</button>}
        {childKind[l.kind] && <button onClick={() => addChild(l)}>+ {kindLabel[childKind[l.kind]!]}</button>}
        <button onClick={() => rename(l)}>Переименовать</button>
        {parentKind[l.kind] && <button onClick={() => startMove(l)}>Перенести</button>}
        <button onClick={() => del(l)}>Удалить</button>
        {movingId === l.id && (
          <select
            defaultValue=""
            onChange={(e) => e.target.value && move(l, Number(e.target.value))}
          >
            <option value="" disabled>Куда?</option>
            {moveCandidates(l).map((c) => (
              <option key={c.id} value={c.id}>{c.name}</option>
            ))}
          </select>
        )}
      </div>
      {childrenOf(l.id).map(node)}
    </div>
  );

  return (
    <div>
      <div className="row">
        <h2>Локации</h2>
        <button onClick={addRoot}>+ Дом</button>
      </div>
      {error && <p className="muted">{error}</p>}
      {roots.map(node)}
    </div>
  );
}
