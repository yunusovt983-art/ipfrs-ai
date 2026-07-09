import type { ActivityEntry, ActivityKind } from "../lib/activity";
import { relTime } from "../lib/format";
import { IconClose } from "./icons";

const KIND_META: Record<ActivityKind, { label: string; cls: string }> = {
  upload: { label: "загрузка", cls: "up" },
  delete: { label: "удаление", cls: "del" },
  bulkDelete: { label: "массовое удаление", cls: "del" },
  deleteBucket: { label: "удаление бакета", cls: "del" },
  rename: { label: "переименование", cls: "edit" },
  restore: { label: "восстановление версии", cls: "ok" },
  pin: { label: "пиннинг", cls: "ok" },
  createBucket: { label: "создание бакета", cls: "ok" },
  createFolder: { label: "создание папки", cls: "ok" },
  import: { label: "импорт манифеста", cls: "ok" },
};

interface Props {
  entries: ActivityEntry[];
  onUndo: (entry: ActivityEntry) => void;
  onClear: () => void;
  onClose: () => void;
}

export function ActivityPanel({ entries, onUndo, onClear, onClose }: Props) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>Журнал операций</h3>
          <button className="icon-btn" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>
        <div className="modal-body">
          {entries.length === 0 ? (
            <div className="insp-note">Пока нет операций.</div>
          ) : (
            <div className="act-list">
              {entries.map((e) => {
                const meta = KIND_META[e.kind];
                return (
                  <div className="act-row" key={e.id}>
                    <span className={"act-dot " + meta.cls} />
                    <div className="act-body">
                      <div className="act-summary">{e.summary}</div>
                      <div className="act-meta">
                        <span className="act-kind">{meta.label}</span>
                        <span className="act-bucket mono">{e.bucket}</span>
                        <span className="act-time">{relTime(e.ts)}</span>
                      </div>
                    </div>
                    {e.undo && (
                      <button className="mini-btn" onClick={() => onUndo(e)}>
                        ↺ Отменить
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
        {entries.length > 0 && (
          <div className="modal-foot">
            <button className="btn ghost" onClick={onClear}>
              Очистить журнал
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
