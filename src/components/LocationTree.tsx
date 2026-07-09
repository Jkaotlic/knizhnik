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

export function LocationTree({ onOpenShelf }: { onOpenShelf: (id: number) => void }) {
  const [locs, setLocs] = useState<Location[]>([]);
  const [error, setError] = useState<string | null>(null);

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

  const node = (l: Location) => (
    <div key={l.id} className="tree-node">
      <div className="row">
        <span>{kindLabel[l.kind]}: {l.name}{l.label ? ` [${l.label}]` : ""}</span>
        {l.kind === "shelf" && <button onClick={() => onOpenShelf(l.id)}>Открыть</button>}
        {childKind[l.kind] && <button onClick={() => addChild(l)}>+ {kindLabel[childKind[l.kind]!]}</button>}
        <button onClick={() => del(l)}>Удалить</button>
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
