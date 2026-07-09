import { useEffect, useState } from "react";
import type { ConnMode, S3Object } from "../types";
import type { IpfrsClient } from "../lib/ipfrs";
import { blobCache } from "../lib/buckets";
import { isImage } from "../lib/format";
import {
  decodeText,
  demoImagePlaceholder,
  fetchBytes,
  humanCount,
  looksBinary,
  parseCsv,
  parseParquet,
  parseSafetensors,
  prettyJson,
  type CsvPreview,
  type ParquetFooter,
  type SafetensorsInfo,
} from "../lib/preview";

function SafetensorsView({ info }: { info: SafetensorsInfo }) {
  return (
    <div className="st-view">
      <div className="st-summary">
        <div className="st-stat"><b>{humanCount(info.totalParams)}</b><span>параметров</span></div>
        <div className="st-stat"><b>{info.count}</b><span>тензоров</span></div>
        <div className="st-stat"><b>{info.dtypes.join(", ") || "—"}</b><span>dtype</span></div>
      </div>
      <div className="st-table">
        {info.tensors.slice(0, 14).map((t) => (
          <div className="st-row" key={t.name} title={t.name}>
            <span className="st-name mono">{t.name}</span>
            <span className="st-shape mono">[{t.shape.join("×")}]</span>
            <span className="st-dtype">{t.dtype}</span>
          </div>
        ))}
        {info.count > 14 && <div className="st-more">… ещё {info.count - 14} тензоров</div>}
      </div>
      {info.truncated && <div className="st-note">заголовок обрезан — показана часть</div>}
    </div>
  );
}

function ParquetView({ info }: { info: ParquetFooter }) {
  const REPS: Record<string, string> = { REQUIRED: "req", OPTIONAL: "opt", REPEATED: "rep" };
  return (
    <div className="pq-view">
      <div className="pq-summary">
        <div className="pq-stat">
          <b>{humanCount(info.numRows)}</b>
          <span>строк</span>
        </div>
        <div className="pq-stat">
          <b>{info.columns.length}</b>
          <span>колонок</span>
        </div>
        <div className="pq-stat">
          <b>{info.rowGroupCount}</b>
          <span>row groups</span>
        </div>
        <div className="pq-stat">
          <b>v{info.version || "?"}</b>
          <span>Parquet</span>
        </div>
      </div>
      <div className="pq-schema-head">Схема</div>
      <div className="pq-table">
        {info.columns.slice(0, 24).map((c, i) => (
          <div className="pq-row" key={c.name + i}>
            <span className="pq-col-name mono" title={c.name}>{c.name}</span>
            <span className="pq-col-type">{c.type}</span>
            <span className={"pq-col-rep pq-" + c.repetition.toLowerCase()}>{REPS[c.repetition]}</span>
          </div>
        ))}
        {info.columns.length > 24 && (
          <div className="st-more">… ещё {info.columns.length - 24} колонок</div>
        )}
        {!info.columns.length && (
          <div className="st-note">
            {info.truncated ? "файл не загружен полностью — footer обрезан" : "схема пуста"}
          </div>
        )}
      </div>
      {info.truncated && <div className="st-note">⚠ footer обрезан — часть данных недоступна</div>}
    </div>
  );
}

function CsvView({ preview }: { preview: CsvPreview }) {
  const delimLabel = preview.delimiter === "\t" ? "TSV" : preview.delimiter === ";" ? "CSV (;)" : "CSV";
  return (
    <div className="csv-view">
      <div className="csv-meta">
        <span className="csv-format">{delimLabel}</span>
        <span>{preview.headers.length} колонок · {humanCount(preview.totalLines)} строк</span>
      </div>
      <div className="csv-scroll">
        <table className="csv-table">
          <thead>
            <tr>
              {preview.headers.map((h, i) => (
                <th key={i} title={h}>{h || <em className="muted">col{i + 1}</em>}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {preview.rows.map((row, ri) => (
              <tr key={ri}>
                {preview.headers.map((_, ci) => (
                  <td key={ci} title={row[ci] ?? ""}>{row[ci] ?? ""}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {preview.totalLines > preview.rows.length && (
        <div className="csv-truncated">
          показано {preview.rows.length} из {humanCount(preview.totalLines)} строк
        </div>
      )}
    </div>
  );
}

interface State {
  loading: boolean;
  bytes?: Uint8Array;
  imgUrl?: string;
}

export function PreviewPane({
  object,
  mode,
  client,
}: {
  object: S3Object;
  mode: ConnMode;
  client: IpfrsClient;
}) {
  const name = object.key.split("/").pop() || object.key;
  const ext  = name.split(".").pop()?.toLowerCase() ?? "";
  const isSt      = ext === "safetensors" || ext === "st";
  const isParquet = ext === "parquet";
  const isCsv     = ext === "csv" || ext === "tsv" || ext === "tab";
  const img = isImage(object.contentType);
  const [st, setSt] = useState<State>({ loading: true });

  // For Parquet we need more bytes to reach the footer (~1MB is usually enough)
  const fetchLimit = isParquet ? 2_097_152 : isSt ? 1_048_576 : 262_144;

  useEffect(() => {
    let revoke: string | undefined;
    setSt({ loading: true });
    (async () => {
      if (img) {
        const blob = blobCache.get(object.cid);
        if (blob) {
          const u = URL.createObjectURL(blob);
          revoke = u;
          setSt({ loading: false, imgUrl: u });
        } else if (mode === "live" && object.cid) {
          setSt({ loading: false, imgUrl: client.ipfsUrl(object.cid) });
        } else {
          setSt({ loading: false });
        }
        return;
      }
      const bytes = await fetchBytes(object, mode, client, fetchLimit);
      setSt({ loading: false, bytes: bytes ?? undefined });
    })();
    return () => { if (revoke) URL.revokeObjectURL(revoke); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [object.cid, mode]);

  if (st.loading) return <div className="preview placeholder">загрузка превью…</div>;

  if (img) {
    return (
      <div className="preview">
        <img src={st.imgUrl ?? demoImagePlaceholder(name)} alt={name} />
        {!st.imgUrl && <div className="preview-tag">демо-превью</div>}
      </div>
    );
  }

  if (!st.bytes) {
    return (
      <div className="preview placeholder">
        содержимое недоступно{mode === "demo" ? " (демо-объект без байтов)" : ""}
      </div>
    );
  }

  // ── Parquet ──
  if (isParquet) {
    const info = parseParquet(st.bytes);
    if (info) return <div className="preview"><ParquetView info={info} /></div>;
    return <div className="preview placeholder">не удалось разобрать Parquet footer</div>;
  }

  // ── Safetensors ──
  if (isSt) {
    const info = parseSafetensors(st.bytes);
    if (info) return <div className="preview"><SafetensorsView info={info} /></div>;
  }

  // ── Text / JSON / CSV ──
  if (!looksBinary(st.bytes)) {
    const text = decodeText(st.bytes);

    if (isCsv) {
      const preview = parseCsv(text, 50);
      return <div className="preview"><CsvView preview={preview} /></div>;
    }

    const isJson = object.contentType.includes("json") || name.endsWith(".json");
    const shown  = (isJson ? prettyJson(text) : text).slice(0, 20_000);
    return (
      <div className="preview">
        <pre className="code-preview">
          {shown}
          {text.length > 20_000 ? "\n… (обрезано)" : ""}
        </pre>
      </div>
    );
  }

  return <div className="preview placeholder">двоичные данные — откройте инспектор блока</div>;
}
