import { useState } from "react";
import { humanSize } from "../lib/format";
import {
  IconChevron,
  IconFolder,
  IconRefresh,
  IconSearch,
  IconUpload,
} from "./icons";

interface Props {
  bucket: string;
  prefix: string;
  query: string;
  view: "list" | "grid";
  smart: boolean;
  stats: { count: number; size: number };
  onNavigate: (prefix: string) => void;
  onQuery: (q: string) => void;
  onSmart: (v: boolean) => void;
  onUpload: () => void;
  onNewFolder: (name: string) => void;
  onRefresh: () => void;
  onView: (v: "list" | "grid") => void;
}

export function Toolbar({
  bucket,
  prefix,
  query,
  view,
  smart,
  stats,
  onNavigate,
  onQuery,
  onSmart,
  onUpload,
  onNewFolder,
  onRefresh,
  onView,
}: Props) {
  const [newFolder, setNewFolder] = useState(false);
  const [folderName, setFolderName] = useState("");

  const segments = prefix ? prefix.replace(/\/$/, "").split("/") : [];

  const crumbTo = (idx: number) => segments.slice(0, idx + 1).join("/") + "/";

  const submitFolder = () => {
    if (folderName.trim()) onNewFolder(folderName);
    setFolderName("");
    setNewFolder(false);
  };

  return (
    <div className="toolbar">
      <div className="crumbs">
        <button className="crumb root" onClick={() => onNavigate("")}>
          {bucket}
        </button>
        {segments.map((seg, i) => (
          <span key={i} className="crumb-wrap">
            <IconChevron size={14} className="crumb-sep" />
            <button
              className="crumb"
              onClick={() => onNavigate(crumbTo(i))}
              disabled={i === segments.length - 1}
            >
              {seg}
            </button>
          </span>
        ))}
        <span className="crumb-count">
          {stats.count} объектов · {humanSize(stats.size)}
        </span>
      </div>

      <div className="tools">
        <div className={"search" + (smart ? " smart" : "")}>
          <IconSearch size={16} />
          <input
            placeholder={smart ? "Умный поиск по содержимому…" : "Поиск объектов…"}
            value={query}
            onChange={(e) => onQuery(e.target.value)}
          />
          <button
            className={"smart-toggle" + (smart ? " on" : "")}
            title="Умный поиск (по содержимому и метаданным)"
            onClick={() => onSmart(!smart)}
          >
            ✦ Умный
          </button>
        </div>

        {newFolder ? (
          <div className="new-folder">
            <input
              autoFocus
              placeholder="имя папки"
              value={folderName}
              onChange={(e) => setFolderName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitFolder();
                if (e.key === "Escape") {
                  setNewFolder(false);
                  setFolderName("");
                }
              }}
            />
            <button className="mini-btn" onClick={submitFolder}>
              OK
            </button>
          </div>
        ) : (
          <button className="btn ghost" onClick={() => setNewFolder(true)}>
            <IconFolder size={16} /> Папка
          </button>
        )}

        <button className="icon-btn" title="Обновить" onClick={onRefresh}>
          <IconRefresh size={17} />
        </button>

        <div className="view-toggle">
          <button
            className={view === "list" ? "active" : ""}
            onClick={() => onView("list")}
            title="Список"
          >
            ☰
          </button>
          <button
            className={view === "grid" ? "active" : ""}
            onClick={() => onView("grid")}
            title="Сетка"
          >
            ▦
          </button>
        </div>

        <button className="btn primary" onClick={onUpload}>
          <IconUpload size={16} /> Загрузить
        </button>
      </div>
    </div>
  );
}
