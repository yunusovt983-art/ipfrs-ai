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

/** A generated placeholder "thumbnail" (data-URI SVG) for demo image objects
 * that have no real bytes — deterministic hue from the filename. */
export function demoImagePlaceholder(name: string): string {
  const ext = (name.split(".").pop() || "IMG").toUpperCase().slice(0, 5);
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) % 360;
  const h2 = (h + 55) % 360;
  const svg =
    `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 400 250'>` +
    `<defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'>` +
    `<stop offset='0' stop-color='hsl(${h},65%,52%)'/>` +
    `<stop offset='1' stop-color='hsl(${h2},68%,42%)'/></linearGradient></defs>` +
    `<rect width='400' height='250' fill='url(#g)'/>` +
    `<g fill='none' stroke='rgba(255,255,255,.85)' stroke-width='7' stroke-linejoin='round' stroke-linecap='round'>` +
    `<rect x='150' y='95' width='100' height='75' rx='8'/>` +
    `<circle cx='176' cy='120' r='9'/>` +
    `<path d='M156 165l24-22 14 12 20-18 26 24'/></g>` +
    `<text x='200' y='205' fill='rgba(255,255,255,.95)' font-family='sans-serif' font-size='19' font-weight='700' text-anchor='middle'>${escapeXml(name)}</text>` +
    `<text x='200' y='228' fill='rgba(255,255,255,.7)' font-family='sans-serif' font-size='12' text-anchor='middle'>демо-превью · ${ext}</text>` +
    `</svg>`;
  return "data:image/svg+xml," + encodeURIComponent(svg);
}

function escapeXml(s: string): string {
  return s.replace(/[<>&'"]/g, (c) =>
    ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", "'": "&apos;", '"': "&quot;" })[c] as string,
  );
}

export function humanCount(n: number): string {
  if (n >= 1e9) return (n / 1e9).toFixed(2) + "B";
  if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
  if (n >= 1e3) return (n / 1e3).toFixed(1) + "K";
  return String(n);
}
