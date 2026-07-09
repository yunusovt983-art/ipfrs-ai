// SemanticSearchPanel — live vector search via /api/v0/semantic/search.
//
// In demo mode: falls back to the same char-ngram ranking as smartSearch,
// annotated as "локальный (ngram)".
// In live mode: builds on-device embedding, POSTs to the gateway, maps
// returned CIDs back to S3Object entries.

import { useEffect, useRef, useState } from "react";
import type { ConnMode, S3Object } from "../types";
import type { IpfrsClient, SemanticStats } from "../lib/ipfrs";
import { buildQueryEmbedding } from "../lib/ipfrs";
import { smartSearch } from "../lib/search";
import { fileCategory, humanSize, relTime } from "../lib/format";
import {
  IconArchive,
  IconCode,
  IconData,
  IconDoc,
  IconDownload,
  IconFile,
  IconImage,
  IconModel,
  IconSearch,
} from "./icons";

const GLYPH: Record<string, React.ComponentType<{ size: number }>> = {
  image: IconImage, code: IconCode, data: IconData,
  model: IconModel, doc: IconDoc, archive: IconArchive, file: IconFile,
};

function Glyph({ name, type }: { name: string; type: string }) {
  const cat = fileCategory(name, type);
  const Cmp = GLYPH[cat] ?? IconFile;
  return <span className={"glyph cat-" + cat}><Cmp size={18} /></span>;
}

interface VecResult {
  object: S3Object;
  score: number;       // 0-1
  source: "vector" | "ngram";
}

interface Props {
  query: string;
  objects: S3Object[];
  mode: ConnMode;
  client: IpfrsClient;
  onOpen: (key: string) => void;
  onDownload: (o: S3Object) => void;
}

type Phase = "idle" | "searching" | "done" | "error";

export function SemanticSearchPanel({ query, objects, mode, client, onOpen, onDownload }: Props) {
  const [results, setResults] = useState<VecResult[]>([]);
  const [phase, setPhase] = useState<Phase>("idle");
  const [errorMsg, setErrorMsg] = useState("");
  const [stats, setStats] = useState<SemanticStats | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Fetch semantic stats once on mount in live mode
  useEffect(() => {
    if (mode !== "live") return;
    client.semanticStats().then((s) => setStats(s));
  }, [mode, client]);

  useEffect(() => {
    abortRef.current?.abort();
    if (!query.trim()) { setResults([]); setPhase("idle"); return; }

    const ctrl = new AbortController();
    abortRef.current = ctrl;
    setPhase("searching");
    setErrorMsg("");

    (async () => {
      if (mode === "live") {
        try {
          const embedding = buildQueryEmbedding(query);
          const raw = await client.semanticSearch(embedding, { topK: 20, minScore: 0.05 });
          if (ctrl.signal.aborted) return;

          // Map CIDs back to local objects; include unmatched as low-score
          const byKey = new Map(objects.map((o) => [o.key, o]));
          const byCid = new Map(objects.map((o) => [o.cid, o]));

          const matched: VecResult[] = raw
            .map((r) => {
              const obj = byCid.get(r.cid) ?? (r.key ? byKey.get(r.key) : undefined);
              if (!obj) return null;
              return { object: obj, score: r.score, source: "vector" as const };
            })
            .filter(Boolean) as VecResult[];

          // Fill up to 10 more with local ngram results not already matched
          const matchedKeys = new Set(matched.map((r) => r.object.key));
          const fallback = (await smartSearch(query, objects))
            .filter((r) => !matchedKeys.has(r.object.key))
            .slice(0, Math.max(0, 10 - matched.length))
            .map((r) => ({ object: r.object, score: r.score * 0.5, source: "ngram" as const }));

          setResults([...matched, ...fallback].sort((a, b) => b.score - a.score));
          setPhase("done");
        } catch (e) {
          if (ctrl.signal.aborted) return;
          // Gateway semantic endpoint unavailable — fall back gracefully
          const msg = (e as Error).message;
          setErrorMsg(msg);
          const fallback = (await smartSearch(query, objects))
            .slice(0, 20)
            .map((r) => ({ object: r.object, score: r.score, source: "ngram" as const }));
          setResults(fallback);
          setPhase("error");
        }
      } else {
        // Demo mode — pure local ngram ranking
        const fallback = (await smartSearch(query, objects))
          .slice(0, 20)
          .map((r) => ({ object: r.object, score: r.score, source: "ngram" as const }));
        if (ctrl.signal.aborted) return;
        setResults(fallback);
        setPhase("done");
      }
    })();

    return () => ctrl.abort();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, mode]);

  if (!query.trim()) {
    return (
      <div className="empty-state">
        <div className="empty-ic"><IconSearch size={30} /></div>
        <p>Семантический поиск</p>
        {mode === "live" && stats && (
          <div className="sem-stats-row">
            <span className="sem-stat-pill">{stats.num_vectors.toLocaleString()} векторов</span>
            <span className="sem-stat-pill">{stats.dimension}d</span>
            <span className="sem-stat-pill">{stats.metric}</span>
          </div>
        )}
        <span className="hint">
          {mode === "live"
            ? stats
              ? `HNSW index · dim=${stats.dimension} · ${stats.metric} · /api/v0/semantic/search`
              : "Подключение к /api/v0/semantic/search…"
            : "Локальный char-ngram (768d FNV-1a) — демо-режим"}
        </span>
        {mode === "live" && !stats && (
          <span className="sem-warn">
            Семантический индекс недоступен. Убедитесь что gateway запущен с флагом --semantic.
          </span>
        )}
      </div>
    );
  }

  return (
    <div className="smart-results">
      {/* ── Status bar ── */}
      <div className="sem-statusbar">
        <span className="sem-count">
          {phase === "searching" && <><span className="sem-spinner" /> поиск…</>}
          {phase !== "searching" && <>{results.length} результатов</>}
        </span>
        <span className={"sem-mode-badge" + (phase === "error" ? " fallback" : "")}>
          {mode === "live" && phase !== "error" ? "⬡ vector" : "◈ ngram"}
        </span>
        {phase === "error" && (
          <span className="sem-fallback-note" title={errorMsg}>
            vector недоступен — ngram fallback
          </span>
        )}
      </div>

      {/* ── Result rows ── */}
      {results.map((r) => {
        const pct = Math.round(r.score * 100);
        return (
          <div key={r.object.key} className="smart-row" onClick={() => onOpen(r.object.key)}>
            <Glyph name={r.object.key} type={r.object.contentType} />
            <div className="sr-main">
              <div className="sr-name">
                <span className="sr-key" title={r.object.key}>{r.object.key}</span>
                <span className={"sr-where" + (r.source === "vector" ? " vec" : "")}>
                  {r.source === "vector" ? "vector" : "ngram"}
                </span>
              </div>
              <div className="sr-meta">
                {humanSize(r.object.size)} · {relTime(r.object.lastModified)}
              </div>
            </div>
            <div className="sr-score">
              <div className="sr-bar">
                <i style={{ width: `${pct}%` }} className={r.source === "vector" ? "vec" : ""} />
              </div>
              <span>{pct}</span>
            </div>
            <button
              className="icon-btn ghost"
              title="Скачать"
              onClick={(e) => { e.stopPropagation(); onDownload(r.object); }}
            >
              <IconDownload size={16} />
            </button>
          </div>
        );
      })}

      {phase === "done" && !results.length && (
        <div className="empty-state"><p>Ничего не найдено по «{query}»</p></div>
      )}
    </div>
  );
}
