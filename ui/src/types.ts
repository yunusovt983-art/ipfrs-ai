// Domain types for the IPFRS S3-style console.
//
// IPFS has no native "bucket / object" model — everything is a CID. This console
// layers an S3 vocabulary on top: a *bucket* is a named manifest, an *object* is a
// key → CID mapping with metadata. Folders are virtual, derived from "/" in keys.

export interface Bucket {
  name: string;
  createdAt: number;
}

/** A previous content version of an object (S3-style versioning). */
export interface ObjectVersion {
  cid: string;
  size: number;
  contentType: string;
  lastModified: number;
}

/** A node in a content-addressed DAG (demo seed or fetched). */
export interface DagNode {
  cid: string;
  name?: string;
  size: number;
  codec: string;
  links: DagNode[];
}

/** One step of a proof-carrying provenance tree. */
export interface ProofStep {
  goal: string;
  rule?: string;
  bindings?: Record<string, string>;
  sub?: ProofStep[];
}
export interface Provenance {
  verified: boolean;
  engine: string;
  root: ProofStep;
}

/** A peer that provides (holds) a CID. */
export interface Provider {
  peer: string;
  region: string;
  rttMs: number;
  role?: string;
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
  /** Prior versions, newest first. Grows each time the key is re-uploaded. */
  versions?: ObjectVersion[];
  /** Demo-seeded DAG structure (live mode fetches the real one). */
  dag?: DagNode;
  /** Demo-seeded provenance proof (live mode fetches from /logic/proof). */
  proof?: Provenance;
  /** Demo-seeded providers (live mode queries /dht/findprovs + /swarm/peers). */
  providers?: Provider[];
}

/** A row in the object browser — either a virtual folder or a real object. */
export type BrowserEntry =
  | { kind: "folder"; name: string; prefix: string; count: number; size: number }
  | { kind: "object"; name: string; object: S3Object };

/** Per-bucket policy (stored client-side). */
export interface BucketPolicy {
  versioning: boolean;
  autopin: boolean;
  /** Soft quota in bytes; 0 = unlimited. */
  quotaBytes: number;
}

export type UploadStatus = "pending" | "uploading" | "done" | "error" | "cancelled";
export interface UploadItem {
  name: string;
  size: number;
  status: UploadStatus;
  error?: string;
}

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
