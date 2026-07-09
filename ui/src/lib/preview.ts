// Content preview helpers: fetch a prefix of an object's bytes and interpret it
// as text/JSON/code, or parse a safetensors header into a tensor summary.

import type { ConnMode, S3Object } from "../types";
import type { IpfrsClient } from "./ipfrs";
import { blobCache } from "./buckets";

/** Fetch up to `maxBytes` of an object's content (cached blob or live gateway). */
export async function fetchBytes(
  obj: S3Object,
  mode: ConnMode,
  client: IpfrsClient,
  maxBytes = 1_048_576,
): Promise<Uint8Array | null> {
  const blob = blobCache.get(obj.cid);
  if (blob) {
    const buf = await blob.slice(0, maxBytes).arrayBuffer();
    return new Uint8Array(buf);
  }
  if (mode === "live" && obj.cid) {
    try {
      const res = await fetch(client.ipfsUrl(obj.cid), {
        headers: { Range: `bytes=0-${maxBytes - 1}` },
      });
      if (!res.ok && res.status !== 206) return null;
      const buf = await res.arrayBuffer();
      return new Uint8Array(buf.slice(0, maxBytes));
    } catch {
      return null;
    }
  }
  return null;
}

export interface TensorInfo {
  name: string;
  dtype: string;
  shape: number[];
  params: number;
}
export interface SafetensorsInfo {
  tensors: TensorInfo[];
  totalParams: number;
  count: number;
  dtypes: string[];
  truncated: boolean;
}

/** Parse a `.safetensors` header (8-byte LE length + JSON metadata). */
export function parseSafetensors(bytes: Uint8Array): SafetensorsInfo | null {
  if (bytes.length < 8) return null;
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const headerLen = Number(dv.getBigUint64(0, true));
  if (!Number.isFinite(headerLen) || headerLen <= 0 || headerLen > 100_000_000) return null;
  const available = bytes.length - 8;
  const truncated = headerLen > available;
  const slice = bytes.slice(8, 8 + Math.min(headerLen, available));
  let json: Record<string, unknown>;
  try {
    json = JSON.parse(new TextDecoder().decode(slice));
  } catch {
    return truncated
      ? { tensors: [], totalParams: 0, count: 0, dtypes: [], truncated: true }
      : null;
  }
  const tensors: TensorInfo[] = [];
  let total = 0;
  const dtypes = new Set<string>();
  for (const [name, meta] of Object.entries(json)) {
    if (name === "__metadata__" || typeof meta !== "object" || meta === null) continue;
    const m = meta as { dtype?: string; shape?: number[] };
    const shape = Array.isArray(m.shape) ? m.shape : [];
    const params = shape.reduce((a, b) => a * b, 1);
    total += params;
    if (m.dtype) dtypes.add(m.dtype);
    tensors.push({ name, dtype: m.dtype ?? "?", shape, params });
  }
  tensors.sort((a, b) => b.params - a.params);
  return { tensors, totalParams: total, count: tensors.length, dtypes: [...dtypes], truncated };
}

export function decodeText(bytes: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: false }).decode(bytes);
}

export function prettyJson(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}

/** Heuristic: does this byte prefix look like binary (NUL / many control chars)? */
export function looksBinary(bytes: Uint8Array): boolean {
  const n = Math.min(bytes.length, 1024);
  let ctrl = 0;
  for (let i = 0; i < n; i++) {
    const b = bytes[i];
    if (b === 0) return true;
    if (b < 9 || (b > 13 && b < 32)) ctrl++;
  }
  return ctrl / Math.max(n, 1) > 0.15;
}

export function humanCount(n: number): string {
  if (n >= 1e9) return (n / 1e9).toFixed(2) + "B";
  if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
  if (n >= 1e3) return (n / 1e3).toFixed(1) + "K";
  return String(n);
}
