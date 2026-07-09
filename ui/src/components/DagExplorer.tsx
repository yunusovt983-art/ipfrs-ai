import { useState } from "react";
import type { ConnMode, DagNode, S3Object } from "../types";
import type { IpfrsClient } from "../lib/ipfrs";
import { humanSize } from "../lib/format";
import { scanCidLinks } from "../lib/inspect";
import { IconChevron, IconClose, IconCopy, IconData } from "./icons";

interface TNode {
  cid: string;
  name?: string;
  size?: number;
  codec?: string;
  links?: TNode[]; // undefined = not loaded (live)
}

function fromDag(n: DagNode): TNode {
  return { cid: n.cid, name: n.name, size: n.size, codec: n.codec, links: n.links.map(fromDag) };
}

function TreeRow({
  node,
  depth,
  mode,
  client,
  onCopy,
}: {
  node: TNode;
  depth: number;
  mode: ConnMode;
  client: IpfrsClient;
  onCopy: (cid: string) => void;
}) {
  const [expanded, setExpanded] = useState(depth === 0);
  const [links, setLinks] = useState<TNode[] | undefined>(node.links);
  const [size, setSize] = useState<number | undefined>(node.size);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string>();

  const loadable = links === undefined && mode === "live";
  const hasChildren = links === undefined ? mode === "live" : links.length > 0;

  const toggle = async () => {
    if (!expanded && loadable) {
      setLoading(true);
      try {
        const bytes = await client.dagGet(node.cid);
        setSize(bytes.length);
        setLinks(scanCidLinks(bytes).map((c) => ({ cid: c })));
      } catch (e) {
        setErr((e as Error).message);
        setLinks([]);
      }
      setLoading(false);
    }
    setExpanded((v) => !v);
  };

  return (
    <div className="dag-node" style={{ marginLeft: depth ? 16 : 0 }}>
      <div className="dag-row">
        <button
          className={"dag-caret" + (hasChildren ? "" : " leaf")}
          onClick={toggle}
          disabled={!hasChildren}
        >
          {hasChildren ? (
            <IconChevron size={13} style={{ transform: expanded ? "rotate(90deg)" : "none" }} />
          ) : (
            <span className="dag-dot" />
          )}
        </button>
        <IconData size={15} className="dag-ic" />
        <span className="dag-label mono" title={node.cid}>
          {node.name ?? `${node.cid.slice(0, 12)}…${node.cid.slice(-6)}`}
        </span>
        {node.codec && <span className="dag-codec">{node.codec}</span>}
        {size != null && <span className="dag-size">{humanSize(size)}</span>}
        <button className="icon-btn ghost" title="Копировать CID" onClick={() => onCopy(node.cid)}>
          <IconCopy size={13} />
        </button>
      </div>
      {loading && <div className="dag-hint" style={{ marginLeft: 32 }}>чтение блока…</div>}
      {err && <div className="dag-hint err" style={{ marginLeft: 32 }}>{err}</div>}
      {expanded &&
        links?.map((c, i) => (
          <TreeRow key={c.cid + i} node={c} depth={depth + 1} mode={mode} client={client} onCopy={onCopy} />
        ))}
      {expanded && links && links.length === 0 && !loadable && (
        <div className="dag-hint" style={{ marginLeft: 32 }}>нет ссылок (лист DAG)</div>
      )}
    </div>
  );
}

export function DagExplorer({
  object,
  mode,
  client,
  onClose,
  onCopy,
}: {
  object: S3Object;
  mode: ConnMode;
  client: IpfrsClient;
  onClose: () => void;
  onCopy: (cid: string) => void;
}) {
  const root: TNode = object.dag
    ? fromDag(object.dag)
    : { cid: object.cid, name: object.key.split("/").pop(), size: object.size, links: mode === "live" ? undefined : [] };

  const count = (n: DagNode): number => 1 + n.links.reduce((s, l) => s + count(l), 0);
  const total = object.dag ? count(object.dag) : undefined;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>DAG-эксплорер {total ? <span className="muted">· {total} блоков</span> : null}</h3>
          <button className="icon-btn" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>
        <div className="modal-body">
          {object.dag ? (
            <div className="dag-src">демо-структура · блоки контент-адресованы по CID</div>
          ) : mode === "live" ? (
            <div className="dag-src">live · ссылки читаются из блоков (DAG-CBOR tag 42) по мере раскрытия</div>
          ) : (
            <div className="dag-src">
              демо-режим: у объекта нет разложенного DAG. Раскрытие под-CID доступно в live против шлюза.
            </div>
          )}
          <div className="dag-tree">
            <TreeRow node={root} depth={0} mode={mode} client={client} onCopy={onCopy} />
          </div>
        </div>
      </div>
    </div>
  );
}
