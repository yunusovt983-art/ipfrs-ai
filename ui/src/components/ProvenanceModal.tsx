import { useEffect, useState } from "react";
import type { ConnMode, ProofStep, S3Object } from "../types";
import type { IpfrsClient } from "../lib/ipfrs";
import { IconCheck, IconClose } from "./icons";

function StepView({ step, depth }: { step: ProofStep; depth: number }) {
  return (
    <div className="proof-step" style={{ marginLeft: depth ? 18 : 0 }}>
      <div className="ps-goal">
        <span className="ps-bullet" />
        <span className="mono">{step.goal}</span>
      </div>
      {step.rule && <div className="ps-rule">← {step.rule}</div>}
      {step.bindings && Object.keys(step.bindings).length > 0 && (
        <div className="ps-binds">
          {Object.entries(step.bindings).map(([k, v]) => (
            <span className="ps-bind mono" key={k}>
              {k} = {v}
            </span>
          ))}
        </div>
      )}
      {step.sub?.map((s, i) => (
        <StepView key={i} step={s} depth={depth + 1} />
      ))}
    </div>
  );
}

interface LiveState {
  loading: boolean;
  data?: unknown;
  none?: boolean;
  err?: string;
}

export function ProvenanceModal({
  object,
  mode,
  client,
  onClose,
}: {
  object: S3Object;
  mode: ConnMode;
  client: IpfrsClient;
  onClose: () => void;
}) {
  const demo = object.proof;
  const [live, setLive] = useState<LiveState>({ loading: false });

  useEffect(() => {
    if (demo || mode !== "live" || !object.cid) return;
    setLive({ loading: true });
    client
      .getProof(object.cid)
      .then((p) => setLive(p == null ? { loading: false, none: true } : { loading: false, data: p }))
      .catch((e) => setLive({ loading: false, err: (e as Error).message }));
  }, [demo, mode, client, object.cid]);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>Провенанс · proof-carrying</h3>
          <button className="icon-btn" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>
        <div className="modal-body">
          {demo ? (
            <>
              <div className="prov-bar">
                <span className={"prov-badge " + (demo.verified ? "ok" : "no")}>
                  <IconCheck size={14} /> {demo.verified ? "доказательство проверено" : "не проверено"}
                </span>
                <span className="prov-engine">{demo.engine}</span>
              </div>
              <div className="prov-tree">
                <StepView step={demo.root} depth={0} />
              </div>
              <div className="prov-note">
                Демо-доказательство происхождения. В live-режиме дерево читается из
                <code> /api/v0/logic/proof/&lt;cid&gt;</code> и проверяется пиром.
              </div>
            </>
          ) : live.loading ? (
            <div className="insp-note">запрос доказательства…</div>
          ) : live.none ? (
            <div className="insp-note">Для этого CID доказательство не найдено.</div>
          ) : live.err ? (
            <div className="insp-note err">Ошибка: {live.err}</div>
          ) : live.data ? (
            <pre className="code-preview">{JSON.stringify(live.data, null, 2)}</pre>
          ) : (
            <div className="insp-note">
              Провенанс доступен в live-режиме (TensorLogic proof-carrying). У демо-объекта нет
              связанного доказательства.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
