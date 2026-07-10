// knowledge.ts — a browser-side mirror of the `ipfrs-knowledge` Rust crate:
// typed knowledge nodes, the SAME character-trigram embedding (byte-for-byte with
// vector.rs), cosine top-k search, and the deterministic wikilink Markdown
// projection from project.rs.
//
// In live mode this would query the gateway's knowledge endpoint; here it runs a
// faithful demo graph entirely on-device so the concept is verifiable in the
// browser. The embedding is intentionally identical to the Rust one — swap it for
// a learned model and everything downstream is unchanged.

export type NodeKind = "person" | "machine" | "concept";

export interface KEntity {
  kind: NodeKind;
  name: string;
  aliases: string[];
  attrs: Record<string, string>;
}

export interface KRelation {
  subject: string; // entity name
  predicate: string;
  object: string; // entity name
  weight: number;
  evidence?: string; // evidence id
}

export interface KEvidence {
  id: string;
  source: string; // the quoted source text (indexed)
}

export interface KGraph {
  entities: KEntity[];
  relations: KRelation[];
  evidence: KEvidence[];
}

// ---- embedding: faithful port of vector.rs::embed ------------------------

const FNV_OFFSET = 0xcbf29ce484222325n;
const FNV_PRIME = 0x100000001b3n;
const U64 = (1n << 64n) - 1n;

export const DEFAULT_DIM = 256;

/** Character-trigram signed-FNV hashing embedding, L2-normalized. */
export function embed(text: string, dim = DEFAULT_DIM): Float32Array {
  const v = new Float32Array(dim);
  const chars = [...` ${text.trim().toLowerCase()} `];
  for (let i = 0; i + 3 <= chars.length; i++) {
    let h = FNV_OFFSET;
    for (let j = 0; j < 3; j++) {
      h ^= BigInt(chars[i + j].codePointAt(0)!);
      h = (h * FNV_PRIME) & U64;
    }
    const idx = Number(h % BigInt(dim));
    const sign = (h >> 63n) & 1n ? -1 : 1;
    v[idx] += sign;
  }
  let mag = 0;
  for (const x of v) mag += x * x;
  mag = Math.sqrt(mag);
  if (mag > 0) for (let i = 0; i < dim; i++) v[i] /= mag;
  return v;
}

/** Cosine similarity (== dot product for normalized vectors). */
export function cosine(a: Float32Array, b: Float32Array): number {
  let s = 0;
  for (let i = 0; i < a.length; i++) s += a[i] * b[i];
  return s;
}

// ---- identity: faithful port of EntityId::of (sha256(kind\0name)) --------

/** Stable entity id = sha256(kind + 0x00 + name.trim().toLowerCase()), hex. */
export async function entityId(kind: string, name: string): Promise<string> {
  const enc = new TextEncoder();
  const k = enc.encode(kind);
  const n = enc.encode(name.trim().toLowerCase());
  const buf = new Uint8Array(k.length + 1 + n.length);
  buf.set(k, 0);
  buf[k.length] = 0;
  buf.set(n, k.length + 1);
  const digest = await crypto.subtle.digest("SHA-256", buf);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

// ---- projection: faithful port of project.rs::slug / render --------------

/** Kebab-case slug, matching project.rs::slug. */
export function slug(name: string): string {
  let out = "";
  let prevDash = false;
  for (const ch of name.trim()) {
    if (/\p{L}|\p{N}/u.test(ch)) {
      out += ch.toLowerCase();
      prevDash = false;
    } else if (!prevDash && out.length) {
      out += "-";
      prevDash = true;
    }
  }
  while (out.endsWith("-")) out = out.slice(0, -1);
  return out || "entity";
}

/** Deterministic Markdown page for an entity, with wikilinks from relations. */
export function renderMarkdown(g: KGraph, e: KEntity, idHex: string): string {
  const lines: string[] = [];
  lines.push("---");
  lines.push(`id: ${idHex}`);
  lines.push(`kind: ${e.kind}`);
  if (e.aliases.length) lines.push(`aliases: [${e.aliases.join(", ")}]`);
  lines.push("---");
  lines.push("");
  lines.push(`# ${e.name}`);
  lines.push("");
  const attrs = Object.entries(e.attrs);
  if (attrs.length) {
    lines.push("## Attributes");
    lines.push("");
    for (const [k, v] of attrs) lines.push(`- **${k}**: ${v}`);
    lines.push("");
  }
  const rels = g.relations.filter((r) => r.subject === e.name);
  if (rels.length) {
    lines.push("## Relations");
    lines.push("");
    for (const r of rels) lines.push(`- ${r.predicate} → [[${slug(r.object)}]] (${r.object}) \`w=${r.weight}\``);
    lines.push("");
  }
  return lines.join("\n");
}

// ---- search over the graph (entities + evidence source text) -------------

export interface KHit {
  kind: "entity" | "evidence";
  title: string;
  subtitle: string;
  score: number;
  entity?: KEntity;
  evidence?: KEvidence;
}

interface Indexed {
  hit: Omit<KHit, "score">;
  vec: Float32Array;
}

/** Build the on-device index: entity text (name+aliases+attrs) + evidence source. */
export function indexGraph(g: KGraph, dim = DEFAULT_DIM): Indexed[] {
  const out: Indexed[] = [];
  for (const e of g.entities) {
    const text = [e.name, ...e.aliases, ...Object.values(e.attrs)].join(" ");
    out.push({
      hit: { kind: "entity", title: e.name, subtitle: e.kind, entity: e },
      vec: embed(text, dim),
    });
  }
  for (const ev of g.evidence) {
    out.push({
      hit: { kind: "evidence", title: "Evidence", subtitle: ev.source, evidence: ev },
      vec: embed(ev.source, dim),
    });
  }
  return out;
}

/** Cosine top-k over a prebuilt index. */
export function searchIndex(index: Indexed[], query: string, k = 8, dim = DEFAULT_DIM): KHit[] {
  const q = embed(query, dim);
  return index
    .map(({ hit, vec }) => ({ ...hit, score: cosine(q, vec) }))
    .sort((a, b) => b.score - a.score)
    .slice(0, k);
}

// ---- the demo graph ------------------------------------------------------

export const DEMO_GRAPH: KGraph = {
  entities: [
    {
      kind: "person",
      name: "Ada Lovelace",
      aliases: ["Ada", "Countess of Lovelace"],
      attrs: { born: "1815", field: "mathematics", note: "first computer programmer" },
    },
    {
      kind: "person",
      name: "Charles Babbage",
      aliases: ["Babbage"],
      attrs: { born: "1791", field: "mathematics, engineering" },
    },
    {
      kind: "machine",
      name: "Analytical Engine",
      aliases: ["the Engine"],
      attrs: { designer: "Charles Babbage", year: "1837", kind: "mechanical general-purpose computer" },
    },
    {
      kind: "concept",
      name: "Algorithm",
      aliases: [],
      attrs: { domain: "computation", note: "a finite sequence of well-defined instructions" },
    },
  ],
  relations: [
    { subject: "Charles Babbage", predicate: "designed", object: "Analytical Engine", weight: 0.98 },
    { subject: "Ada Lovelace", predicate: "wrote-notes-on", object: "Analytical Engine", weight: 0.95, evidence: "ev-noteG" },
    { subject: "Ada Lovelace", predicate: "invented", object: "Algorithm", weight: 1.0, evidence: "ev-noteG" },
  ],
  evidence: [
    {
      id: "ev-noteG",
      source:
        "Note G: the first published algorithm intended to be carried out by a machine — from Menabrea's memoir on the Analytical Engine, translated with extensive notes by Ada Lovelace, 1843.",
    },
  ],
};
