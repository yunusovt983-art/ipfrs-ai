// Raw block / DAG inspection helpers.
//
// The gateway's /api/v0/dag/get returns the raw block bytes (base64), not parsed
// IPLD links. These helpers show a hex view, guess the format from magic bytes,
// and best-effort scan for DAG-CBOR CID links (tag 42) so a block's children can
// be surfaced without a full IPLD decoder.

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

export function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const a = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) a[i] = bin.charCodeAt(i);
  return a;
}

export function detectFormat(bytes: Uint8Array): string {
  const b = bytes;
  const m = (arr: number[]) => arr.every((v, i) => b[i] === v);
  if (b.length >= 4 && m([0x89, 0x50, 0x4e, 0x47])) return "PNG";
  if (b.length >= 3 && m([0xff, 0xd8, 0xff])) return "JPEG";
  if (b.length >= 4 && m([0x47, 0x49, 0x46, 0x38])) return "GIF";
  if (b.length >= 4 && m([0x25, 0x50, 0x44, 0x46])) return "PDF";
  if (b.length >= 2 && m([0x1f, 0x8b])) return "gzip";
  if (b.length >= 4 && m([0x50, 0x4b, 0x03, 0x04])) return "zip";
  if (b.length >= 8 && m([0x00, 0x61, 0x73, 0x6d])) return "wasm";
  // CBOR major types map/array/tag are common in DAG-CBOR blocks.
  const t = b[0] >> 5;
  if (t === 5 || t === 4 || t === 6) return "CBOR (возможно DAG-CBOR)";
  // Printable-heavy prefix → text (bytes ≥128 counted as UTF-8 multibyte).
  let printable = 0;
  const n = Math.min(b.length, 256);
  for (let i = 0; i < n; i++) {
    const c = b[i];
    if (c === 9 || c === 10 || c === 13 || (c >= 32 && c < 127) || c >= 128) printable++;
  }
  if (n > 0 && printable / n > 0.9) return "текст (UTF-8)";
  return "двоичные данные";
}

export function hexDump(bytes: Uint8Array, max = 320): string {
  const n = Math.min(bytes.length, max);
  const lines: string[] = [];
  for (let off = 0; off < n; off += 16) {
    const row = bytes.slice(off, off + 16);
    const hex = [...row].map((b) => b.toString(16).padStart(2, "0")).join(" ");
    const ascii = [...row].map((b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : ".")).join("");
    lines.push(`${off.toString(16).padStart(6, "0")}  ${hex.padEnd(47)}  ${ascii}`);
  }
  if (bytes.length > n) lines.push(`… +${bytes.length - n} байт`);
  return lines.join("\n");
}

/** Read a CBOR byte-string length starting at `i` (major type 2). Returns [len, headerBytes] or null. */
function readByteStringLen(b: Uint8Array, i: number): [number, number] | null {
  const ib = b[i];
  if (ib >> 5 !== 2) return null;
  const low = ib & 0x1f;
  if (low < 24) return [low, 1];
  if (low === 24) return [b[i + 1], 2];
  if (low === 25) return [(b[i + 1] << 8) | b[i + 2], 3];
  if (low === 26) return [(b[i + 1] << 24) | (b[i + 2] << 16) | (b[i + 3] << 8) | b[i + 4], 5];
  return null;
}

/**
 * Scan for DAG-CBOR CID links: tag 42 (0xD8 0x2A) + byte string whose content is
 * `0x00` (identity multibase prefix) followed by the CIDv1 bytes. Returns CIDv1
 * base32 strings.
 */
export function scanCidLinks(bytes: Uint8Array): string[] {
  const found = new Set<string>();
  for (let i = 0; i + 1 < bytes.length; i++) {
    if (bytes[i] !== 0xd8 || bytes[i + 1] !== 0x2a) continue;
    const bs = readByteStringLen(bytes, i + 2);
    if (!bs) continue;
    const [len, hdr] = bs;
    const start = i + 2 + hdr;
    if (len < 2 || start + len > bytes.length) continue;
    if (bytes[start] !== 0x00) continue; // identity multibase prefix
    const cidBytes = bytes.slice(start + 1, start + len);
    if (cidBytes[0] !== 0x01) continue; // CIDv1
    found.add("b" + base32(cidBytes));
  }
  return [...found];
}
