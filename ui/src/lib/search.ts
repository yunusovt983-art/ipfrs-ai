// Client-side relevance ranking ("умный поиск").
//
// Three-tier approach:
//
// Tier 1: Lexical — exact/partial match on key, type, tag.
//         Boosts: exact basename > path contain > subsequence > type.
//
// Tier 2: Char-ngram TF-IDF — represents every string as a bag of 2-3 char
//         n-grams, then scores via cosine similarity.  No embedding model
//         needed; works well for Cyrillic & Latin, fuzzy typos, prefix/suffix.
//
// Tier 3: Content — for text blobs cached in blobCache, lexical scan +
//         snippet extraction.

import type { S3Object } from "../types";
import { blobCache } from "./buckets";
import { isText } from "./format";

export interface Ranked {
  object: S3Object;
  score: number;       // normalised 0..1
  snippet?: string;
  where: string;       // "имя" | "путь" | "содержимое" | "тип" | "ngram"
  /** Term-level highlight regions: [{start, end}] in the key string */
  highlights?: HighlightRange[];
  /** Snippet with the matching portion already annotated */
  snippetHighlight?: HighlightRange[];
}

export interface HighlightRange {
  start: number;
  end: number;
}

// ── helpers ────────────────────────────────────────────────────────────────

function subsequence(needle: string, hay: string): boolean {
  let i = 0;
  for (let j = 0; j < hay.length && i < needle.length; j++) {
    if (hay[j] === needle[i]) i++;
  }
  return i === needle.length;
}

function snippetAround(text: string, idx: number, term: string, window = 40): string {
  const start = Math.max(0, idx - window);
  const end   = Math.min(text.length, idx + term.length + window);
  const pre   = start > 0 ? "…" : "";
  const post  = end < text.length ? "…" : "";
  return pre + text.slice(start, end).replace(/\s+/g, " ").trim() + post;
}

/** Find all non-overlapping occurrences of `term` in `text` (lowercased). */
function findAllOccurrences(text: string, term: string): number[] {
  const positions: number[] = [];
  let idx = 0;
  while ((idx = text.indexOf(term, idx)) !== -1) {
    positions.push(idx);
    idx += term.length;
  }
  return positions;
}

/** Build highlight ranges for all terms found in `text` (lowercased). */
function buildHighlights(text: string, terms: string[]): HighlightRange[] {
  const ranges: HighlightRange[] = [];
  for (const t of terms) {
    for (const pos of findAllOccurrences(text.toLowerCase(), t)) {
      ranges.push({ start: pos, end: pos + t.length });
    }
  }
  // Merge overlapping ranges.
  return mergeRanges(ranges);
}

function mergeRanges(ranges: HighlightRange[]): HighlightRange[] {
  if (!ranges.length) return [];
  const sorted = [...ranges].sort((a, b) => a.start - b.start);
  const merged: HighlightRange[] = [sorted[0]];
  for (let i = 1; i < sorted.length; i++) {
    const last = merged[merged.length - 1];
    if (sorted[i].start <= last.end) {
      last.end = Math.max(last.end, sorted[i].end);
    } else {
      merged.push(sorted[i]);
    }
  }
  return merged;
}

// ── char n-gram TF-IDF ────────────────────────────────────────────────────

const NGRAM_SIZES = [2, 3] as const;

function charNgrams(text: string): Map<string, number> {
  const freq = new Map<string, number>();
  const s = text.toLowerCase().replace(/\s+/g, " ").trim();
  for (const n of NGRAM_SIZES) {
    for (let i = 0; i <= s.length - n; i++) {
      const ng = s.slice(i, i + n);
      freq.set(ng, (freq.get(ng) ?? 0) + 1);
    }
  }
  return freq;
}

function cosineSim(a: Map<string, number>, b: Map<string, number>): number {
  let dot = 0;
  let normA = 0;
  let normB = 0;
  for (const [k, v] of a) {
    dot += v * (b.get(k) ?? 0);
    normA += v * v;
  }
  for (const [, v] of b) normB += v * v;
  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

// ── main export ───────────────────────────────────────────────────────────

export async function smartSearch(query: string, objects: S3Object[]): Promise<Ranked[]> {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const terms = q.split(/\s+/).filter(Boolean);
  const queryNgrams = charNgrams(q);

  const out: Ranked[] = [];

  for (const o of objects) {
    if (o.key.endsWith("/.keep")) continue;

    const key  = o.key.toLowerCase();
    const base = (o.key.split("/").pop() ?? "").toLowerCase();
    const type = o.contentType.toLowerCase();

    let score = 0;
    let where = "";
    let snippet: string | undefined;
    let snippetHighlight: HighlightRange[] | undefined;

    // ── Tier 1: lexical ──────────────────────────────────────────────────
    for (const t of terms) {
      if (base.includes(t)) {
        score += 6;
        where ||= "имя";
      } else if (key.includes(t)) {
        score += 3;
        where ||= "путь";
      } else if (subsequence(t, base)) {
        score += 1;
        where ||= "имя~";
      }
      if (type.includes(t)) {
        score += 2;
        where ||= "тип";
      }
    }

    // ── Tier 2: char-ngram cosine ─────────────────────────────────────────
    // score the key + base against the query
    const keySim   = cosineSim(queryNgrams, charNgrams(key));
    const baseSim  = cosineSim(queryNgrams, charNgrams(base));
    const typeSim  = cosineSim(queryNgrams, charNgrams(type));
    const ngramMax = Math.max(keySim, baseSim, typeSim);
    if (ngramMax > 0.08) {
      // weight: 4 if base similarity is highest, else 2
      const weight = baseSim >= keySim && baseSim >= typeSim ? 4 : 2;
      score += ngramMax * weight;
      if (!where) where = "ngram";
    }

    // ── Tier 3: content (cached text blobs) ───────────────────────────────
    const blob = blobCache.get(o.cid);
    if (blob && blob.size <= 512 * 1024 && isText(o.contentType)) {
      const text = (await blob.text()).toLowerCase();
      for (const t of terms) {
        const idx = text.indexOf(t);
        if (idx >= 0) {
          score += 5;
          where = "содержимое";
          if (!snippet) {
            const raw = snippetAround(text, idx, t);
            snippet = raw;
            // highlight region relative to snippet start (offset by leading "…")
            const preLen  = idx > 40 ? 1 : 0; // "…" = 1 char
            const relStart = Math.min(idx, 40) + preLen;
            snippetHighlight = [{ start: relStart, end: relStart + t.length }];
          }
        }
      }
    }

    if (score > 0) {
      // Build key highlight ranges.
      const highlights = buildHighlights(o.key, terms);
      out.push({ object: o, score, snippet, where: where || "имя", highlights, snippetHighlight });
    }
  }

  const max = Math.max(1, ...out.map((r) => r.score));
  return out
    .map((r) => ({ ...r, score: r.score / max }))
    .sort((a, b) => b.score - a.score);
}
