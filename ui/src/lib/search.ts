// Client-side relevance ranking ("умный поиск").
//
// The gateway's /api/v0/semantic/search is embedding-based (it takes a query
// vector, not text), so true vector search needs an embedding model. Until one is
// wired, this ranks objects by lexical relevance over key + type, and — for cached
// text objects — over their actual content, returning scored, snippet-annotated
// results.

import type { S3Object } from "../types";
import { blobCache } from "./buckets";
import { isText } from "./format";

export interface Ranked {
  object: S3Object;
  score: number; // normalized 0..1
  snippet?: string;
  where: string; // "имя" | "путь" | "содержимое" | "тип"
}

function subsequence(needle: string, hay: string): boolean {
  let i = 0;
  for (let j = 0; j < hay.length && i < needle.length; j++) {
    if (hay[j] === needle[i]) i++;
  }
  return i === needle.length;
}

function snippetAround(text: string, idx: number, term: string): string {
  const start = Math.max(0, idx - 30);
  const end = Math.min(text.length, idx + term.length + 40);
  return (start > 0 ? "…" : "") + text.slice(start, end).replace(/\s+/g, " ").trim() + (end < text.length ? "…" : "");
}

export async function smartSearch(query: string, objects: S3Object[]): Promise<Ranked[]> {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const terms = q.split(/\s+/).filter(Boolean);
  const out: Ranked[] = [];

  for (const o of objects) {
    if (o.key.endsWith("/.keep")) continue;
    const key = o.key.toLowerCase();
    const base = (o.key.split("/").pop() ?? "").toLowerCase();
    const type = o.contentType.toLowerCase();
    let score = 0;
    let where = "";
    let snippet: string | undefined;

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

    // Content search for cached text objects (real "по содержимому").
    const blob = blobCache.get(o.cid);
    if (blob && blob.size <= 512 * 1024 && isText(o.contentType)) {
      const text = (await blob.text()).toLowerCase();
      for (const t of terms) {
        const idx = text.indexOf(t);
        if (idx >= 0) {
          score += 5;
          where = "содержимое";
          if (!snippet) snippet = snippetAround(text, idx, t);
        }
      }
    }

    if (score > 0) out.push({ object: o, score, snippet, where: where || "имя" });
  }

  const max = Math.max(1, ...out.map((r) => r.score));
  return out
    .map((r) => ({ ...r, score: r.score / max }))
    .sort((a, b) => b.score - a.score);
}
