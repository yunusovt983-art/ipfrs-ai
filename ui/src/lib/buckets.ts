// S3-bucket manifest persistence + virtual-folder derivation.
//
// Buckets and object metadata live in localStorage (the manifest). Object *bytes*
// live either on the IPFS gateway (live mode) or in an in-memory cache (demo /
// freshly-uploaded), keyed by CID.

import type { Bucket, BrowserEntry, BucketPolicy, DagNode, S3Object } from "../types";
import { demoCidFromString } from "./ipfrs";
import { guessType } from "./format";

const BUCKETS_KEY = "ipfrs.s3.buckets";
const OBJ_PREFIX = "ipfrs.s3.objs.";
const POLICY_PREFIX = "ipfrs.s3.policy.";

export const DEFAULT_POLICY: BucketPolicy = { versioning: true, autopin: false, quotaBytes: 0 };

export function getPolicy(bucket: string): BucketPolicy {
  try {
    const raw = localStorage.getItem(POLICY_PREFIX + bucket);
    return raw ? { ...DEFAULT_POLICY, ...JSON.parse(raw) } : DEFAULT_POLICY;
  } catch {
    return DEFAULT_POLICY;
  }
}

export function savePolicy(bucket: string, policy: BucketPolicy): void {
  localStorage.setItem(POLICY_PREFIX + bucket, JSON.stringify(policy));
}

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

/** Bump this when the seed data changes to force a re-seed of localStorage. */
const SEED_VERSION = "v4";
const SEED_VER_KEY = "ipfrs.s3.seed.version";

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

/** Attach illustrative DAG / proof / providers to ALL seeded objects. */
async function attachDemoIpfrs(bucket: string, objs: S3Object[]): Promise<void> {
  const cid = (seed: string) => demoCidFromString(`${bucket}/${seed}`);
  const find = (k: string) => objs.find((o) => o.key === k);

  // ── ml-models ──────────────────────────────────────────────────────────────
  if (bucket === "ml-models") {
    // llama/model.safetensors  — large sharded DAG + providers
    const model = find("llama/model.safetensors");
    if (model) {
      const shard = async (i: number, size: number): Promise<DagNode> => ({
        cid: await cid(`shard-${i}`),
        name: `shard-0000${i}.bin`,
        size,
        codec: "raw",
        links: [],
      });
      model.dag = {
        cid: model.cid,
        name: "model.safetensors",
        size: model.size,
        codec: "dag-pb",
        links: [
          {
            cid: await cid("index"),
            name: "weight_index.json",
            size: 48_210,
            codec: "dag-cbor",
            links: [],
          },
          await shard(0, 1_610_612_736),
          await shard(1, 1_610_612_736),
          {
            ...(await shard(2, 1_610_039_296)),
            links: [
              { cid: await cid("shard-2-a"), name: "chunk-a", size: 805_306_368, codec: "raw", links: [] },
              { cid: await cid("shard-2-b"), name: "chunk-b", size: 804_732_928, codec: "raw", links: [] },
            ],
          },
        ],
      };
      model.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "integrity(model.safetensors)",
          rule: "integrity(X) :- pinned(X), cid_ok(X).",
          bindings: { X: "model.safetensors" },
          sub: [
            { goal: "pinned(model.safetensors)", rule: "факт в базе знаний" },
            {
              goal: "cid_ok(model.safetensors)",
              rule: "cid_ok(X) :- hash(X) == cid(X).",
              sub: [{ goal: "hash(model.safetensors) == Qm…", rule: "SHA2-256 совпадает ✓" }],
            },
          ],
        },
      };
      model.providers = [
        { peer: "12D3KooW…Alice", region: "eu-central", rttMs: 11, role: "origin" },
        { peer: "12D3KooW…Bob", region: "us-east", rttMs: 74 },
        { peer: "12D3KooW…Carol", region: "ap-south", rttMs: 156 },
        { peer: "12D3KooW…Dave", region: "eu-west", rttMs: 29 },
      ];
    }

    // llama/config.json — small single-block DAG + proof
    const cfg = find("llama/config.json");
    if (cfg) {
      cfg.dag = {
        cid: cfg.cid,
        name: "config.json",
        size: cfg.size,
        codec: "dag-json",
        links: [],
      };
      cfg.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "valid_config(config.json)",
          rule: "valid_config(X) :- schema_ok(X), source(X).",
          bindings: { X: "config.json" },
          sub: [
            { goal: "schema_ok(config.json)", rule: "JSON schema v7 ✓" },
            { goal: "source(config.json)", rule: "derived_from(model.safetensors) ✓" },
          ],
        },
      };
      cfg.providers = [
        { peer: "12D3KooW…Alice", region: "eu-central", rttMs: 11, role: "origin" },
        { peer: "12D3KooW…Eve", region: "us-west", rttMs: 88 },
      ];
    }

    // llama/tokenizer.json
    const tok = find("llama/tokenizer.json");
    if (tok) {
      tok.dag = {
        cid: tok.cid,
        name: "tokenizer.json",
        size: tok.size,
        codec: "dag-json",
        links: [
          { cid: await cid("vocab-chunk"), name: "vocab.bin", size: 1_400_000, codec: "raw", links: [] },
          { cid: await cid("merges-chunk"), name: "merges.txt", size: 442_133, codec: "raw", links: [] },
        ],
      };
      tok.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "tokenizer_valid(tokenizer.json)",
          rule: "tokenizer_valid(X) :- vocab_ok(X), bpe_ok(X).",
          bindings: { X: "tokenizer.json" },
          sub: [
            { goal: "vocab_ok(tokenizer.json)", rule: "vocab size == 32000 ✓" },
            { goal: "bpe_ok(tokenizer.json)", rule: "merges count == 31999 ✓" },
          ],
        },
      };
      tok.providers = [
        { peer: "12D3KooW…Alice", region: "eu-central", rttMs: 11, role: "origin" },
        { peer: "12D3KooW…Frank", region: "eu-west", rttMs: 44 },
        { peer: "12D3KooW…Grace", region: "sa-east", rttMs: 210 },
      ];
    }

    // bert-base/model.safetensors
    const bert = find("bert-base/model.safetensors");
    if (bert) {
      bert.dag = {
        cid: bert.cid,
        name: "model.safetensors",
        size: bert.size,
        codec: "dag-pb",
        links: [
          { cid: await cid("bert-shard-0"), name: "shard-0.bin", size: 219_001_088, codec: "raw", links: [] },
          { cid: await cid("bert-shard-1"), name: "shard-1.bin", size: 219_001_088, codec: "raw", links: [] },
        ],
      };
      bert.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "integrity(bert-base/model.safetensors)",
          rule: "integrity(X) :- pinned(X), cid_ok(X).",
          bindings: { X: "bert-base/model.safetensors" },
          sub: [
            { goal: "pinned(bert-base/model.safetensors)", rule: "факт в базе знаний" },
            { goal: "cid_ok(bert-base/model.safetensors)", rule: "SHA2-256 совпадает ✓" },
          ],
        },
      };
      bert.providers = [
        { peer: "12D3KooW…Hank", region: "us-east", rttMs: 52, role: "origin" },
        { peer: "12D3KooW…Iris", region: "eu-central", rttMs: 31 },
      ];
    }

    // bert-base/vocab.txt
    const vocab = find("bert-base/vocab.txt");
    if (vocab) {
      vocab.dag = { cid: vocab.cid, name: "vocab.txt", size: vocab.size, codec: "raw", links: [] };
      vocab.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "vocab_authentic(vocab.txt)",
          rule: "vocab_authentic(X) :- hash_ok(X), known_source(X).",
          bindings: { X: "vocab.txt", S: "HuggingFace Hub" },
          sub: [
            { goal: "hash_ok(vocab.txt)", rule: "CID == SHA2-256(data) ✓" },
            { goal: "known_source(vocab.txt)", rule: "origin == huggingface.co ✓" },
          ],
        },
      };
      vocab.providers = [
        { peer: "12D3KooW…Hank", region: "us-east", rttMs: 52, role: "origin" },
        { peer: "12D3KooW…Jack", region: "ap-east", rttMs: 130 },
      ];
    }

    // README.md
    const readme = find("README.md");
    if (readme) {
      readme.dag = { cid: readme.cid, name: "README.md", size: readme.size, codec: "dag-json", links: [] };
      readme.proof = {
        verified: false,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "authored(README.md)",
          rule: "authored(X) :- signed(X).",
          bindings: { X: "README.md" },
          sub: [{ goal: "signed(README.md)", rule: "подпись не найдена ✗" }],
        },
      };
      readme.providers = [
        { peer: "12D3KooW…Alice", region: "eu-central", rttMs: 11, role: "origin" },
      ];
    }
  }

  // ── datasets ────────────────────────────────────────────────────────────────
  if (bucket === "datasets") {
    // manifest.json — full proof + providers
    const man = find("manifest.json");
    if (man) {
      man.dag = { cid: man.cid, name: "manifest.json", size: man.size, codec: "dag-json", links: [] };
      man.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "provenance(manifest.json)",
          rule: "derived_from(D,S) :- transform(S,D), source(S).",
          bindings: { D: "manifest.json", S: "wiki/train.parquet" },
          sub: [
            { goal: "transform(train.parquet, manifest.json)", rule: "факт в базе знаний" },
            {
              goal: "source(train.parquet)",
              rule: "verified_source(X) :- hash_ok(X).",
              sub: [{ goal: "hash_ok(train.parquet)", rule: "cid == hash(data) ✓" }],
            },
          ],
        },
      };
      man.providers = [
        { peer: "12D3KooW…Alice", region: "eu-central", rttMs: 13, role: "origin" },
        { peer: "12D3KooW…Erin", region: "us-west", rttMs: 92 },
      ];
    }

    // wiki/train.parquet
    const train = find("wiki/train.parquet");
    if (train) {
      train.dag = {
        cid: train.cid,
        name: "train.parquet",
        size: train.size,
        codec: "dag-pb",
        links: [
          { cid: await cid("train-row-group-0"), name: "row-group-0.parquet", size: 402_000_000, codec: "raw", links: [] },
          { cid: await cid("train-row-group-1"), name: "row-group-1.parquet", size: 402_000_000, codec: "raw", links: [] },
          { cid: await cid("train-row-group-2"), name: "row-group-2.parquet", size: 400_887_552, codec: "raw", links: [] },
        ],
      };
      train.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "dataset_authentic(wiki/train.parquet)",
          rule: "dataset_authentic(X) :- licensed(X), hash_ok(X).",
          bindings: { X: "wiki/train.parquet" },
          sub: [
            { goal: "licensed(wiki/train.parquet)", rule: "CC-BY-SA 4.0 ✓" },
            { goal: "hash_ok(wiki/train.parquet)", rule: "CID совпадает ✓" },
          ],
        },
      };
      train.providers = [
        { peer: "12D3KooW…Alice", region: "eu-central", rttMs: 13, role: "origin" },
        { peer: "12D3KooW…Bob", region: "us-east", rttMs: 74 },
        { peer: "12D3KooW…Carol", region: "ap-south", rttMs: 156 },
      ];
    }

    // wiki/valid.parquet
    const valid = find("wiki/valid.parquet");
    if (valid) {
      valid.dag = {
        cid: valid.cid,
        name: "valid.parquet",
        size: valid.size,
        codec: "dag-pb",
        links: [
          { cid: await cid("valid-row-group-0"), name: "row-group-0.parquet", size: 96_337_920, codec: "raw", links: [] },
        ],
      };
      valid.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "dataset_authentic(wiki/valid.parquet)",
          rule: "dataset_authentic(X) :- licensed(X), hash_ok(X).",
          bindings: { X: "wiki/valid.parquet" },
          sub: [
            { goal: "licensed(wiki/valid.parquet)", rule: "CC-BY-SA 4.0 ✓" },
            { goal: "hash_ok(wiki/valid.parquet)", rule: "CID совпадает ✓" },
          ],
        },
      };
      valid.providers = [
        { peer: "12D3KooW…Alice", region: "eu-central", rttMs: 13, role: "origin" },
        { peer: "12D3KooW…Erin", region: "us-west", rttMs: 92 },
      ];
    }

    // embeddings/vectors.bin
    const vecs = find("embeddings/vectors.bin");
    if (vecs) {
      vecs.dag = {
        cid: vecs.cid,
        name: "vectors.bin",
        size: vecs.size,
        codec: "dag-pb",
        links: [
          { cid: await cid("vec-chunk-0"), name: "chunk-0.bin", size: 134_217_728, codec: "raw", links: [] },
          { cid: await cid("vec-chunk-1"), name: "chunk-1.bin", size: 134_217_728, codec: "raw", links: [] },
        ],
      };
      vecs.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "vectors_consistent(embeddings/vectors.bin)",
          rule: "vectors_consistent(X) :- dim_ok(X), norm_ok(X).",
          bindings: { X: "vectors.bin", dim: "768" },
          sub: [
            { goal: "dim_ok(vectors.bin)", rule: "embedding dim == 768 ✓" },
            { goal: "norm_ok(vectors.bin)", rule: "L2 norms ∈ [0.98, 1.02] ✓" },
          ],
        },
      };
      vecs.providers = [
        { peer: "12D3KooW…Alice", region: "eu-central", rttMs: 13, role: "origin" },
        { peer: "12D3KooW…Frank", region: "eu-west", rttMs: 44 },
      ];
    }
  }

  // ── site-assets ─────────────────────────────────────────────────────────────
  if (bucket === "site-assets") {
    const img = find("img/banner.jpg");
    if (img) {
      img.dag = { cid: img.cid, name: "banner.jpg", size: img.size, codec: "raw", links: [] };
      img.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "asset_ok(img/banner.jpg)",
          rule: "asset_ok(X) :- mime_ok(X), cid_ok(X).",
          bindings: { X: "img/banner.jpg", mime: "image/jpeg" },
          sub: [
            { goal: "mime_ok(img/banner.jpg)", rule: "Content-Type: image/jpeg ✓" },
            { goal: "cid_ok(img/banner.jpg)", rule: "CID совпадает ✓" },
          ],
        },
      };
      img.providers = [
        { peer: "12D3KooW…Kate", region: "us-east", rttMs: 38, role: "origin" },
        { peer: "12D3KooW…Leo", region: "eu-west", rttMs: 67 },
      ];
    }

    const logo = find("img/logo.svg");
    if (logo) {
      logo.dag = { cid: logo.cid, name: "logo.svg", size: logo.size, codec: "raw", links: [] };
      logo.proof = {
        verified: true,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "asset_ok(img/logo.svg)",
          rule: "asset_ok(X) :- mime_ok(X), cid_ok(X).",
          bindings: { X: "img/logo.svg", mime: "image/svg+xml" },
          sub: [
            { goal: "mime_ok(img/logo.svg)", rule: "Content-Type: image/svg+xml ✓" },
            { goal: "cid_ok(img/logo.svg)", rule: "CID совпадает ✓" },
          ],
        },
      };
      logo.providers = [
        { peer: "12D3KooW…Kate", region: "us-east", rttMs: 38, role: "origin" },
      ];
    }

    const html = find("index.html");
    if (html) {
      html.dag = {
        cid: html.cid,
        name: "index.html",
        size: html.size,
        codec: "dag-json",
        links: [
          { cid: await cid("style-css"), name: "style.css", size: 12_450, codec: "raw", links: [] },
          { cid: await cid("app-js"), name: "app.js", size: 14_800, codec: "raw", links: [] },
        ],
      };
      html.proof = {
        verified: false,
        engine: "TensorLogic · Datalog",
        root: {
          goal: "page_signed(index.html)",
          rule: "page_signed(X) :- sig_valid(X).",
          bindings: { X: "index.html" },
          sub: [{ goal: "sig_valid(index.html)", rule: "подпись отсутствует ✗" }],
        },
      };
      html.providers = [
        { peer: "12D3KooW…Kate", region: "us-east", rttMs: 38, role: "origin" },
        { peer: "12D3KooW…Mia", region: "ap-east", rttMs: 180 },
      ];
    }
  }
}

/**
 * Non-destructive migration: enrich already-seeded objects with demo DAG / proof /
 * providers when they predate those fields (localStorage from an older build).
 * Idempotent; never touches user-uploaded objects.
 */
export async function ensureDemoData(): Promise<void> {
  for (const bucket of Object.keys(SEEDS)) {
    const objs = listObjects(bucket);
    if (!objs.length) continue;
    const key = (o: S3Object) => `${o.dag ? 1 : 0}${o.proof ? 1 : 0}${o.providers ? 1 : 0}`;
    const before = objs.map(key).join();
    await attachDemoIpfrs(bucket, objs);
    if (objs.map(key).join() !== before) saveObjects(bucket, objs);
  }
}

export async function seedIfEmpty(): Promise<void> {
  // If seed version changed, wipe and re-seed to pick up new demo data.
  const storedVer = localStorage.getItem(SEED_VER_KEY);
  if (storedVer !== SEED_VERSION) {
    // Remove bucket data (keeps user-created buckets intact only when ver matches).
    if (localStorage.getItem(BUCKETS_KEY)) {
      for (const name of Object.keys(SEEDS)) {
        localStorage.removeItem(OBJ_PREFIX + name);
      }
      // Remove bucket entries for known seed buckets so they are re-created.
      const existing: Bucket[] = listBuckets();
      const filtered = existing.filter((b) => !Object.keys(SEEDS).includes(b.name));
      if (filtered.length < existing.length) saveBuckets(filtered);
    }
    localStorage.setItem(SEED_VER_KEY, SEED_VERSION);
  }

  if (localStorage.getItem(BUCKETS_KEY)) {
    // Buckets already exist — only enrich stale objects.
    await ensureDemoData();
    return;
  }

  const now = Date.now();
  const day = 86_400_000;
  const buckets: Bucket[] = [];
  for (const [name, seeds] of Object.entries(SEEDS)) {
    buckets.push({ name, createdAt: now - 60 * day });
    const objs: S3Object[] = [];
    for (const s of seeds) {
      const base = s.key.split("/").pop() ?? s.key;
      // Give a couple of manifest/readme objects a version history for demo.
      const versioned = base === "manifest.json" || base === "README.md";
      const versions = versioned
        ? [
            {
              cid: await demoCidFromString(`${name}/${s.key}:v2`),
              size: Math.round(s.size * 0.92),
              contentType: guessType(s.key),
              lastModified: now - (s.daysAgo + 9) * day,
            },
            {
              cid: await demoCidFromString(`${name}/${s.key}:v1`),
              size: Math.round(s.size * 0.8),
              contentType: guessType(s.key),
              lastModified: now - (s.daysAgo + 25) * day,
            },
          ]
        : undefined;
      objs.push({
        key: s.key,
        cid: await demoCidFromString(`${name}/${s.key}:${s.size}`),
        size: s.size,
        contentType: guessType(s.key),
        lastModified: now - s.daysAgo * day,
        pinned: true,
        versions,
      });
    }
    await attachDemoIpfrs(name, objs);
    saveObjects(name, objs);
  }
  saveBuckets(buckets);
}

