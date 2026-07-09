import { useState } from "react";
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

interface Props {
  buckets: Bucket[];
  current: string | null;
  conn: ConnStatus;
  info: GatewayInfo | null;
  mode: ConnMode;
  onSelect: (name: string) => void;
  onCreate: (name: string) => void;
  onDelete: (name: string) => void;
  onOpenSettings: () => void;
  onToggleTheme: () => void;
}

const CONN_LABEL: Record<ConnStatus, string> = {
  unknown: "демо-режим",
  connecting: "подключение…",
  online: "онлайн",
  offline: "офлайн",
};

export function Sidebar({
  buckets,
  current,
  conn,
  info,
  mode,
  onSelect,
  onCreate,
  onDelete,
  onOpenSettings,
  onToggleTheme,
}: Props) {
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");

  const submit = () => {
    if (name.trim()) onCreate(name);
    setName("");
    setAdding(false);
  };

  const dotClass = mode === "demo" ? "demo" : conn;

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
        <button className="icon-btn" title="Создать бакет" onClick={() => setAdding((v) => !v)}>
          <IconPlus size={16} />
        </button>
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
        {buckets.map((b) => {
          const objs = listObjects(b.name).filter((o) => !o.key.endsWith("/.keep"));
          const size = objs.reduce((s, o) => s + o.size, 0);
          return (
            <div
              key={b.name}
              className={"bucket-item" + (current === b.name ? " active" : "")}
              onClick={() => onSelect(b.name)}
            >
              <IconBucket size={18} />
              <div className="bucket-meta">
                <div className="bucket-name">{b.name}</div>
                <div className="bucket-stat">
                  {objs.length} объектов · {humanSize(size)}
                </div>
              </div>
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
          );
        })}
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
