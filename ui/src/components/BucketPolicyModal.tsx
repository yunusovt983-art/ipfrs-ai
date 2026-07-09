import { useState } from "react";
import { getPolicy, savePolicy } from "../lib/buckets";
import { IconClose } from "./icons";

interface Props {
  bucket: string;
  onClose: () => void;
  onSaved: () => void;
}

export function BucketPolicyModal({ bucket, onClose, onSaved }: Props) {
  const initial = getPolicy(bucket);
  const [versioning, setVersioning] = useState(initial.versioning);
  const [autopin, setAutopin] = useState(initial.autopin);
  const [quotaGb, setQuotaGb] = useState(initial.quotaBytes ? initial.quotaBytes / 1e9 : 0);

  const save = () => {
    savePolicy(bucket, {
      versioning,
      autopin,
      quotaBytes: Math.max(0, quotaGb) * 1e9,
    });
    onSaved();
    onClose();
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>
            Политики · <span className="mono">{bucket}</span>
          </h3>
          <button className="icon-btn" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>
        <div className="modal-body">
          <label className="policy-row">
            <div>
              <div className="pr-title">Версионирование</div>
              <div className="pr-sub">Хранить историю CID при перезаписи ключа</div>
            </div>
            <input type="checkbox" checked={versioning} onChange={(e) => setVersioning(e.target.checked)} />
          </label>
          <label className="policy-row">
            <div>
              <div className="pr-title">Автопиннинг</div>
              <div className="pr-sub">Закреплять новые объекты при загрузке</div>
            </div>
            <input type="checkbox" checked={autopin} onChange={(e) => setAutopin(e.target.checked)} />
          </label>
          <div className="policy-row">
            <div>
              <div className="pr-title">Мягкая квота</div>
              <div className="pr-sub">Предупреждать при превышении (0 = без лимита)</div>
            </div>
            <div className="quota-input">
              <input
                type="number"
                min="0"
                step="1"
                value={quotaGb}
                onChange={(e) => setQuotaGb(Number(e.target.value))}
              />
              <span>ГБ</span>
            </div>
          </div>
        </div>
        <div className="modal-foot">
          <button className="btn ghost" onClick={onClose}>
            Отмена
          </button>
          <button className="btn primary" onClick={save}>
            Сохранить
          </button>
        </div>
      </div>
    </div>
  );
}
