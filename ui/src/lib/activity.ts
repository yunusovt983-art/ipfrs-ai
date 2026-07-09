// Activity log with undo support (client-side, capped ring in localStorage).

import type { S3Object } from "../types";

export type ActivityKind =
  | "upload"
  | "delete"
  | "bulkDelete"
  | "deleteBucket"
  | "rename"
  | "restore"
  | "pin"
  | "createBucket"
  | "createFolder"
  | "import";

export type UndoAction =
  | { type: "restoreObjects"; bucket: string; objects: S3Object[] }
  | { type: "renameBack"; bucket: string; fromKey: string; toKey: string }
  | { type: "restoreBucket"; bucket: string; objects: S3Object[]; createdAt: number };

export interface ActivityEntry {
  id: number;
  ts: number;
  kind: ActivityKind;
  bucket: string;
  summary: string;
  undo?: UndoAction;
}

const KEY = "ipfrs.s3.activity";
const CAP = 80;
let seq = Date.now();

export function getActivity(): ActivityEntry[] {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as ActivityEntry[]) : [];
  } catch {
    return [];
  }
}

function save(list: ActivityEntry[]): void {
  localStorage.setItem(KEY, JSON.stringify(list.slice(0, CAP)));
}

export function logActivity(e: Omit<ActivityEntry, "id" | "ts">): ActivityEntry {
  const entry: ActivityEntry = { ...e, id: ++seq, ts: Date.now() };
  save([entry, ...getActivity()]);
  return entry;
}

export function removeActivity(id: number): void {
  save(getActivity().filter((e) => e.id !== id));
}

export function clearActivity(): void {
  localStorage.removeItem(KEY);
}
