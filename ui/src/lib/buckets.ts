// S3-bucket manifest persistence + virtual-folder derivation.
//
// Buckets and object metadata live in localStorage (the manifest). Object *bytes*
// live either on the IPFS gateway (live mode) or in an in-memory cache (demo /
// freshly-uploaded), keyed by CID.

import type { Bucket, BrowserEntry, S3Object } from "../types";
import { demoCidFromString } from "./ipfrs";
import { guessType } from "./format";

const BUCKETS_KEY = "ipfrs.s3.buckets";
const OBJ_PREFIX = "ipfrs.s3.objs.";

/** In-memory content cache (CID → Blob) for preview/download without a gateway. */
export const blobCache = new Map<string, Blob>();

export function listBuckets(): Bucket[] {
  try {
    const raw = localStorage.getItem(BUCKETS_KEY);
    return raw ? (JSON.parse(raw) as Bucket[]) : [];
  } catch {
    return [];
  }
}

export function saveBuckets(buckets: Bucket[]): void {
  localStorage.setItem(BUCKETS_KEY, JSON.stringify(buckets));
}

export function listObjects(bucket: string): S3Object[] {
  try {
    const raw = localStorage.getItem(OBJ_PREFIX + bucket);
    return raw ? (JSON.parse(raw) as S3Object[]) : [];
  } catch {
    return [];
  }
}

export function saveObjects(bucket: string, objs: S3Object[]): void {
  localStorage.setItem(OBJ_PREFIX + bucket, JSON.stringify(objs));
}

export function deleteBucketData(bucket: string): void {
  localStorage.removeItem(OBJ_PREFIX + bucket);
}

/**
 * Derive the browser rows for `prefix` within `objects`: virtual folders (the
 * next path segment) followed by the objects that sit directly under the prefix.
 */
export function deriveEntries(objects: S3Object[], prefix: string): BrowserEntry[] {
  const folders = new Map<string, { count: number; size: number }>();
  const files: S3Object[] = [];
  for (const o of objects) {
    if (!o.key.startsWith(prefix)) continue;
    const rest = o.key.slice(prefix.length);
    const slash = rest.indexOf("/");
    if (slash === -1) {
      if (rest === ".keep") continue; // empty-folder marker, not a real file
      files.push(o);
    } else {
      const folder = rest.slice(0, slash);
      const agg = folders.get(folder) ?? { count: 0, size: 0 };
      agg.count += 1;
      agg.size += o.size;
      folders.set(folder, agg);
    }
  }
  const folderEntries: BrowserEntry[] = [...folders.entries()]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([name, agg]) => ({
      kind: "folder",
      name,
      prefix: prefix + name + "/",
      count: agg.count,
      size: agg.size,
    }));
  const fileEntries: BrowserEntry[] = files
    .sort((a, b) => a.key.localeCompare(b.key))
    .map((object) => ({
      kind: "object",
      name: object.key.slice(prefix.length),
      object,
    }));
  return [...folderEntries, ...fileEntries];
}

export function bucketStats(objects: S3Object[]): { count: number; size: number } {
  return {
    count: objects.length,
    size: objects.reduce((s, o) => s + o.size, 0),
  };
}

// ---- Demo seed ------------------------------------------------------------

interface Seed {
  key: string;
  size: number;
  daysAgo: number;
}

const SEEDS: Record<string, Seed[]> = {
  "ml-models": [
    { key: "llama/config.json", size: 812, daysAgo: 12 },
    { key: "llama/model.safetensors", size: 4_831_264_768, daysAgo: 12 },
    { key: "llama/tokenizer.json", size: 1_842_133, daysAgo: 12 },
    { key: "bert-base/model.safetensors", size: 438_002_176, daysAgo: 40 },
    { key: "bert-base/vocab.txt", size: 231_508, daysAgo: 40 },
    { key: "README.md", size: 2_048, daysAgo: 3 },
  ],
  datasets: [
    { key: "wiki/train.parquet", size: 1_204_887_552, daysAgo: 8 },
    { key: "wiki/valid.parquet", size: 96_337_920, daysAgo: 8 },
    { key: "embeddings/vectors.bin", size: 268_435_456, daysAgo: 21 },
    { key: "manifest.json", size: 1_536, daysAgo: 1 },
  ],
  "site-assets": [
    { key: "img/banner.jpg", size: 204_128, daysAgo: 30 },
    { key: "img/logo.svg", size: 3_204, daysAgo: 30 },
    { key: "index.html", size: 28_698, daysAgo: 2 },
  ],
};

export async function seedIfEmpty(): Promise<void> {
  if (localStorage.getItem(BUCKETS_KEY)) return;
  const now = Date.now();
  const day = 86_400_000;
  const buckets: Bucket[] = [];
  for (const [name, seeds] of Object.entries(SEEDS)) {
    buckets.push({ name, createdAt: now - 60 * day });
    const objs: S3Object[] = [];
    for (const s of seeds) {
      objs.push({
        key: s.key,
        cid: await demoCidFromString(`${name}/${s.key}:${s.size}`),
        size: s.size,
        contentType: guessType(s.key),
        lastModified: now - s.daysAgo * day,
        pinned: true,
      });
    }
    saveObjects(name, objs);
  }
  saveBuckets(buckets);
}
