import { useEffect, useMemo, useState, type ReactNode } from "react";
import type { ConnMode, S3Object } from "../types";
import type { IpfrsClient } from "../lib/ipfrs";
import { blobCache } from "../lib/buckets";
import { fileCategory, humanSize, isImage, relTime } from "../lib/format";
import {
  IconClose,
  IconCopy,
  IconDownload,
  IconLink,
  IconTrash,
} from "./icons";

interface Props {
  object: S3Object;
  mode: ConnMode;
  client: IpfrsClient;
  onClose: () => void;
  onDownload: (o: S3Object) => void;
  onCopy: (cid: string) => void;
  onDelete: (key: string) => void;
}

export function DetailsDrawer({
  object,
  mode,
  client,
  onClose,
  onDownload,
  onCopy,
  onDelete,
}: Props) {
  const name = object.key.split("/").pop() || object.key;
  const cat = fileCategory(name, object.contentType);
  const showImg = isImage(object.contentType);

  // Preview URL: in-memory blob (demo/uploaded) or gateway path (live).
  const [imgUrl, setImgUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!showImg) {
      setImgUrl(null);
      return;
    }
    const blob = blobCache.get(object.cid);
    if (blob) {
      const url = URL.createObjectURL(blob);
      setImgUrl(url);
      return () => URL.revokeObjectURL(url);
    }
    if (mode === "live" && object.cid) {
      setImgUrl(client.ipfsUrl(object.cid));
    } else {
      setImgUrl(null);
    }
  }, [object.cid, showImg, mode, client]);

  const rows: [string, ReactNode][] = useMemo(
    () => [
      ["Ключ", <span className="mono wrap">{object.key}</span>],
      [
        "CID",
        object.cid ? (
          <button className="cid-full" onClick={() => onCopy(object.cid)} title="Копировать">
            <span className="mono wrap">{object.cid}</span>
            <IconCopy size={14} />
          </button>
        ) : (
          <span className="muted">—</span>
        ),
      ],
      ["Размер", humanSize(object.size)],
      ["Content-Type", <span className="mono">{object.contentType}</span>],
      ["Изменён", new Date(object.lastModified).toLocaleString("ru-RU")],
      ["Изменён (отн.)", relTime(object.lastModified)],
      ["Класс хранения", "IPFS (content-addressed)"],
      ["Пин", object.pinned ? "закреплён" : "не закреплён"],
    ],
    [object, onCopy],
  );

  return (
    <aside className="drawer">
      <div className="drawer-head">
        <div className={"drawer-glyph cat-" + cat} />
        <div className="drawer-title" title={name}>{name}</div>
        <button className="icon-btn" onClick={onClose} title="Закрыть">
          <IconClose size={18} />
        </button>
      </div>

      {showImg &&
        (imgUrl ? (
          <div className="preview">
            <img src={imgUrl} alt={name} />
          </div>
        ) : (
          <div className="preview placeholder">превью недоступно в демо</div>
        ))}

      <div className="drawer-meta">
        {rows.map(([k, v]) => (
          <div className="meta-row" key={k}>
            <div className="meta-k">{k}</div>
            <div className="meta-v">{v}</div>
          </div>
        ))}
      </div>

      <div className="drawer-actions">
        <button className="btn primary" onClick={() => onDownload(object)}>
          <IconDownload size={16} /> Скачать
        </button>
        <button className="btn ghost" onClick={() => onCopy(object.cid)} disabled={!object.cid}>
          <IconCopy size={16} /> CID
        </button>
        {mode === "live" && object.cid && (
          <a className="btn ghost" href={client.ipfsUrl(object.cid)} target="_blank" rel="noreferrer">
            <IconLink size={16} /> Шлюз
          </a>
        )}
        <button
          className="btn danger"
          onClick={() => {
            if (confirm(`Удалить объект «${name}»?`)) onDelete(object.key);
          }}
        >
          <IconTrash size={16} /> Удалить
        </button>
      </div>
    </aside>
  );
}
