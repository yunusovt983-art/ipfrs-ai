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

  async pin(cid: string): Promise<void> {
    // Best-effort; the gateway may not expose pin/add — ignore failures.
    await fetch(this.url(`/api/v0/pin/add?arg=${encodeURIComponent(cid)}`), {
      method: "POST",
    }).catch(() => undefined);
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
