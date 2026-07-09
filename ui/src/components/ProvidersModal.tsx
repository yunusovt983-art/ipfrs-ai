import { useEffect, useState } from "react";
import type { ConnMode, Provider, S3Object } from "../types";
import type { IpfrsClient } from "../lib/ipfrs";
import { IconClose, IconCopy } from "./icons";

function shortPeer(p: string): string {
  return p.length > 20 ? `${p.slice(0, 10)}…${p.slice(-6)}` : p;
}

function ProviderRow({
  p,
  maxRtt,
  onCopy,
}: {
  p: Provider;
  maxRtt: number;
  onCopy: (s: string) => void;
}) {
  const pct = Math.max(6, Math.round((p.rttMs / maxRtt) * 100));
  const grade = p.rttMs < 40 ? "fast" : p.rttMs < 100 ? "mid" : "slow";
  return (
    <div className="prov-row">
      <div className="prov-peer">
        <span className="mono" title={p.peer}>{shortPeer(p.peer)}</span>
        {p.role === "origin" && <span className="prov-origin">origin</span>}
        <button className="icon-btn ghost" title="Копировать" onClick={() => onCopy(p.peer)}>
          <IconCopy size={12} />
        </button>
      </div>
      <span className="prov-region">{p.region}</span>
      <div className="prov-rtt">
        <div className="prov-rtt-bar">
          <i className={grade} style={{ width: `${pct}%` }} />
        </div>
        <span className="prov-ms">{p.rttMs} ms</span>
      </div>
    </div>
  );
}

interface LiveState {
  loading: boolean;
  provs?: string[];
  peers?: string[];
  err?: string;
}

export function ProvidersModal({
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
  onCopy: (s: string) => void;
}) {
  const demo = object.providers;
  const [live, setLive] = useState<LiveState>({ loading: false });

  useEffect(() => {
    if (demo || mode !== "live" || !object.cid) return;
    setLive({ loading: true });
    Promise.allSettled([client.findProviders(object.cid), client.swarmPeers()]).then(
      ([provs, peers]) =>
        setLive({
          loading: false,
          provs: provs.status === "fulfilled" ? provs.value : [],
          peers: peers.status === "fulfilled" ? peers.value : [],
          err: provs.status === "rejected" ? (provs.reason as Error).message : undefined,
        }),
    );
  }, [demo, mode, client, object.cid]);

  const maxRtt = demo ? Math.max(1, ...demo.map((p) => p.rttMs)) : 1;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>Провайдеры и пиры</h3>
          <button className="icon-btn" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>
        <div className="modal-body">
          {demo ? (
            <>
              <div className="prov-headline">
                реплицирован на <b>{demo.length}</b> узлах · выбор по RTT + региону (гео-роутинг)
              </div>
              <div className="prov-list">
                {[...demo]
                  .sort((a, b) => a.rttMs - b.rttMs)
                  .map((p) => (
                    <ProviderRow key={p.peer} p={p} maxRtt={maxRtt} onCopy={onCopy} />
                  ))}
              </div>
              <div className="prov-note">
                Демо-данные. В live-режиме провайдеры CID берутся из
                <code> /api/v0/dht/findprovs</code>, подключённые пиры — из
                <code> /api/v0/swarm/peers</code>.
              </div>
            </>
          ) : live.loading ? (
            <div className="insp-note">запрос DHT и списка пиров…</div>
          ) : mode === "live" ? (
            <>
              <div className="prov-headline">
                провайдеров CID: <b>{live.provs?.length ?? 0}</b> · подключённых пиров:{" "}
                <b>{live.peers?.length ?? 0}</b>
              </div>
              <div className="prov-list">
                {(live.provs ?? []).map((id) => (
                  <div className="prov-row" key={id}>
                    <div className="prov-peer">
                      <span className="mono">{shortPeer(id)}</span>
                      <span className="prov-origin">провайдер</span>
                    </div>
                  </div>
                ))}
                {(live.peers ?? []).map((id) => (
                  <div className="prov-row" key={"p" + id}>
                    <div className="prov-peer">
                      <span className="mono">{shortPeer(id)}</span>
                    </div>
                    <span className="prov-region">подключён</span>
                  </div>
                ))}
              </div>
              {!live.provs?.length && (
                <div className="prov-note">
                  Провайдеры CID приходят асинхронно через события DHT — список может быть пустым сразу
                  после запроса.
                </div>
              )}
            </>
          ) : (
            <div className="insp-note">
              Карта пиров доступна в live-режиме. У демо-объекта нет seeded-провайдеров.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
