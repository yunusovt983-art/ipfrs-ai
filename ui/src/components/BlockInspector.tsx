import { useEffect, useState } from "react";
import type { ConnMode, S3Object } from "../types";
import type { IpfrsClient } from "../lib/ipfrs";
import { blobCache } from "../lib/buckets";
import { humanSize } from "../lib/format";
import { detectFormat, hexDump, scanCidLinks } from "../lib/inspect";
import { IconClose, IconCopy } from "./icons";

interface Props {
  object: S3Object;
  mode: ConnMode;
  client: IpfrsClient;
  onClose: () => void;
  onCopy: (cid: string) => void;
}

interface State {
  loading: boolean;
  bytes?: Uint8Array;
  err?: string;
  source?: string;
}

export function BlockInspector({ object, mode, client, onClose, onCopy }: Props) {
  const [st, setSt] = useState<State>({ loading: true });

  useEffect(() => {
    setSt({ loading: true });
    (async () => {
      const blob = blobCache.get(object.cid);
      if (blob) {
        const buf = await blob.slice(0, 65_536).arrayBuffer();
        setSt({ loading: false, bytes: new Uint8Array(buf), source: "локальный кэш" });
        return;
      }
      if (mode === "live" && object.cid) {
        try {
          const bytes = await client.dagGet(object.cid);
          setSt({ loading: false, bytes: bytes.slice(0, 65_536), source: "dag/get" });
        } catch (e) {
          setSt({ loading: false, err: (e as Error).message });
        }
        return;
      }
      setSt({ loading: false, err: "demo" });
    })();
  }, [object.cid, mode, client]);

  const links = st.bytes ? scanCidLinks(st.bytes) : [];

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>Инспектор блока</h3>
          <button className="icon-btn" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>
        <div className="modal-body">
          <div className="insp-cid">
            <span className="mono wrap">{object.cid || "—"}</span>
            {object.cid && (
              <button className="icon-btn ghost" title="Копировать CID" onClick={() => onCopy(object.cid)}>
                <IconCopy size={15} />
              </button>
            )}
          </div>

          {st.loading && <div className="insp-note">чтение блока…</div>}

          {st.err === "demo" && (
            <div className="insp-note">
              Инспектор блока читает содержимое из локального кэша (для загруженных объектов) или
              из шлюза в live-режиме. У этого демо-объекта нет байтов.
            </div>
          )}
          {st.err && st.err !== "demo" && <div className="insp-note err">Ошибка: {st.err}</div>}

          {st.bytes && (
            <>
              <div className="insp-meta">
                <span><b>Формат:</b> {detectFormat(st.bytes)}</span>
                <span><b>Прочитано:</b> {humanSize(st.bytes.length)}</span>
                <span><b>Источник:</b> {st.source}</span>
              </div>

              {links.length > 0 && (
                <div className="insp-links">
                  <div className="insp-sub">Связанные CID ({links.length})</div>
                  {links.map((l) => (
                    <button key={l} className="insp-link" onClick={() => onCopy(l)} title="Копировать">
                      <span className="mono">{l.slice(0, 18)}…{l.slice(-6)}</span>
                      <IconCopy size={13} />
                    </button>
                  ))}
                </div>
              )}

              <div className="insp-sub">Hex-дамп</div>
              <pre className="hex-dump">{hexDump(st.bytes)}</pre>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
