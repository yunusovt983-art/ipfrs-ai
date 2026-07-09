import { useMemo, useState } from "react";
import type { Bucket, ConnMode, ConnStatus, GatewayInfo } from "../types";
import { listObjects } from "../lib/buckets";
import { humanSize } from "../lib/format";
import {
  IconBucket,
  IconGear,
  IconLogo,
  IconPlus,
  IconTrash,
} from "./icons";

type SortMode = "name" | "date" | "size";

interface Props {
  buckets: Bucket[];
  current: string | null;
  conn: ConnStatus;
  info: GatewayInfo | null;
  mode: ConnMode;
  onSelect: (name: string) => void;
  onCreate: (name: string) => void;
  onDelete: (name: string) => void;
  onPolicy: (name: string) => void;
  onOpenSettings: () => void;
  onToggleTheme: () => void;
}

const CONN_LABEL: Record<ConnStatus, string> = {
  unknown: "демо-режим",
  connecting: "подключение…",
  online: "онлайн",
  offline: "офлайн",
};

const SORT_LABELS: Record<SortMode, string> = { name: "A–Z", date: "Дата", size: "Размер" };

export function Sidebar({
  buckets,
  current,
  conn,
  info,
  mode,
  onSelect,
  onCreate,
  onDelete,
  onPolicy,
  onOpenSettings,
  onToggleTheme,
}: Props) {
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [sortMode, setSortMode] = useState<SortMode>("name");
  const [sortDir, setSortDir] = useState<1 | -1>(1);

  const submit = () => {
    if (name.trim()) onCreate(name);
    setName("");
    setAdding(false);
  };

  const dotClass = mode === "demo" ? "demo" : conn;

  /** Enrich each bucket with stats, then sort. */
  const enriched = useMemo(() => {
    return buckets.map((b) => {
      const objs = listObjects(b.name).filter((o) => !o.key.endsWith("/.keep"));
      const size = objs.reduce((s, o) => s + o.size, 0);
      return { ...b, objCount: objs.length, totalSize: size };
    });
  }, [buckets]);

  const sorted = useMemo(() => {
    const arr = [...enriched];
    arr.sort((a, b) => {
      let v = 0;
      if (sortMode === "name") v = a.name.localeCompare(b.name, "ru");
      else if (sortMode === "date") v = a.createdAt - b.createdAt;
      else if (sortMode === "size") v = a.totalSize - b.totalSize;
      return v * sortDir;
    });
    return arr;
  }, [enriched, sortMode, sortDir]);

  const cycleSort = (mode: SortMode) => {
    if (sortMode === mode) {
      setSortDir((d) => (d === 1 ? -1 : 1));
    } else {
      setSortMode(mode);
      setSortDir(1);
    }
  };

  return (
    <aside className="sidebar">
      <div className="brand">
        <IconLogo size={24} />
        <div>
          <div className="brand-name">IPFRS</div>
          <div className="brand-sub">S3 Console</div>
        </div>
      </div>

      <div className="side-head">
        <span>Бакеты</span>
        <div className="side-head-actions">
          {(["name", "date", "size"] as SortMode[]).map((m) => (
            <button
              key={m}
              className={"sort-pill" + (sortMode === m ? " active" : "")}
              title={`Сортировать по: ${SORT_LABELS[m]}`}
              onClick={() => cycleSort(m)}
            >
              {SORT_LABELS[m]}
              {sortMode === m && <span className="sort-arrow">{sortDir === 1 ? "↑" : "↓"}</span>}
            </button>
          ))}
          <button className="icon-btn" title="Создать бакет" onClick={() => setAdding((v) => !v)}>
            <IconPlus size={16} />
          </button>
        </div>
      </div>

      {adding && (
        <div className="new-bucket">
          <input
            autoFocus
            placeholder="имя-бакета"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") submit();
              if (e.key === "Escape") {
                setAdding(false);
                setName("");
              }
            }}
          />
          <button className="mini-btn" onClick={submit}>
            OK
          </button>
        </div>
      )}

      <nav className="bucket-list">
        {sorted.map((b) => (
          <div
            key={b.name}
            className={"bucket-item" + (current === b.name ? " active" : "")}
            onClick={() => onSelect(b.name)}
          >
            <IconBucket size={18} />
            <div className="bucket-meta">
              <div className="bucket-name">{b.name}</div>
              <div className="bucket-stat">
                {b.objCount} объектов · {humanSize(b.totalSize)}
              </div>
            </div>
            <button
              className="icon-btn ghost del"
              title="Политики бакета"
              onClick={(e) => {
                e.stopPropagation();
                onPolicy(b.name);
              }}
            >
              <IconGear size={14} />
            </button>
            <button
              className="icon-btn ghost del"
              title="Удалить бакет"
              onClick={(e) => {
                e.stopPropagation();
                if (confirm(`Удалить бакет «${b.name}» и все его объекты?`)) onDelete(b.name);
              }}
            >
              <IconTrash size={15} />
            </button>
          </div>
        ))}
        {!buckets.length && <div className="side-empty">пока нет бакетов</div>}
      </nav>

      <div className="side-foot">
        <button className="conn" onClick={onOpenSettings} title="Настройки подключения">
          <span className={"conn-dot " + dotClass} />
          <div className="conn-text">
            <div>{mode === "live" ? CONN_LABEL[conn] : "демо-режим"}</div>
            <div className="conn-sub">
              {mode === "live" && conn === "online"
                ? `v${info?.version ?? "?"} · пиров: ${info?.peers ?? 0}`
                : mode === "live"
                  ? "IPFRS gateway"
                  : "локальный манифест"}
            </div>
          </div>
        </button>
        <div className="side-actions">
          <button className="icon-btn" title="Тема" onClick={onToggleTheme}>
            ◐
          </button>
          <button className="icon-btn" title="Настройки" onClick={onOpenSettings}>
            <IconGear size={17} />
          </button>
        </div>
      </div>
    </aside>
  );
}
