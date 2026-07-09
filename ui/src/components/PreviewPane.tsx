import { useEffect, useState } from "react";
import type { ConnMode, S3Object } from "../types";
import type { IpfrsClient } from "../lib/ipfrs";
import { blobCache } from "../lib/buckets";
import { isImage } from "../lib/format";
import {
  decodeText,
  fetchBytes,
  humanCount,
  looksBinary,
  parseSafetensors,
  prettyJson,
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
  const isSt = name.endsWith(".safetensors") || name.endsWith(".st");
  const img = isImage(object.contentType);
  const [st, setSt] = useState<State>({ loading: true });

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
      const bytes = await fetchBytes(object, mode, client, isSt ? 1_048_576 : 262_144);
      setSt({ loading: false, bytes: bytes ?? undefined });
    })();
    return () => {
      if (revoke) URL.revokeObjectURL(revoke);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [object.cid, mode]);

  if (st.loading) return <div className="preview placeholder">загрузка превью…</div>;

  if (img) {
    return st.imgUrl ? (
      <div className="preview">
        <img src={st.imgUrl} alt={name} />
      </div>
    ) : (
      <div className="preview placeholder">превью изображения недоступно в демо</div>
    );
  }

  if (!st.bytes) {
    return (
      <div className="preview placeholder">
        содержимое недоступно{mode === "demo" ? " (демо-объект без байтов)" : ""}
      </div>
    );
  }

  if (isSt) {
    const info = parseSafetensors(st.bytes);
    if (info) {
      return (
        <div className="preview">
          <SafetensorsView info={info} />
        </div>
      );
    }
  }

  if (!looksBinary(st.bytes)) {
    const text = decodeText(st.bytes);
    const isJson = object.contentType.includes("json") || name.endsWith(".json");
    const shown = (isJson ? prettyJson(text) : text).slice(0, 20_000);
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
