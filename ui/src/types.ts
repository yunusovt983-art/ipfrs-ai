// Domain types for the IPFRS S3-style console.
//
// IPFS has no native "bucket / object" model — everything is a CID. This console
// layers an S3 vocabulary on top: a *bucket* is a named manifest, an *object* is a
// key → CID mapping with metadata. Folders are virtual, derived from "/" in keys.

export interface Bucket {
  name: string;
  createdAt: number;
}

export interface S3Object {
  /** Full key within the bucket, e.g. "docs/spec/readme.md". */
  key: string;
  /** Content identifier returned by IPFS `add` (or a demo pseudo-CID). */
  cid: string;
  size: number;
  contentType: string;
  lastModified: number;
  /** Whether the object is pinned on the gateway (best-effort). */
  pinned: boolean;
}

/** A row in the object browser — either a virtual folder or a real object. */
export type BrowserEntry =
  | { kind: "folder"; name: string; prefix: string; count: number; size: number }
  | { kind: "object"; name: string; object: S3Object };

export type ConnMode = "demo" | "live";

export interface Settings {
  mode: ConnMode;
  /** Gateway base URL for live mode, e.g. http://127.0.0.1:8080 */
  gateway: string;
  /** Pin objects on upload (live mode). */
  pinOnUpload: boolean;
}

export type ConnStatus = "unknown" | "connecting" | "online" | "offline";

export interface GatewayInfo {
  version?: string;
  peerId?: string;
  peers?: number;
}

export interface Toast {
  id: number;
  kind: "success" | "error" | "info";
  message: string;
}
