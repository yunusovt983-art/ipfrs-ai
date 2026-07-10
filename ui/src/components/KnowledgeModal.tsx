// KnowledgeModal — "поиск по знаниям" over a typed, content-addressed knowledge
// graph. Mirrors the ipfrs-knowledge crate: the same char-trigram embedding,
// cosine top-k, and the deterministic wikilink Markdown projection.

import { useEffect, useMemo, useState } from "react";
import {
  DEMO_GRAPH,
  entityId,
  indexGraph,
  renderMarkdown,
  searchIndex,
  type KEntity,
  type KHit,
} from "../lib/knowledge";
import { IconClose, IconSearch } from "./icons";

const KIND_GLYPH: Record<string, string> = { person: "🧑", machine: "⚙️", concept: "💡" };

export function KnowledgeModal({ onClose }: { onClose: () => void }) {
  const g = DEMO_GRAPH;
  const index = useMemo(() => indexGraph(g), [g]);
  const [query, setQuery] = useState("lovelace");
  const [selected, setSelected] = useState<KEntity | null>(g.entities[0]);
  const [ids, setIds] = useState<Record<string, string>>({});

  // Compute real EntityIds (sha256(kind\0name)) once, like EntityId::of.
  useEffect(() => {
    let live = true;
    (async () => {
      const entries = await Promise.all(
        g.entities.map(async (e) => [e.name, await entityId(e.kind, e.name)] as const),
      );
      if (live) setIds(Object.fromEntries(entries));
    })();
    return () => {
      live = false;
    };
  }, [g]);

  const hits: KHit[] = useMemo(
    () => (query.trim() ? searchIndex(index, query, 8) : []),
    [index, query],
  );

  const md = selected ? renderMarkdown(g, selected, ids[selected.name] ?? "…") : "";

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal know-modal" onClick={(e) => e.stopPropagation()}>
        <div className="know-head">
          <div className="know-title">
            <span className="know-badge">🧠</span>
            <div>
              <div className="know-h1">Поиск по знаниям</div>
              <div className="know-sub">
                типизированный граф · char-ngram embedding · косинус top-k ·
                проекция в Markdown
              </div>
            </div>
          </div>
          <button className="icon-btn" title="Закрыть" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>

        <div className="know-searchbar">
          <IconSearch size={16} />
          <input
            autoFocus
            value={query}
            placeholder="lovelace · algorithm · babbage · memoir …"
            onChange={(e) => setQuery(e.target.value)}
          />
          <span className="know-note">demo · on-device</span>
        </div>

        <div className="know-body">
          <div className="know-results">
            {hits.length === 0 && <div className="know-empty">Введите запрос</div>}
            {hits.map((h, i) => (
              <button
                key={i}
                className={
                  "know-hit" +
                  (h.entity && selected?.name === h.entity.name ? " active" : "")
                }
                onClick={() => h.entity && setSelected(h.entity)}
              >
                <span className="know-hit-glyph">
                  {h.kind === "evidence" ? "📄" : KIND_GLYPH[h.subtitle] ?? "•"}
                </span>
                <span className="know-hit-main">
                  <span className="know-hit-title">{h.title}</span>
                  <span className="know-hit-sub">{h.subtitle}</span>
                </span>
                <span className="know-score" title={`cosine ${h.score.toFixed(3)}`}>
                  <span className="know-score-bar" style={{ width: `${Math.max(0, h.score) * 100}%` }} />
                  <span className="know-score-num">{h.score.toFixed(2)}</span>
                </span>
              </button>
            ))}
          </div>

          <div className="know-detail">
            {selected ? (
              <>
                <div className="know-detail-head">
                  <span>{KIND_GLYPH[selected.kind]}</span> {selected.name}
                  <code className="know-id">{(ids[selected.name] ?? "").slice(0, 16)}…</code>
                </div>
                <pre className="know-md">{md}</pre>
              </>
            ) : (
              <div className="know-empty">Выберите узел</div>
            )}
          </div>
        </div>

        <div className="know-foot">
          Тот же embedding и проекция, что в крейте <code>ipfrs-knowledge</code>. В
          live-режиме запрос уходит на шлюз; здесь — детерминированно on-device.
        </div>
      </div>
    </div>
  );
}
