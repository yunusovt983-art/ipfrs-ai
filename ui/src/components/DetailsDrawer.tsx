import { useEffect, useState, useMemo, type ReactNode } from "react";
import type { ConnMode, S3Object } from "../types";
import type { IpfrsClient } from "../lib/ipfrs";
import { fileCategory, humanSize, relTime } from "../lib/format";
import { PreviewPane } from "./PreviewPane";
import {
  IconCheck,
  IconCode,
  IconCopy,
  IconClose,
  IconData,
  IconDownload,
  IconLink,
  IconModel,
  IconPin,
  IconTrash,
} from "./icons";

type Tab = "info" | "versions" | "actions";

interface Props {
  object: S3Object;
  mode: ConnMode;
  client: IpfrsClient;
  onClose: () => void;
  onDownload: (o: S3Object) => void;
  onDownloadVersion: (cid: string, filename: string) => void;
  onCopy: (cid: string) => void;
  onShare: (o: S3Object) => void;
  onPin: (o: S3Object) => void;
  onRestore: (key: string, versionCid: string) => void;
  onInspect: (o: S3Object) => void;
  onDag: (o: S3Object) => void;
  onProof: (o: S3Object) => void;
  onProviders: (o: S3Object) => void;
  onDelete: (key: string) => void;
}

export function DetailsDrawer({
  object,
  mode,
  client,
  onClose,
  onDownload,
  onDownloadVersion,
  onCopy,
  onShare,
  onPin,
  onRestore,
  onInspect,
  onDag,
  onProof,
  onProviders,
  onDelete,
}: Props) {
  const name = object.key.split("/").pop() || object.key;
  const cat = fileCategory(name, object.contentType);
  const versionCount = object.versions?.length ?? 0;

  const [tab, setTab] = useState<Tab>("info");

  // Reset to "info" whenever the selected object changes
  useEffect(() => {
    setTab("info");
  }, [object.key]);

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
      ["Класс хранения", "IPFS (content-addressed)"],
      ["Пин", object.pinned ? "закреплён ✓" : "не закреплён"],
    ],
    [object, onCopy],
  );

  return (
    <aside className="drawer">
      {/* ── Header ── */}
      <div className="drawer-head">
        <div className={"drawer-glyph glyph cat-" + cat} />
        <div className="drawer-title" title={name}>{name}</div>
        <button className="icon-btn" onClick={onClose} title="Закрыть">
          <IconClose size={18} />
        </button>
      </div>

      {/* ── Tab bar ── */}
      <div className="drawer-tabs">
        <button
          className={"drawer-tab" + (tab === "info" ? " active" : "")}
          onClick={() => setTab("info")}
        >
          Инфо
        </button>
        <button
          className={"drawer-tab" + (tab === "versions" ? " active" : "")}
          onClick={() => setTab("versions")}
        >
          Версии
          <span className="tab-badge">{versionCount + 1}</span>
        </button>
        <button
          className={"drawer-tab" + (tab === "actions" ? " active" : "")}
          onClick={() => setTab("actions")}
        >
          Действия
        </button>
      </div>

      {/* ── Pane: Инфо ── */}
      {tab === "info" && (
        <div className="drawer-pane">
          <PreviewPane object={object} mode={mode} client={client} />

          <div className="drawer-meta">
            {rows.map(([k, v]) => (
              <div className="meta-row" key={k}>
                <div className="meta-k">{k}</div>
                <div className="meta-v">{v}</div>
              </div>
            ))}
          </div>

          <div className="ipfrs-tools">
            <div className="tools-head">IPFRS</div>
            <div className="tools-row">
              <button className="tool-btn" onClick={() => onDag(object)} title="DAG-эксплорер">
                <IconData size={16} /> DAG
              </button>
              <button
                className={"tool-btn" + (object.proof ? " has" : "")}
                onClick={() => onProof(object)}
                title="Провенанс / proof"
              >
                <IconCheck size={16} /> Провенанс
              </button>
              <button
                className={"tool-btn" + (object.providers ? " has" : "")}
                onClick={() => onProviders(object)}
                title="Провайдеры и пиры"
              >
                <IconModel size={16} /> Пиры
              </button>
              <button className="tool-btn" onClick={() => onInspect(object)} title="Инспектор блока">
                <IconCode size={16} /> Блок
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ── Pane: Версии ── */}
      {tab === "versions" && (
        <div className="drawer-pane">
          <div className="versions" style={{ padding: "16px 18px" }}>
            <div className="versions-head">История версий ({versionCount + 1})</div>

            {/* Current version */}
            <div className="version-row current">
              <div className="v-tag">v{versionCount + 1}</div>
              <div className="v-meta">
                <span className="mono v-cid">{object.cid.slice(0, 14)}…</span>
                <span className="v-sub">{humanSize(object.size)} · текущая</span>
              </div>
              <div className="v-actions">
                <button
                  className="icon-btn ghost"
                  title="Скачать"
                  onClick={() => onDownload(object)}
                >
                  <IconDownload size={15} />
                </button>
              </div>
            </div>

            {/* Past versions */}
            {versionCount === 0 ? (
              <div style={{ padding: "20px 0", color: "var(--faint)", fontSize: "0.82rem" }}>
                Только одна версия. При повторной загрузке с тем же ключом — появятся предыдущие.
              </div>
            ) : (
              object.versions!.map((v, i) => (
                <div className="version-row" key={v.cid + i}>
                  <div className="v-tag">v{versionCount - i}</div>
                  <div className="v-meta">
                    <span className="mono v-cid" title={v.cid}>{v.cid.slice(0, 14)}…</span>
                    <span className="v-sub">{humanSize(v.size)} · {relTime(v.lastModified)}</span>
                  </div>
                  <div className="v-actions">
                    <button
                      className="icon-btn ghost"
                      title="Скачать версию"
                      onClick={() => onDownloadVersion(v.cid, name)}
                    >
                      <IconDownload size={15} />
                    </button>
                    <button className="mini-btn" onClick={() => onRestore(object.key, v.cid)}>
                      Восстановить
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {/* ── Pane: Действия ── */}
      {tab === "actions" && (
        <div className="drawer-pane">
          <div className="drawer-actions-pane">
            <div className="drawer-action-label">Файл</div>
            <div className="drawer-action-group">
              <button className="btn primary" onClick={() => onDownload(object)}>
                <IconDownload size={16} /> Скачать
              </button>
              <button className="btn ghost" onClick={() => onShare(object)} disabled={!object.cid}>
                <IconLink size={16} /> Поделиться
              </button>
            </div>

            <div className="drawer-action-label" style={{ marginTop: 8 }}>IPFS</div>
            <div className="drawer-action-group">
              <button
                className={"btn ghost" + (object.pinned ? " pinned" : "")}
                onClick={() => onPin(object)}
                title={object.pinned ? "Открепить" : "Закрепить"}
              >
                <IconPin size={16} /> {object.pinned ? "Открепить" : "Закрепить"}
              </button>
              <button className="btn ghost" onClick={() => onCopy(object.cid)} disabled={!object.cid}>
                <IconCopy size={16} /> Копировать CID
              </button>
              {mode === "live" && object.cid && (
                <a className="btn ghost" href={client.ipfsUrl(object.cid)} target="_blank" rel="noreferrer">
                  <IconLink size={16} /> Открыть в шлюзе
                </a>
              )}
            </div>

            <div className="drawer-action-label" style={{ marginTop: 8 }}>Опасная зона</div>
            <div className="drawer-action-group">
              <button
                className="btn danger"
                onClick={() => {
                  if (confirm(`Удалить объект «${name}»?`)) onDelete(object.key);
                }}
              >
                <IconTrash size={16} /> Удалить объект
              </button>
            </div>
          </div>
        </div>
      )}
    </aside>
  );
}
