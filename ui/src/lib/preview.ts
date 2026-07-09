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

// ── Parquet footer decoder (pure JS, no wasm) ──────────────────────────────
//
// Parquet file layout:
//   4-byte magic  PAR1
//   <row groups>
//   <footer bytes>
//   4-byte LE footer length
//   4-byte magic  PAR1
//
// The footer is Thrift-encoded.  We implement a minimal Thrift binary decoder
// that only reads the fields we care about:
//   FileMetaData.schema (field 2, list of SchemaElement)
//   FileMetaData.num_rows (field 3, i64)
//   FileMetaData.row_groups (field 4, list)
//   FileMetaData.version (field 1, i32)
//
// Thrift binary wire types:
//   0=STOP  1=BOOL_T  2=BOOL_F  3=BYTE  4=DOUBLE  5=I16  6=I32  8=I64
//  11=BINARY  12=STRUCT  13=MAP  14=SET  15=LIST

export interface ParquetColumn {
  name: string;
  type: string;
  repetition: "REQUIRED" | "OPTIONAL" | "REPEATED";
  logicalType?: string;
}

export interface ParquetFooter {
  version: number;
  numRows: number;
  rowGroupCount: number;
  totalByteSize: number;
  columns: ParquetColumn[];
  truncated: boolean;
}

// Thrift wire types
const T_STOP = 0, T_BOOL = 2, T_BYTE = 3, T_I16 = 5, T_I32 = 6, T_I64 = 8,
      T_DOUBLE = 4, T_BINARY = 11, T_STRUCT = 12, T_MAP = 13,
      T_SET = 14, T_LIST = 15;

// Parquet physical types
const PARQUET_TYPES: Record<number, string> = {
  0: "BOOLEAN", 1: "INT32", 2: "INT64", 3: "INT96",
  4: "FLOAT", 5: "DOUBLE", 6: "BYTE_ARRAY", 7: "FIXED_LEN_BYTE_ARRAY",
};
const REPETITION: Record<number, ParquetColumn["repetition"]> = {
  0: "REQUIRED", 1: "OPTIONAL", 2: "REPEATED",
};

class ThriftReader {
  pos = 0;
  constructor(private buf: Uint8Array) {}

  remaining() { return this.buf.length - this.pos; }

  readByte(): number {
    if (this.pos >= this.buf.length) throw new Error("eof");
    return this.buf[this.pos++];
  }

  readI16(): number {
    const v = (this.buf[this.pos] << 8) | this.buf[this.pos + 1];
    this.pos += 2;
    return v > 32767 ? v - 65536 : v;
  }

  readI32(): number {
    const b = this.buf;
    const v = ((b[this.pos] << 24) | (b[this.pos+1] << 16) | (b[this.pos+2] << 8) | b[this.pos+3]) >>> 0;
    this.pos += 4;
    return v > 2147483647 ? v - 4294967296 : v;
  }

  readI64(): number {
    // Read as two 32-bit halves; we only care about values < 2^53
    const hi = this.readI32();
    const lo = this.readI32() >>> 0;
    return hi * 4294967296 + lo;
  }

  readDouble(): number {
    const b = this.buf.slice(this.pos, this.pos + 8);
    this.pos += 8;
    return new DataView(b.buffer, b.byteOffset).getFloat64(0, false);
  }

  readBinary(): string {
    const len = this.readI32();
    if (len < 0 || len > this.remaining()) return "";
    const s = new TextDecoder("utf-8", { fatal: false }).decode(
      this.buf.slice(this.pos, this.pos + len),
    );
    this.pos += len;
    return s;
  }

  /** Skip a full Thrift field value of given type. */
  skipValue(type: number): void {
    switch (type) {
      case T_BOOL: case T_BYTE: this.pos++; break;
      case T_I16: this.pos += 2; break;
      case T_I32: this.pos += 4; break;
      case T_I64: case T_DOUBLE: this.pos += 8; break;
      case T_BINARY: { const n = this.readI32(); this.pos += Math.max(0, n); break; }
      case T_STRUCT: this.skipStruct(); break;
      case T_LIST: case T_SET: this.skipListOrSet(); break;
      case T_MAP: this.skipMap(); break;
    }
  }

  skipStruct(): void {
    while (true) {
      const type = this.readByte();
      if (type === T_STOP) break;
      this.readI16(); // field id
      this.skipValue(type);
    }
  }

  skipListOrSet(): void {
    const et = this.readByte();
    const n  = this.readI32();
    for (let i = 0; i < n; i++) this.skipValue(et);
  }

  skipMap(): void {
    const kt = this.readByte();
    const vt = this.readByte();
    const n  = this.readI32();
    for (let i = 0; i < n; i++) { this.skipValue(kt); this.skipValue(vt); }
  }

  /** Read a SchemaElement struct → ParquetColumn (partial). */
  readSchemaElement(): Partial<ParquetColumn> & { numChildren?: number } {
    const col: Partial<ParquetColumn> & { numChildren?: number } = {};
    while (true) {
      const type = this.readByte();
      if (type === T_STOP) break;
      const fid = this.readI16();
      if (fid === 1 && type === T_I32) { col.type = PARQUET_TYPES[this.readI32()] ?? "UNKNOWN"; }
      else if (fid === 2 && type === T_I32) { /* type_length */ this.readI32(); }
      else if (fid === 3 && type === T_I32) { col.repetition = REPETITION[this.readI32()] ?? "OPTIONAL"; }
      else if (fid === 4 && type === T_BINARY) { col.name = this.readBinary(); }
      else if (fid === 5 && type === T_I32) { col.numChildren = this.readI32(); }
      else if (fid === 6 && type === T_STRUCT) {
        // ConvertedType is encoded as struct-wrapped enum; skip
        this.skipStruct();
      }
      else this.skipValue(type);
    }
    return col;
  }

  /** Read a RowGroup struct → total byte size. */
  readRowGroup(): number {
    let totalByteSz = 0;
    while (true) {
      const type = this.readByte();
      if (type === T_STOP) break;
      const fid = this.readI16();
      if (fid === 3 && type === T_I64) totalByteSz += this.readI64();
      else this.skipValue(type);
    }
    return totalByteSz;
  }
}

export function parseParquet(bytes: Uint8Array): ParquetFooter | null {
  if (bytes.length < 12) return null;
  // Check magic PAR1
  const magic = String.fromCharCode(...bytes.slice(0, 4));
  if (magic !== "PAR1") return null;

  // Last 8 bytes: [4-byte footer len LE][4-byte magic PAR1]
  const tailStart = bytes.length - 8;
  if (tailStart < 4) return null;
  const dv = new DataView(bytes.buffer, bytes.byteOffset);
  const footerLen = dv.getUint32(tailStart, true);
  if (footerLen <= 0 || footerLen > bytes.length - 8) {
    // Footer not fully present in our slice
    return { version: 0, numRows: 0, rowGroupCount: 0, totalByteSize: 0, columns: [], truncated: true };
  }

  const footerStart = tailStart - footerLen;
  const r = new ThriftReader(bytes.slice(footerStart, tailStart));

  let version = 0, numRows = 0, rowGroupCount = 0, totalByteSize = 0;
  const columns: ParquetColumn[] = [];
  let truncated = false;

  try {
    while (true) {
      const type = r.readByte();
      if (type === T_STOP) break;
      const fid = r.readI16();

      if (fid === 1 && type === T_I32) {
        version = r.readI32();
      } else if (fid === 2 && type === T_LIST) {
        // schema: list<SchemaElement>
        const et = r.readByte(); // element type (should be T_STRUCT=12)
        const n  = r.readI32();
        for (let i = 0; i < n; i++) {
          if (et === T_STRUCT) {
            const elem = r.readSchemaElement();
            // Only leaf nodes (numChildren undefined or 0) are columns
            if (!elem.numChildren && elem.name) {
              columns.push({
                name: elem.name,
                type: elem.type ?? "UNKNOWN",
                repetition: elem.repetition ?? "OPTIONAL",
              });
            }
          } else {
            r.skipValue(et);
          }
        }
      } else if (fid === 3 && type === T_I64) {
        numRows = r.readI64();
      } else if (fid === 4 && type === T_LIST) {
        // row_groups
        const et = r.readByte();
        const n  = r.readI32();
        rowGroupCount = n;
        for (let i = 0; i < n; i++) {
          if (et === T_STRUCT) totalByteSize += r.readRowGroup();
          else r.skipValue(et);
        }
      } else {
        r.skipValue(type);
      }
    }
  } catch {
    truncated = true;
  }

  return { version, numRows, rowGroupCount, totalByteSize, columns, truncated };
}

// ── CSV / TSV parser (first N rows) ──────────────────────────────────────────

export interface CsvPreview {
  headers: string[];
  rows: string[][];
  totalLines: number;
  delimiter: "," | "\t" | ";";
}

function detectDelimiter(firstLine: string): CsvPreview["delimiter"] {
  const tabs = (firstLine.match(/\t/g) ?? []).length;
  const commas = (firstLine.match(/,/g) ?? []).length;
  const semis = (firstLine.match(/;/g) ?? []).length;
  if (tabs >= commas && tabs >= semis) return "\t";
  if (semis > commas) return ";";
  return ",";
}

function splitCsvLine(line: string, delim: string): string[] {
  const cells: string[] = [];
  let cur = "";
  let inQ = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (ch === '"') {
      if (inQ && line[i + 1] === '"') { cur += '"'; i++; }
      else inQ = !inQ;
    } else if (ch === delim && !inQ) {
      cells.push(cur); cur = "";
    } else {
      cur += ch;
    }
  }
  cells.push(cur);
  return cells.map((c) => c.trim());
}

export function parseCsv(text: string, maxRows = 50): CsvPreview {
  const lines = text.split(/\r?\n/).filter(Boolean);
  if (!lines.length) return { headers: [], rows: [], totalLines: 0, delimiter: "," };
  const delimiter = detectDelimiter(lines[0]);
  const headers = splitCsvLine(lines[0], delimiter);
  const rows = lines.slice(1, maxRows + 1).map((l) => splitCsvLine(l, delimiter));
  return { headers, rows, totalLines: lines.length - 1, delimiter };
}
