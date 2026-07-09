import type { UploadItem, UploadStatus } from "../types";
import { humanSize } from "../lib/format";
import { IconCheck, IconClose, IconUpload } from "./icons";

interface Props {
  dragging: boolean;
  items: UploadItem[];
  onCancel: () => void;
  onRetry: (idx: number) => void;
}

function StatusIcon({ s }: { s: UploadStatus }) {
  if (s === "done") return <IconCheck size={13} />;
  if (s === "error") return <IconClose size={13} />;
  if (s === "uploading") return <span className="up-spin">◠</span>;
  if (s === "cancelled") return <span>–</span>;
  return <span className="up-dot" />;
}

export function UploadOverlay({ dragging, items, onCancel, onRetry }: Props) {
  if (items.length) {
    const done = items.filter((i) => i.status === "done").length;
    const errors = items.filter((i) => i.status === "error").length;
    const active = items.some((i) => i.status === "pending" || i.status === "uploading");
    const totalProgress =
      items.length > 0
        ? Math.round(
            items.reduce((acc, it) => {
              if (it.status === "done" || it.status === "cancelled") return acc + 100;
              if (it.status === "uploading") return acc + (it.progress ?? 0);
              return acc;
            }, 0) / items.length,
          )
        : 0;

    return (
      <div className="upload-panel">
        <div className="up-head">
          <span className="up-title">
            <IconUpload size={15} /> Загрузка {done}/{items.length}
            {errors > 0 && (
              <span style={{ color: "var(--danger)", marginLeft: 6, fontSize: "0.78rem" }}>
                · {errors} ошибок
              </span>
            )}
          </span>
          {active && (
            <button className="up-cancel" onClick={onCancel}>
              Отмена
            </button>
          )}
        </div>
        {/* Overall progress bar */}
        <div className="up-overall-bar">
          <div className="up-overall-fill" style={{ width: `${totalProgress}%` }} />
        </div>
        <div className="up-list">
          {items.map((it, idx) => (
            <div className={"up-item " + it.status} key={idx} title={it.error || it.name}>
              <span className="up-status">
                <StatusIcon s={it.status} />
              </span>
              <div className="up-item-body">
                <div className="up-item-row">
                  <span className="up-name">{it.name}</span>
                  <span className="up-size">{humanSize(it.size)}</span>
                  {it.status === "error" && (
                    <button
                      className="up-retry"
                      title="Повторить загрузку"
                      onClick={() => onRetry(idx)}
                    >
                      ↺ Повторить
                    </button>
                  )}
                </div>
                {it.status === "error" && it.error && (
                  <div style={{ fontSize: "0.72rem", color: "var(--danger)", marginTop: 3 }}>
                    {it.error}
                  </div>
                )}
                {(it.status === "uploading" || it.status === "done") && (
                  <div className="up-file-bar">
                    <div
                      className={"up-file-fill" + (it.status === "done" ? " done" : "")}
                      style={{ width: `${it.status === "done" ? 100 : (it.progress ?? 0)}%` }}
                    />
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
    );
  }
  if (dragging) {
    return (
      <div className="drop-overlay">
        <div className="drop-inner">
          <IconUpload size={40} />
          <div>Отпустите файлы для загрузки</div>
        </div>
      </div>
    );
  }
  return null;
}
