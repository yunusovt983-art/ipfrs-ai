import type { UploadItem, UploadStatus } from "../types";
import { humanSize } from "../lib/format";
import { IconCheck, IconClose, IconUpload } from "./icons";

interface Props {
  dragging: boolean;
  items: UploadItem[];
  onCancel: () => void;
}

function StatusIcon({ s }: { s: UploadStatus }) {
  if (s === "done") return <IconCheck size={13} />;
  if (s === "error") return <IconClose size={13} />;
  if (s === "uploading") return <span className="up-spin">◠</span>;
  if (s === "cancelled") return <span>–</span>;
  return <span className="up-dot" />;
}

export function UploadOverlay({ dragging, items, onCancel }: Props) {
  if (items.length) {
    const done = items.filter((i) => i.status === "done").length;
    const active = items.some((i) => i.status === "pending" || i.status === "uploading");
    return (
      <div className="upload-panel">
        <div className="up-head">
          <span className="up-title">
            <IconUpload size={15} /> Загрузка {done}/{items.length}
          </span>
          {active && (
            <button className="up-cancel" onClick={onCancel}>
              Отмена
            </button>
          )}
        </div>
        <div className="up-list">
          {items.map((it, idx) => (
            <div className={"up-item " + it.status} key={idx} title={it.error || it.name}>
              <span className="up-status">
                <StatusIcon s={it.status} />
              </span>
              <span className="up-name">{it.name}</span>
              <span className="up-size">{humanSize(it.size)}</span>
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
