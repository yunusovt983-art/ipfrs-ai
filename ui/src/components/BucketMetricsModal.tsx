import { useMemo, useRef } from "react";
import { listObjects } from "../lib/buckets";
import { fileCategory, humanSize, relTime } from "../lib/format";
import { IconClose, IconDownload, IconUpload } from "./icons";

interface Props {
  bucket: string;
  onClose: () => void;
  onExport: (bucket: string) => void;
  onImport: (bucket: string, file: File) => void;
}

const CAT_LABEL: Record<string, string> = {
  image: "Изображения",
  code: "Код",
  data: "Данные",
  model: "Модели",
  doc: "Документы",
  archive: "Архивы",
  file: "Прочее",
};

const SIZE_BUCKETS: [string, (n: number) => boolean][] = [
  ["< 1 МБ", (n) => n < 1e6],
  ["1–100 МБ", (n) => n >= 1e6 && n < 1e8],
  ["100 МБ – 1 ГБ", (n) => n >= 1e8 && n < 1e9],
  ["> 1 ГБ", (n) => n >= 1e9],
];

export function BucketMetricsModal({ bucket, onClose, onExport, onImport }: Props) {
  const importInput = useRef<HTMLInputElement>(null);

  const m = useMemo(() => {
    const objs = listObjects(bucket).filter((o) => !o.key.endsWith("/.keep"));
    const totalSize = objs.reduce((s, o) => s + o.size, 0);
    const pinned = objs.filter((o) => o.pinned).length;

    const byCat = new Map<string, { count: number; size: number }>();
    for (const o of objs) {
      const cat = fileCategory(o.key.split("/").pop() ?? "", o.contentType);
      const agg = byCat.get(cat) ?? { count: 0, size: 0 };
      agg.count++;
      agg.size += o.size;
      byCat.set(cat, agg);
    }
    const cats = [...byCat.entries()].sort((a, b) => b[1].size - a[1].size);

    const hist = SIZE_BUCKETS.map(([label, pred]) => ({
      label,
      count: objs.filter((o) => pred(o.size)).length,
    }));

    const top = [...objs].sort((a, b) => b.size - a.size).slice(0, 5);
    return { count: objs.length, totalSize, pinned, cats, hist, top };
  }, [bucket]);

  const maxCatSize = Math.max(1, ...m.cats.map(([, a]) => a.size));
  const maxHist = Math.max(1, ...m.hist.map((h) => h.count));

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>
            Метрики · <span className="mono">{bucket}</span>
          </h3>
          <button className="icon-btn" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>
        <div className="modal-body">
          <div className="metrics-row">
            <div className="metric-tile"><div className="mt-num">{m.count}</div><div className="mt-lbl">объектов</div></div>
            <div className="metric-tile"><div className="mt-num">{humanSize(m.totalSize)}</div><div className="mt-lbl">объём</div></div>
            <div className="metric-tile"><div className="mt-num">{m.pinned}</div><div className="mt-lbl">закреплено</div></div>
          </div>

          {m.count > 0 ? (
            <>
              <div className="metric-h">По типам</div>
              <div className="cat-bars">
                {m.cats.map(([cat, agg]) => (
                  <div className="cat-bar" key={cat}>
                    <span className="cb-label">{CAT_LABEL[cat] ?? cat}</span>
                    <div className="cb-track">
                      <i className={"cat-" + cat} style={{ width: `${(agg.size / maxCatSize) * 100}%` }} />
                    </div>
                    <span className="cb-val">{agg.count} · {humanSize(agg.size)}</span>
                  </div>
                ))}
              </div>

              <div className="metric-h">Распределение по размеру</div>
              <div className="hist">
                {m.hist.map((h) => (
                  <div className="hist-col" key={h.label}>
                    <div className="hc-bar-wrap">
                      <div className="hc-bar" style={{ height: `${(h.count / maxHist) * 100}%` }} />
                    </div>
                    <div className="hc-count">{h.count}</div>
                    <div className="hc-label">{h.label}</div>
                  </div>
                ))}
              </div>

              <div className="metric-h">Крупнейшие объекты</div>
              <div className="top-list">
                {m.top.map((o) => (
                  <div className="top-row" key={o.key}>
                    <span className="tr-name mono" title={o.key}>{o.key}</span>
                    <span className="tr-time">{relTime(o.lastModified)}</span>
                    <span className="tr-size">{humanSize(o.size)}</span>
                  </div>
                ))}
              </div>
            </>
          ) : (
            <div className="insp-note">Бакет пуст.</div>
          )}
        </div>
        <div className="modal-foot">
          <button className="btn ghost" onClick={() => importInput.current?.click()}>
            <IconUpload size={15} /> Импорт манифеста
          </button>
          <button className="btn ghost" onClick={() => onExport(bucket)}>
            <IconDownload size={15} /> Экспорт манифеста
          </button>
          <button className="btn primary" onClick={onClose}>
            Готово
          </button>
        </div>
        <input
          ref={importInput}
          type="file"
          accept="application/json,.json"
          hidden
          onChange={(e) => {
            if (e.target.files?.[0]) onImport(bucket, e.target.files[0]);
            e.target.value = "";
          }}
        />
      </div>
    </div>
  );
}
