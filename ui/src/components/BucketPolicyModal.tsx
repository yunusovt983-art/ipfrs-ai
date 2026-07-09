import { useState } from "react";
import { getPolicy, savePolicy } from "../lib/buckets";
import { humanSize } from "../lib/format";
import { IconClose } from "./icons";

interface Props {
  bucket: string;
  usedBytes: number;
  onClose: () => void;
  onSaved: () => void;
}

const DEFAULTS = { versioning: false, autopin: false, quotaBytes: 0 };

type QuotaUnit = "mb" | "gb";

export function BucketPolicyModal({ bucket, usedBytes, onClose, onSaved }: Props) {
  const initial = getPolicy(bucket);
  const [versioning, setVersioning] = useState(initial.versioning);
  const [autopin, setAutopin] = useState(initial.autopin);
  const [unit, setUnit] = useState<QuotaUnit>("gb");

  // Convert stored bytes → current unit for display
  const toUnit = (bytes: number) => (unit === "gb" ? bytes / 1e9 : bytes / 1e6);
  const fromUnit = (val: number) => (unit === "gb" ? val * 1e9 : val * 1e6);

  const [quotaVal, setQuotaVal] = useState<number>(() =>
    initial.quotaBytes ? toUnit(initial.quotaBytes) : 0,
  );

  const quotaBytes = Math.max(0, quotaVal) ? fromUnit(Math.max(0, quotaVal)) : 0;
  const usagePercent = quotaBytes > 0 ? Math.min(100, (usedBytes / quotaBytes) * 100) : 0;
  const nearLimit = usagePercent > 80;

  const reset = () => {
    setVersioning(DEFAULTS.versioning);
    setAutopin(DEFAULTS.autopin);
    setQuotaVal(0);
  };

  const save = () => {
    savePolicy(bucket, { versioning, autopin, quotaBytes });
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

          {/* Versioning */}
          <label className="policy-row">
            <div>
              <div className="pr-title">Версионирование</div>
              <div className="pr-sub">Хранить историю CID при перезаписи ключа</div>
              {!versioning && initial.versioning && (
                <div className="pr-warn">
                  ⚠ Существующие версии останутся, но новые не будут создаваться
                </div>
              )}
            </div>
            <input type="checkbox" checked={versioning} onChange={(e) => setVersioning(e.target.checked)} />
          </label>

          {/* Autopin */}
          <label className="policy-row">
            <div>
              <div className="pr-title">Автопиннинг</div>
              <div className="pr-sub">Закреплять новые объекты при загрузке</div>
            </div>
            <input type="checkbox" checked={autopin} onChange={(e) => setAutopin(e.target.checked)} />
          </label>

          {/* Soft quota */}
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
                value={quotaVal}
                onChange={(e) => setQuotaVal(Number(e.target.value))}
              />
              <div className="quota-unit-toggle">
                <button
                  className={"unit-btn" + (unit === "mb" ? " active" : "")}
                  onClick={() => {
                    // convert current value to new unit
                    const bytes = fromUnit(Math.max(0, quotaVal));
                    setUnit("mb");
                    setQuotaVal(bytes ? bytes / 1e6 : 0);
                  }}
                >МБ</button>
                <button
                  className={"unit-btn" + (unit === "gb" ? " active" : "")}
                  onClick={() => {
                    const bytes = fromUnit(Math.max(0, quotaVal));
                    setUnit("gb");
                    setQuotaVal(bytes ? bytes / 1e9 : 0);
                  }}
                >ГБ</button>
              </div>
            </div>
          </div>

          {/* Quota usage bar */}
          {quotaBytes > 0 && (
            <div className="quota-usage">
              <div className="qu-label">
                <span>Использовано: <strong>{humanSize(usedBytes)}</strong></span>
                <span className={nearLimit ? "qu-warn" : "qu-ok"}>
                  {usagePercent.toFixed(1)}% из {humanSize(quotaBytes)}
                </span>
              </div>
              <div className="qu-bar">
                <div
                  className={"qu-fill" + (nearLimit ? " warn" : "")}
                  style={{ width: `${usagePercent}%` }}
                />
              </div>
            </div>
          )}

        </div>
        <div className="modal-foot">
          <button className="btn ghost" onClick={reset} title="Сбросить к умолчаниям">
            Сбросить
          </button>
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
