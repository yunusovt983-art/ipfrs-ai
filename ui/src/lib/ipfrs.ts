// Thin client for the IPFRS gateway (Kubo-compatible IPFS HTTP API, /api/v0/*).
//
// The gateway exposes /api/v0/add, /cat, /version, /id, /swarm/peers, /ipfs/:cid,
// etc. (see ipfrs-interface/src/gateway). This module wraps the handful the S3
// console needs and provides a demo fallback that computes deterministic
// pseudo-CIDs client-side so the UI is fully usable without a running gateway.

import type { GatewayInfo } from "../types";

export interface AddResult {
  cid: string;
  size: number;
  name: string;
}

export interface SemanticResult {
  cid: string;
  score: number;
  key?: string;
}

export interface SemanticStats {
  num_vectors: number;
  dimension: number;
  metric: string;
  cache_size: number;
  cache_capacity: number;
}

async function withTimeout<T>(p: Promise<T>, ms: number): Promise<T> {
  const ctl = new AbortController();
  const t = setTimeout(() => ctl.abort(), ms);
  try {
    return await p;
  } finally {
    clearTimeout(t);
  }
}

function normBase(base: string): string {
  return base.replace(/\/+$/, "");
}

export interface KnowledgeHit {
  cid: string;
  score: number;
  kind: string;
  title: string;
}

export class IpfrsClient {
  constructor(private base: string) {}

  private url(path: string): string {
    return `${normBase(this.base)}${path}`;
  }

  /** Probe the gateway: version + peer id + peer count. Throws on failure. */
  async info(signal?: AbortSignal): Promise<GatewayInfo> {
    const ver = await fetch(this.url("/api/v0/version"), { method: "GET", signal });
    if (!ver.ok) throw new Error(`version: HTTP ${ver.status}`);
    const vjson = await ver.json().catch(() => ({}) as Record<string, unknown>);
    const info: GatewayInfo = {
      version: (vjson.Version as string) ?? (vjson.version as string) ?? "unknown",
    };
    try {
      const idr = await fetch(this.url("/api/v0/id"), { method: "GET", signal });
      if (idr.ok) {
        const j = await idr.json();
        info.peerId = (j.ID as string) ?? (j.id as string);
      }
    } catch {
      /* optional */
    }
    try {
      const pr = await fetch(this.url("/api/v0/swarm/peers"), { method: "GET", signal });
      if (pr.ok) {
        const j = await pr.json();
        const peers = (j.Peers as unknown[]) ?? (j.peers as unknown[]) ?? [];
        info.peers = Array.isArray(peers) ? peers.length : undefined;
      }
    } catch {
      /* optional */
    }
    return info;
  }

  /** Upload bytes via /api/v0/add (multipart). Returns the resulting CID. */
  async add(file: File): Promise<AddResult> {
    const form = new FormData();
    form.append("file", file, file.name);
    const res = await withTimeout(
      fetch(this.url("/api/v0/add"), { method: "POST", body: form }),
      120_000,
    );
    if (!res.ok) throw new Error(`add: HTTP ${res.status}`);
    // Kubo streams newline-delimited JSON; take the last complete object.
    const text = await res.text();
    const lines = text.trim().split("\n").filter(Boolean);
    const last = JSON.parse(lines[lines.length - 1]);
    return {
      cid: (last.Hash as string) ?? (last.Cid as string) ?? (last.cid as string),
      size: Number(last.Size ?? file.size),
      name: (last.Name as string) ?? file.name,
    };
  }

  /**
   * Upload with real XHR progress events.
   * `onProgress` fires with 0-100 as bytes are sent.
   */
  addWithProgress(
    file: File,
    onProgress: (pct: number) => void,
    signal?: AbortSignal,
  ): Promise<AddResult> {
    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      const form = new FormData();
      form.append("file", file, file.name);

      xhr.upload.addEventListener("progress", (e) => {
        if (e.lengthComputable) {
          onProgress(Math.min(99, Math.round((e.loaded / e.total) * 100)));
        }
      });

      xhr.addEventListener("load", () => {
        if (xhr.status < 200 || xhr.status >= 300) {
          reject(new Error(`add: HTTP ${xhr.status}`));
          return;
        }
        try {
          const lines = xhr.responseText.trim().split("\n").filter(Boolean);
          const last = JSON.parse(lines[lines.length - 1]);
          onProgress(100);
          resolve({
            cid: (last.Hash as string) ?? (last.Cid as string) ?? (last.cid as string),
            size: Number(last.Size ?? file.size),
            name: (last.Name as string) ?? file.name,
          });
        } catch (e) {
          reject(new Error("add: JSON parse error"));
        }
      });

      xhr.addEventListener("error", () => reject(new Error("add: network error")));
      xhr.addEventListener("abort", () => reject(new Error("add: aborted")));

      signal?.addEventListener("abort", () => xhr.abort());

      xhr.open("POST", this.url("/api/v0/add"));
      xhr.send(form);
    });
  }

  /** URL to fetch/download an object's content. */
  catUrl(cid: string): string {
    return this.url(`/api/v0/cat?arg=${encodeURIComponent(cid)}`);
  }

  /** Public gateway path form, handy for previews / sharing. */
  ipfsUrl(cid: string): string {
    return this.url(`/ipfs/${cid}`);
  }

  /** Fetch a raw block by CID via /api/v0/dag/get (returns decoded bytes). */
  async dagGet(cid: string): Promise<Uint8Array> {
    const res = await fetch(this.url("/api/v0/dag/get"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ arg: cid }),
    });
    if (!res.ok) throw new Error(`dag/get: HTTP ${res.status}`);
    const j = await res.json();
    const b64 = (j.Data as string) ?? "";
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  /** Fetch a proof-carrying provenance tree for a CID (null if none). */
  async getProof(cid: string): Promise<unknown | null> {
    const res = await fetch(this.url(`/api/v0/logic/proof/${encodeURIComponent(cid)}`));
    if (res.status === 404) return null;
    if (!res.ok) throw new Error(`proof: HTTP ${res.status}`);
    const j = await res.json();
    return j.proof ?? null;
  }

  /** DHT providers of a CID (peer id strings). */
  async findProviders(cid: string): Promise<string[]> {
    const res = await fetch(this.url("/api/v0/dht/findprovs"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ arg: cid }),
    });
    if (!res.ok) throw new Error(`findprovs: HTTP ${res.status}`);
    const j = await res.json();
    return ((j.Responses as { ID: string }[]) ?? []).map((r) => r.ID);
  }

  /** Currently connected swarm peers (peer id strings). */
  async swarmPeers(): Promise<string[]> {
    const res = await fetch(this.url("/api/v0/swarm/peers"));
    if (!res.ok) throw new Error(`peers: HTTP ${res.status}`);
    const j = await res.json();
    return ((j.Peers as { Peer: string }[]) ?? []).map((p) => p.Peer);
  }

  /**
   * Vector-based semantic search via the IPFRS semantic context.
   *
   * POST /api/v0/semantic/search
   * Request:  { query: number[], k?: number, filter?: QueryFilter }
   * Response: { results: [{ cid: string, score: number }] }
   *
   * The server accepts a Float32 embedding vector as `query` and returns
   * the top-k nearest neighbours by cosine/L2 distance from the HNSW index.
   */
  async semanticSearch(
    queryEmbedding: Float32Array,
    opts: { topK?: number; minScore?: number } = {},
  ): Promise<SemanticResult[]> {
    const body = {
      query: Array.from(queryEmbedding),   // server field name is "query"
      k: opts.topK ?? 20,
    };
    const res = await withTimeout(
      fetch(this.url("/api/v0/semantic/search"), {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      }),
      10_000,
    );
    if (!res.ok) throw new Error(`semantic/search: HTTP ${res.status}`);
    const j = await res.json() as { results?: Array<{ cid?: string; score?: number }> };
    const raw = j.results ?? [];
    const minScore = opts.minScore ?? 0.0;
    return raw
      .map((r) => ({
        cid: r.cid ?? "",
        score: Number(r.score ?? 0),
      }))
      .filter((r) => r.cid && r.score >= minScore);
  }

  /**
   * Index a CID with its embedding for later semantic search.
   *
   * POST /api/v0/semantic/index
   * Request:  { cid: string, embedding: number[] }
   * Response: { indexed: boolean }
   *
   * Called after `add()` in live mode to make content semantically searchable.
   */
  async semanticIndex(cid: string, embedding: Float32Array): Promise<boolean> {
    const body = {
      cid,
      embedding: Array.from(embedding),
    };
    const res = await withTimeout(
      fetch(this.url("/api/v0/semantic/index"), {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      }),
      10_000,
    );
    if (!res.ok) return false;
    const j = await res.json() as { indexed?: boolean };
    return j.indexed ?? false;
  }

  /**
   * Get semantic index stats from the gateway.
   *
   * GET /api/v0/semantic/stats
   * Response: { num_vectors, dimension, metric, cache_size, cache_capacity }
   */
  async semanticStats(): Promise<SemanticStats | null> {
    try {
      const res = await withTimeout(fetch(this.url("/api/v0/semantic/stats")), 5_000);
      if (!res.ok) return null;
      return await res.json() as SemanticStats;
    } catch {
      return null;
    }
  }

  async pin(cid: string): Promise<void> {
    const res = await fetch(this.url(`/api/v0/pin/add?arg=${encodeURIComponent(cid)}`), {
      method: "POST",
    });
    if (!res.ok) throw new Error(`pin: HTTP ${res.status}`);
  }

  async unpin(cid: string): Promise<void> {
    const res = await fetch(this.url(`/api/v0/pin/rm?arg=${encodeURIComponent(cid)}`), {
      method: "POST",
    });
    if (!res.ok) throw new Error(`unpin: HTTP ${res.status}`);
  }

  // ---- knowledge graph (/api/v0/knowledge/*) ----------------------------

  private async kPost<T>(path: string, body: unknown): Promise<T | null> {
    try {
      const res = await withTimeout(
        fetch(this.url(path), {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(body),
        }),
        10_000,
      );
      if (!res.ok) return null;
      return (await res.json()) as T;
    } catch {
      return null;
    }
  }

  /** GET /api/v0/knowledge/stats — null if the feature is off or unreachable. */
  async knowledgeStats(): Promise<{ entities: number; index: number } | null> {
    try {
      const res = await withTimeout(fetch(this.url("/api/v0/knowledge/stats")), 5_000);
      if (!res.ok) return null;
      return (await res.json()) as { entities: number; index: number };
    } catch {
      return null;
    }
  }

  async knowledgeAddEntity(
    kind: string,
    name: string,
    aliases: string[] = [],
    attrs: Record<string, string> = {},
  ): Promise<string | null> {
    const r = await this.kPost<{ id: string }>("/api/v0/knowledge/entity", {
      kind,
      name,
      aliases,
      attrs,
    });
    return r?.id ?? null;
  }

  async knowledgeAddRelation(
    subjectKind: string,
    subjectName: string,
    predicate: string,
    objectKind: string,
    objectName: string,
    weight = 1.0,
  ): Promise<string | null> {
    const r = await this.kPost<{ cid: string }>("/api/v0/knowledge/relation", {
      subject_kind: subjectKind,
      subject_name: subjectName,
      predicate,
      object_kind: objectKind,
      object_name: objectName,
      weight,
    });
    return r?.cid ?? null;
  }

  /** POST /api/v0/knowledge/commit — persist a head, returns its CID. */
  async knowledgeCommit(): Promise<string | null> {
    const r = await this.kPost<{ head: string }>("/api/v0/knowledge/commit", {});
    return r?.head ?? null;
  }

  /** POST /api/v0/knowledge/search — cosine top-k over the gateway's index. */
  async knowledgeSearch(query: string, k = 8): Promise<KnowledgeHit[] | null> {
    const r = await this.kPost<{ results: KnowledgeHit[] }>("/api/v0/knowledge/search", {
      query,
      k,
    });
    return r?.results ?? null;
  }

  /** GET /api/v0/knowledge/heads — live head + recent auto-pinned ring. */
  async knowledgeHeads(): Promise<{ live: string | null; recent: string[]; retain: number } | null> {
    try {
      const res = await withTimeout(fetch(this.url("/api/v0/knowledge/heads")), 5_000);
      if (!res.ok) return null;
      return (await res.json()) as { live: string | null; recent: string[]; retain: number };
    } catch {
      return null;
    }
  }

  /** POST /api/v0/knowledge/gc — mark-and-sweep; returns the report. */
  async knowledgeGc(keepHistory: boolean): Promise<{ kept: number; deleted: number; roots: number } | null> {
    return this.kPost<{ kept: number; deleted: number; roots: number }>("/api/v0/knowledge/gc", {
      keep_history: keepHistory,
    });
  }

  /** GET /api/v0/knowledge/export — the whole graph as one CAR blob. */
  async knowledgeExport(): Promise<Blob | null> {
    try {
      const res = await fetch(this.url("/api/v0/knowledge/export"));
      if (!res.ok) return null;
      return await res.blob();
    } catch {
      return null;
    }
  }

  /** GET /api/v0/knowledge/diff — incremental CAR from `from` to `to`. */
  async knowledgeDiff(to: string, from: string): Promise<Blob | null> {
    try {
      const q = `?to=${encodeURIComponent(to)}&from=${encodeURIComponent(from)}`;
      const res = await fetch(this.url("/api/v0/knowledge/diff" + q));
      if (!res.ok) return null;
      return await res.blob();
    } catch {
      return null;
    }
  }

  /** POST /api/v0/knowledge/import (CAR bytes) — returns the adopted head CID. */
  async knowledgeImport(data: ArrayBuffer): Promise<string | null> {
    try {
      const res = await withTimeout(
        fetch(this.url("/api/v0/knowledge/import"), {
          method: "POST",
          headers: { "content-type": "application/vnd.ipld.car" },
          body: data,
        }),
        20_000,
      );
      if (!res.ok) return null;
      const j = (await res.json()) as { head?: string };
      return j.head ?? null;
    } catch {
      return null;
    }
  }

  /** GET /api/v0/knowledge/projection — { "<slug>.md": markdown }. */
  async knowledgeProjection(): Promise<Record<string, string> | null> {
    try {
      const res = await withTimeout(fetch(this.url("/api/v0/knowledge/projection")), 5_000);
      if (!res.ok) return null;
      const j = (await res.json()) as { pages: Record<string, string> };
      return j.pages;
    } catch {
      return null;
    }
  }
}

// ---- Demo pseudo-CID (deterministic, offline) -----------------------------

const B32 = "abcdefghijklmnopqrstuvwxyz234567";

function base32(bytes: Uint8Array): string {
  let bits = 0;
  let value = 0;
  let out = "";
  for (const b of bytes) {
    value = (value << 8) | b;
    bits += 8;
    while (bits >= 5) {
      out += B32[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += B32[(value << (5 - bits)) & 31];
  return out;
}

/** SHA-256 → CIDv1-looking base32 string ("bafybei…"). Demo only. */
export async function demoCid(data: ArrayBuffer): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", data));
  return "bafybei" + base32(digest).slice(0, 52);
}

/** Stable pseudo-CID from a string seed (for seeded demo objects). */
export async function demoCidFromString(seed: string): Promise<string> {
  return demoCid(new TextEncoder().encode(seed).buffer as ArrayBuffer);
}

// ---- On-device char-ngram embedding (no model needed) ---------------------
//
// Produces a sparse Float32 vector suitable for cosine-similarity semantic
// search via /api/v0/semantic/search.  The gateway side is expected to have
// indexed objects using the same scheme — if it uses a real embedding model
// the scores will still be meaningful because the gateway normalises them.

const NGRAM_DIM = 768; // Matches RouterConfig::default() dimension in ipfrs-semantic

function fnv1a32(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h;
}

export function buildQueryEmbedding(query: string): Float32Array {
  const q = query.toLowerCase().trim();
  const vec = new Float32Array(NGRAM_DIM);
  // 2-gram and 3-gram bag-of-ngrams projected to fixed-dim via hash
  for (const n of [2, 3]) {
    for (let i = 0; i <= q.length - n; i++) {
      const ng = q.slice(i, i + n);
      const idx = fnv1a32(ng) % NGRAM_DIM;
      vec[idx] += 1;
    }
  }
  // L2 normalise
  let norm = 0;
  for (let i = 0; i < NGRAM_DIM; i++) norm += vec[i] * vec[i];
  norm = Math.sqrt(norm);
  if (norm > 0) for (let i = 0; i < NGRAM_DIM; i++) vec[i] /= norm;
  return vec;
}
