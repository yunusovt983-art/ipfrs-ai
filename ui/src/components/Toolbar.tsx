import { useEffect, useRef, useState } from "react";
import { humanSize } from "../lib/format";
import {
  IconChart,
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
  semantic: boolean;
  stats: { count: number; size: number };
  canBack: boolean;
  canForward: boolean;
  foldersAt: (prefix: string) => string[];
  onBack: () => void;
  onForward: () => void;
  onNavigate: (prefix: string) => void;
  onQuery: (q: string) => void;
  onSmart: (v: boolean) => void;
  onSemantic: (v: boolean) => void;
  onUpload: () => void;
  onNewFolder: (name: string) => void;
  onRefresh: () => void;
  onMetrics: () => void;
  onView: (v: "list" | "grid") => void;
}

export function Toolbar({
  bucket,
  prefix,
  query,
  view,
  smart,
  semantic,
  stats,
  canBack,
  canForward,
  foldersAt,
  onBack,
  onForward,
  onNavigate,
  onQuery,
  onSmart,
  onSemantic,
  onUpload,
  onNewFolder,
  onRefresh,
  onMetrics,
  onView,
}: Props) {
  const [newFolder, setNewFolder] = useState(false);
  const [folderName, setFolderName] = useState("");
  const [openCrumb, setOpenCrumb] = useState<number | null>(null);
  const crumbsRef = useRef<HTMLDivElement>(null);

  const segments = prefix ? prefix.replace(/\/$/, "").split("/") : [];
  const crumbTo = (idx: number) => segments.slice(0, idx + 1).join("/") + "/";
  const parentOf = (idx: number) => (idx === 0 ? "" : segments.slice(0, idx).join("/") + "/");

  const submitFolder = () => {
    if (folderName.trim()) onNewFolder(folderName);
    setFolderName("");
    setNewFolder(false);
  };

  // Close dropdown on outside click
  useEffect(() => {
    if (openCrumb === null) return;
    const handler = (e: MouseEvent) => {
      if (crumbsRef.current && !crumbsRef.current.contains(e.target as Node)) {
        setOpenCrumb(null);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [openCrumb]);

  // Dropdown of sibling folders for crumb `idx` (-1 = bucket root).
  const Dropdown = ({ idx }: { idx: number }) => {
    const parent = idx < 0 ? "" : parentOf(idx);
    const current = idx < 0 ? "" : segments[idx];
    const sibs = foldersAt(parent).filter((f) => f !== current);
    if (!sibs.length) return null;
    return (
      <div className="crumb-menu" role="menu">
        {sibs.map((f) => (
          <button
            key={f}
            role="menuitem"
            onClick={() => {
              onNavigate(parent + f + "/");
              setOpenCrumb(null);
            }}
          >
            <IconFolder size={14} /> {f}
          </button>
        ))}
      </div>
    );
  };

  return (
    <div className="toolbar">
      <div className="crumbs" ref={crumbsRef}>
        <div className="navhist">
          <button className="icon-btn" title="Назад" disabled={!canBack} onClick={onBack}>
            <IconChevron size={16} style={{ transform: "rotate(180deg)" }} />
          </button>
          <button className="icon-btn" title="Вперёд" disabled={!canForward} onClick={onForward}>
            <IconChevron size={16} />
          </button>
        </div>

        <span className="crumb-wrap">
          <button className="crumb root" onClick={() => onNavigate("")}>
            {bucket}
          </button>
          {foldersAt("").filter((f) => f !== segments[0]).length > 0 && (
            <button
              className={"crumb-caret" + (openCrumb === -1 ? " open" : "")}
              onClick={() => setOpenCrumb((v) => (v === -1 ? null : -1))}
              aria-expanded={openCrumb === -1}
            >
              <IconChevron size={12} style={{ transform: "rotate(90deg)" }} />
            </button>
          )}
          {openCrumb === -1 && <Dropdown idx={-1} />}
        </span>

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
            {foldersAt(parentOf(i)).filter((f) => f !== seg).length > 0 && (
              <button
                className={"crumb-caret" + (openCrumb === i ? " open" : "")}
                onClick={() => setOpenCrumb((v) => (v === i ? null : i))}
                aria-expanded={openCrumb === i}
              >
                <IconChevron size={12} style={{ transform: "rotate(90deg)" }} />
              </button>
            )}
            {openCrumb === i && <Dropdown idx={i} />}
          </span>
        ))}

        <span className="crumb-count">
          {stats.count} объектов · {humanSize(stats.size)}
        </span>
      </div>

      <div className="tools">
        <div className={"search" + (smart ? " smart" : "") + (semantic ? " semantic" : "")}>
          <IconSearch size={16} />
          <input
            placeholder={semantic ? "Семантический поиск…" : smart ? "Умный поиск по содержимому…" : "Поиск объектов…"}
            value={query}
            onChange={(e) => onQuery(e.target.value)}
          />
          <div className="search-toggles">
            <button
              className={"smart-toggle" + (smart ? " on" : "")}
              title="Умный поиск (лексика + n-gram по содержимому)"
              onClick={() => { onSmart(!smart); if (!smart) onSemantic(false); }}
            >
              ✦ Smart
            </button>
            <button
              className={"smart-toggle vec-toggle" + (semantic ? " on" : "")}
              title="Семантический поиск (vector embedding)"
              onClick={() => { onSemantic(!semantic); if (!semantic) onSmart(false); }}
            >
              ⬡ Vector
            </button>
          </div>
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

        <button className="icon-btn" title="Метрики бакета" onClick={onMetrics}>
          <IconChart size={17} />
        </button>

        <button className="icon-btn" title="Обновить" onClick={onRefresh}>
          <IconRefresh size={17} />
        </button>

        <div className="view-toggle">
          <button className={view === "list" ? "active" : ""} onClick={() => onView("list")} title="Список">
            ☰
          </button>
          <button className={view === "grid" ? "active" : ""} onClick={() => onView("grid")} title="Сетка">
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
