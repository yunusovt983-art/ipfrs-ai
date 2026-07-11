// KnowledgeModal — "поиск по знаниям" over a typed, content-addressed knowledge
// graph. Mirrors the ipfrs-knowledge crate: the same char-trigram embedding,
// cosine top-k, and the deterministic wikilink Markdown projection.
//
// Two sources, one UI:
//  · demo  — an on-device faithful graph (works on the static Pages deploy).
//  · live  — queries the gateway's /api/v0/knowledge/* (seeded from the same demo
//            graph on first open); results are backed by a real committed head CID.

import { useEffect, useMemo, useRef, useState } from "react";
import type { ConnMode } from "../types";
import type { IpfrsClient, KnowledgeHit } from "../lib/ipfrs";
import {
  DEMO_GRAPH,
  entityId,
  indexGraph,
  renderMarkdown,
  searchIndex,
  slug,
  type KEntity,
  type KHit,
} from "../lib/knowledge";
import { IconClose, IconSearch } from "./icons";

const KIND_GLYPH: Record<string, string> = { person: "🧑", machine: "⚙️", concept: "💡" };

type Source = "demo" | "live";

export function KnowledgeModal({
  mode,
  client,
  onClose,
}: {
  mode: ConnMode;
  client: IpfrsClient;
  onClose: () => void;
}) {
  const g = DEMO_GRAPH;
  const index = useMemo(() => indexGraph(g), [g]);
  const [query, setQuery] = useState("lovelace");
  const [selected, setSelected] = useState<KEntity | null>(g.entities[0]);
  const [ids, setIds] = useState<Record<string, string>>({});

  const [source, setSource] = useState<Source>("demo");
  const [liveHits, setLiveHits] = useState<KnowledgeHit[] | null>(null);
  const [liveProjection, setLiveProjection] = useState<Record<string, string> | null>(null);
  const [status, setStatus] = useState("");
  const [refreshKey, setRefreshKey] = useState(0);
  const [heads, setHeads] = useState<{ live: string | null; recent: string[]; retain: number } | null>(null);
  const [gcReport, setGcReport] = useState<{ kept: number; deleted: number } | null>(null);
  const debounce = useRef<number | undefined>(undefined);
  const fileRef = useRef<HTMLInputElement | null>(null);

  // Real EntityIds (sha256(kind\0name)) for the demo projection, once.
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

  // In live mode: probe the gateway, seed the demo graph if empty, capture the head.
  useEffect(() => {
    if (mode !== "live") return;
    let alive = true;
    (async () => {
      setStatus("Проверка шлюза…");
      const stats = await client.knowledgeStats();
      if (!alive) return;
      if (!stats) {
        setStatus("Шлюз без знаний — демо");
        setSource("demo");
        return;
      }
      if (stats.entities === 0) {
        setStatus("Посев демо-графа…");
        for (const e of g.entities) {
          await client.knowledgeAddEntity(e.kind, e.name, e.aliases, e.attrs);
        }
        for (const r of g.relations) {
          const obj = g.entities.find((e) => e.name === r.object);
          const subj = g.entities.find((e) => e.name === r.subject);
          if (subj && obj) {
            await client.knowledgeAddRelation(subj.kind, subj.name, r.predicate, obj.kind, obj.name, r.weight);
          }
        }
      }
      const h = await client.knowledgeCommit();
      if (!alive) return;
      setSource("live");
      setStatus(h ? `live · head ${h.slice(0, 12)}…` : "live");
      setHeads(await client.knowledgeHeads());
    })();
    return () => {
      alive = false;
    };
  }, [mode, client, g]);

  // Live search (debounced) once we're in live mode.
  useEffect(() => {
    if (source !== "live") return;
    window.clearTimeout(debounce.current);
    debounce.current = window.setTimeout(async () => {
      const hits = await client.knowledgeSearch(query.trim() || " ", 8);
      setLiveHits(hits ?? []);
    }, 180);
    return () => window.clearTimeout(debounce.current);
  }, [source, query, client, refreshKey]);

  const demoHits: KHit[] = useMemo(
    () => (query.trim() ? searchIndex(index, query, 8) : []),
    [index, query],
  );

  async function pickEntity(name: string, kind: string) {
    const demo = g.entities.find((e) => e.name === name);
    if (demo) setSelected(demo);
    if (source === "live" && !liveProjection) {
      setLiveProjection(await client.knowledgeProjection());
    }
    void kind;
  }

  async function exportCar() {
    setStatus("Экспорт CAR…");
    const blob = await client.knowledgeExport();
    if (!blob) {
      setStatus("Экспорт не удался");
      return;
    }
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "knowledge.car.zst";
    a.click();
    URL.revokeObjectURL(url);
    setStatus(`live · экспортировано ${(blob.size / 1024).toFixed(1)} KB`);
  }

  async function exportDiff() {
    const recent = heads?.recent ?? [];
    if (recent.length < 2) return;
    const [to, from] = [recent[0], recent[1]];
    setStatus("Diff CAR…");
    const blob = await client.knowledgeDiff(to, from);
    if (!blob) {
      setStatus("Diff не удался");
      return;
    }
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "knowledge-diff.car.zst";
    a.click();
    URL.revokeObjectURL(url);
    setStatus(`live · diff ${(blob.size / 1024).toFixed(1)} KB (${from.slice(0, 8)}…→${to.slice(0, 8)}…)`);
  }

  async function importCar(file: File) {
    setStatus("Импорт CAR…");
    const head = await client.knowledgeImport(await file.arrayBuffer());
    if (!head) {
      setStatus("Импорт не удался");
      return;
    }
    setLiveProjection(null); // re-fetch on next selection
    setRefreshKey((k) => k + 1); // re-run live search against the new head
    setStatus(`live · head ${head.slice(0, 12)}…`);
    setHeads(await client.knowledgeHeads());
  }

  async function runGc() {
    setStatus("GC…");
    const r = await client.knowledgeGc(false);
    if (!r) {
      setStatus("GC не удался");
      return;
    }
    setGcReport({ kept: r.kept, deleted: r.deleted });
    setStatus(`live · GC: kept ${r.kept}, deleted ${r.deleted}`);
    setHeads(await client.knowledgeHeads());
  }

  // The projection text for the selected entity.
  const md = useMemo(() => {
    if (!selected) return "";
    if (source === "live" && liveProjection) {
      return liveProjection[`${slug(selected.name)}.md`] ?? renderMarkdown(g, selected, ids[selected.name] ?? "…");
    }
    return renderMarkdown(g, selected, ids[selected.name] ?? "…");
  }, [selected, source, liveProjection, g, ids]);

  const rows =
    source === "live"
      ? (liveHits ?? []).map((h) => ({
          title: h.title || "(без имени)",
          subtitle: h.kind,
          score: h.score,
          kind: h.kind,
          entityName: h.kind === "entity" ? h.title : null,
        }))
      : demoHits.map((h) => ({
          title: h.title,
          subtitle: h.subtitle,
          score: h.score,
          kind: h.kind,
          entityName: h.entity ? h.entity.name : null,
        }));

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
          <span className={"know-note" + (source === "live" ? " live" : "")}>
            {source === "live" ? status || "live" : "demo · on-device"}
          </span>
        </div>

        <div className="know-body">
          <div className="know-results">
            {rows.length === 0 && <div className="know-empty">Введите запрос</div>}
            {rows.map((h, i) => (
              <button
                key={i}
                className={"know-hit" + (h.entityName && selected?.name === h.entityName ? " active" : "")}
                onClick={() => h.entityName && pickEntity(h.entityName, h.kind)}
              >
                <span className="know-hit-glyph">
                  {h.kind === "evidence" ? "📄" : KIND_GLYPH[h.subtitle] ?? KIND_GLYPH[h.kind] ?? "•"}
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

        {source === "live" && heads && (
          <div className="know-snapshots">
            <span className="know-snap-label">
              Снимки {heads.recent.length}/{heads.retain}
            </span>
            <div className="know-snap-chips">
              {heads.recent.slice(0, 6).map((h) => (
                <code
                  key={h}
                  className={"know-snap-chip" + (h === heads.live ? " live" : "")}
                  title={h + (h === heads.live ? " (текущий head)" : "")}
                >
                  {h.slice(0, 8)}…
                </code>
              ))}
            </div>
            <button className="btn ghost" onClick={runGc} title="Сборка мусора: оставить пины + последние N heads">
              GC
            </button>
            {gcReport && (
              <span className="know-snap-gc">
                kept {gcReport.kept} · deleted {gcReport.deleted}
              </span>
            )}
          </div>
        )}

        <div className="know-foot">
          <div className="know-foot-text">
            Тот же embedding и проекция, что в крейте <code>ipfrs-knowledge</code>.{" "}
            {source === "live"
              ? "Запрос идёт на шлюз /api/v0/knowledge/*; результаты подкреплены реальным head-CID."
              : "В live-режиме запрос уходит на шлюз; здесь — детерминированно on-device."}
          </div>
          {source === "live" && (
            <div className="know-foot-actions">
              <button className="btn ghost" onClick={exportCar} title="Скачать весь граф одним CAR-файлом">
                ⬇ Export CAR
              </button>
              {(heads?.recent.length ?? 0) >= 2 && (
                <button
                  className="btn ghost"
                  onClick={exportDiff}
                  title="Скачать инкрементальный CAR между двумя последними снимками"
                >
                  ⬇ Diff CAR
                </button>
              )}
              <button
                className="btn ghost"
                onClick={() => fileRef.current?.click()}
                title="Загрузить граф из CAR-файла"
              >
                ⬆ Import CAR
              </button>
              <input
                ref={fileRef}
                type="file"
                accept=".car,application/vnd.ipld.car"
                hidden
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) void importCar(f);
                  e.target.value = "";
                }}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
