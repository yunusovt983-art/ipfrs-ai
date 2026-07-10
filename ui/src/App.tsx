import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import type {
  Bucket,
  BrowserEntry,
  ConnStatus,
  GatewayInfo,
  S3Object,
  Settings,
  Toast,
  UploadItem,
} from "./types";
import {
  blobCache,
  bucketStats,
  deleteBucketData,
  deriveEntries,
  ensureDemoData,
  getPolicy,
  listBuckets,
  listObjects,
  saveBuckets,
  saveObjects,
  seedIfEmpty,
} from "./lib/buckets";
import { demoCid, IpfrsClient, buildQueryEmbedding } from "./lib/ipfrs";
import { guessType } from "./lib/format";
import { smartSearch, type Ranked } from "./lib/search";
import {
  clearActivity,
  getActivity,
  logActivity,
  removeActivity,
  type ActivityEntry,
} from "./lib/activity";
import { Sidebar } from "./components/Sidebar";
import { Toolbar } from "./components/Toolbar";
import { ObjectList } from "./components/ObjectList";
import { SmartResults } from "./components/SmartResults";
import { SemanticSearchPanel } from "./components/SemanticSearchPanel";
import { DetailsDrawer } from "./components/DetailsDrawer";
import { BlockInspector } from "./components/BlockInspector";
import { DagExplorer } from "./components/DagExplorer";
import { ProvenanceModal } from "./components/ProvenanceModal";
import { ProvidersModal } from "./components/ProvidersModal";
import { SettingsModal } from "./components/SettingsModal";
import { BucketPolicyModal } from "./components/BucketPolicyModal";
import { BucketMetricsModal } from "./components/BucketMetricsModal";
import { KnowledgeModal } from "./components/KnowledgeModal";
import { ActivityPanel } from "./components/ActivityPanel";
import { Toasts } from "./components/Toasts";
import { UploadOverlay } from "./components/UploadOverlay";

const SETTINGS_KEY = "ipfrs.s3.settings";
const THEME_KEY = "ipfrs.s3.theme";

const DEFAULT_SETTINGS: Settings = {
  mode: "demo",
  gateway: "http://127.0.0.1:8080",
  pinOnUpload: true,
};

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    return raw ? { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } : DEFAULT_SETTINGS;
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function App() {
  const [buckets, setBuckets] = useState<Bucket[]>([]);
  const [currentBucket, setCurrentBucket] = useState<string | null>(null);
  const [prefix, setPrefix] = useState("");
  const [objects, setObjects] = useState<S3Object[]>([]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [settings, setSettings] = useState<Settings>(loadSettings);
  const [conn, setConn] = useState<ConnStatus>("unknown");
  const [info, setInfo] = useState<GatewayInfo | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [showKnowledge, setShowKnowledge] = useState(false);
  const [uploadItems, setUploadItems] = useState<UploadItem[]>([]);
  const [dragging, setDragging] = useState(false);
  const [view, setView] = useState<"list" | "grid">("list");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [smart, setSmart] = useState(false);
  const [semantic, setSemantic] = useState(false);
  const [ranked, setRanked] = useState<Ranked[]>([]);
  const [inspect, setInspect] = useState<S3Object | null>(null);
  const [dagObj, setDagObj] = useState<S3Object | null>(null);
  const [proofObj, setProofObj] = useState<S3Object | null>(null);
  const [providersObj, setProvidersObj] = useState<S3Object | null>(null);
  const [policyBucket, setPolicyBucket] = useState<string | null>(null);
  const [metricsBucket, setMetricsBucket] = useState<string | null>(null);
  const [activity, setActivity] = useState<ActivityEntry[] | null>(null);
  const [hist, setHist] = useState<{ bucket: string; prefix: string }[]>([]);
  const [histIdx, setHistIdx] = useState(-1);
  const fileInput = useRef<HTMLInputElement>(null);
  const cancelUpload = useRef(false);
  const toastId = useRef(0);

  const client = useMemo(() => new IpfrsClient(settings.gateway), [settings.gateway]);

  const toast = useCallback((kind: Toast["kind"], message: string) => {
    const id = ++toastId.current;
    setToasts((t) => [...t, { id, kind, message }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4200);
  }, []);

  // ---- init -------------------------------------------------------------
  useEffect(() => {
    const saved = localStorage.getItem(THEME_KEY);
    if (saved) document.documentElement.setAttribute("data-theme", saved);
    (async () => {
      await seedIfEmpty();
      await ensureDemoData();
      const bs = listBuckets();
      setBuckets(bs);
      if (bs.length) selectBucket(bs[0].name);
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ---- connection probe (live mode) -------------------------------------
  const testConnection = useCallback(async () => {
    if (settings.mode !== "live") {
      setConn("unknown");
      setInfo(null);
      return;
    }
    setConn("connecting");
    try {
      const i = await client.info();
      setInfo(i);
      setConn("online");
    } catch {
      setInfo(null);
      setConn("offline");
    }
  }, [client, settings.mode]);

  useEffect(() => {
    testConnection();
  }, [testConnection]);

  // ---- bucket / navigation ---------------------------------------------
  const applyLocation = useCallback((bucket: string, pfx: string) => {
    setCurrentBucket(bucket);
    setPrefix(pfx);
    setQuery("");
    setSelectedKey(null);
    setSelected(new Set());
    setObjects(listObjects(bucket));
  }, []);

  const navigateTo = useCallback(
    (bucket: string, pfx: string) => {
      applyLocation(bucket, pfx);
      setHist((h) => [...h.slice(0, histIdx + 1), { bucket, prefix: pfx }]);
      setHistIdx((i) => i + 1);
    },
    [applyLocation, histIdx],
  );

  const selectBucket = useCallback((name: string) => navigateTo(name, ""), [navigateTo]);

  const back = useCallback(() => {
    if (histIdx <= 0) return;
    const i = histIdx - 1;
    setHistIdx(i);
    applyLocation(hist[i].bucket, hist[i].prefix);
  }, [applyLocation, hist, histIdx]);

  const forward = useCallback(() => {
    if (histIdx >= hist.length - 1) return;
    const i = histIdx + 1;
    setHistIdx(i);
    applyLocation(hist[i].bucket, hist[i].prefix);
  }, [applyLocation, hist, histIdx]);

  const refresh = useCallback(() => {
    if (currentBucket) setObjects(listObjects(currentBucket));
  }, [currentBucket]);

  const persist = useCallback(
    (name: string, next: S3Object[]) => {
      saveObjects(name, next);
      if (name === currentBucket) setObjects(next);
    },
    [currentBucket],
  );

  const createBucket = useCallback(
    (name: string) => {
      const clean = name.trim().toLowerCase().replace(/[^a-z0-9-]/g, "-");
      if (!clean) return;
      if (buckets.some((b) => b.name === clean)) {
        toast("error", `Бакет «${clean}» уже существует`);
        return;
      }
      const next = [...buckets, { name: clean, createdAt: Date.now() }];
      setBuckets(next);
      saveBuckets(next);
      saveObjects(clean, []);
      selectBucket(clean);
      logActivity({ kind: "createBucket", bucket: clean, summary: `Создан бакет «${clean}»` });
      toast("success", `Бакет «${clean}» создан`);
    },
    [buckets, selectBucket, toast],
  );

  const deleteBucket = useCallback(
    (name: string) => {
      const removedObjs = listObjects(name);
      const removedBucket = buckets.find((b) => b.name === name);
      const next = buckets.filter((b) => b.name !== name);
      setBuckets(next);
      saveBuckets(next);
      deleteBucketData(name);
      logActivity({
        kind: "deleteBucket",
        bucket: name,
        summary: `Удалён бакет «${name}» (${removedObjs.length} объектов)`,
        undo: {
          type: "restoreBucket",
          bucket: name,
          objects: removedObjs,
          createdAt: removedBucket?.createdAt ?? Date.now(),
        },
      });
      if (currentBucket === name) {
        if (next.length) selectBucket(next[0].name);
        else {
          setCurrentBucket(null);
          setObjects([]);
        }
      }
      toast("info", `Бакет «${name}» удалён`);
    },
    [buckets, currentBucket, selectBucket, toast],
  );

  // ---- upload -----------------------------------------------------------
  const uploadFiles = useCallback(
    async (files: FileList | File[]) => {
      if (!currentBucket) {
        toast("error", "Сначала выберите бакет");
        return;
      }
      const list = Array.from(files);
      if (!list.length) return;
      const bucket = currentBucket;
      const policy = getPolicy(bucket);
      cancelUpload.current = false;
      setUploadItems(list.map((f) => ({ name: f.name, size: f.size, status: "pending", file: f })));

      let objs = listObjects(bucket);
      if (policy.quotaBytes > 0) {
        const used = objs.reduce((s, o) => s + o.size, 0);
        const incoming = list.reduce((s, f) => s + f.size, 0);
        if (used + incoming > policy.quotaBytes) {
          toast("error", `Квота бакета превышена: использовано ${Math.round(used / 1024 / 1024)} МБ из ${Math.round(policy.quotaBytes / 1024 / 1024)} МБ`);
          setUploadItems([]);
          return;
        }
      }

      let done = 0;
      for (let i = 0; i < list.length; i++) {
        if (cancelUpload.current) {
          setUploadItems((its) =>
            its.map((it, idx) => (idx >= i && it.status === "pending" ? { ...it, status: "cancelled" } : it)),
          );
          break;
        }
        const file = list[i];
        setUploadItems((its) => its.map((it, idx) => (idx === i ? { ...it, status: "uploading" } : it)));
        const key = prefix + file.name;
        try {
          let cid: string;
          let pinned = false;
          if (settings.mode === "live") {
            const r = await client.addWithProgress(file, (pct) => {
              setUploadItems((its) =>
                its.map((it, idx) => (idx === i ? { ...it, progress: pct } : it)),
              );
            });
            cid = r.cid;
            if (settings.pinOnUpload || policy.autopin) {
              try {
                await client.pin(cid);
                pinned = true;
              } catch {
                pinned = false;
              }
            }
            // Auto-index in semantic search after upload
            try {
              const embedding = buildQueryEmbedding(file.name + " " + (file.type || ""));
              await client.semanticIndex(cid, embedding);
            } catch {
              // Non-critical — search still works via ngram fallback
            }
          } else {
            // Demo: compute pseudo-CID + animate upload progress
            const buf = await file.arrayBuffer();
            cid = await demoCid(buf);
            blobCache.set(cid, file);
            // Simulate progress animation
            await new Promise<void>((resolve) => {
              const steps = [10, 35, 65, 88, 100];
              let s = 0;
              const tick = () => {
                const pct = steps[s++];
                setUploadItems((its) =>
                  its.map((it, idx) =>
                    idx === i ? { ...it, progress: pct } : it,
                  ),
                );
                if (s < steps.length) setTimeout(tick, 60);
                else resolve();
              };
              setTimeout(tick, 40);
            });
            pinned = true;
          }
          const prev = objs.find((o) => o.key === key);
          const versions =
            policy.versioning && prev
              ? [
                  { cid: prev.cid, size: prev.size, contentType: prev.contentType, lastModified: prev.lastModified },
                  ...(prev.versions ?? []),
                ]
              : undefined;
          const obj: S3Object = {
            key,
            cid,
            size: file.size,
            contentType: file.type || guessType(file.name),
            lastModified: Date.now(),
            pinned,
            versions,
          };
          objs = [...objs.filter((o) => o.key !== key), obj];
          persist(bucket, objs);
          done++;
          setUploadItems((its) => its.map((it, idx) => (idx === i ? { ...it, status: "done" } : it)));
        } catch (e) {
          setUploadItems((its) =>
            its.map((it, idx) => (idx === i ? { ...it, status: "error", error: (e as Error).message } : it)),
          );
        }
      }
      if (done) {
        logActivity({ kind: "upload", bucket, summary: `Загружено объектов: ${done}` });
        toast("success", `Загружено объектов: ${done}`);
      }
      setTimeout(() => setUploadItems([]), 3000);
    },
    [client, currentBucket, persist, prefix, settings, toast],
  );

  const cancelUploads = useCallback(() => {
    cancelUpload.current = true;
    toast("info", "Загрузка отменяется…");
  }, [toast]);

  /** Retry a single failed upload by index. */
  const retryUpload = useCallback(
    async (idx: number) => {
      const item = uploadItems[idx];
      if (!item?.file || !currentBucket) return;
      const bucket = currentBucket;
      const policy = getPolicy(bucket);
      const file = item.file;
      const key = prefix + file.name;

      setUploadItems((its) =>
        its.map((it, i) => (i === idx ? { ...it, status: "uploading", error: undefined, progress: 0 } : it)),
      );

      try {
        let cid: string;
        let pinned = false;
        if (settings.mode === "live") {
          const r = await client.addWithProgress(file, (pct) => {
            setUploadItems((its) =>
              its.map((it, i) => (i === idx ? { ...it, progress: pct } : it)),
            );
          });
          cid = r.cid;
          if (settings.pinOnUpload || policy.autopin) {
            try { await client.pin(cid); pinned = true; } catch { pinned = false; }
          }
        } else {
          const buf = await file.arrayBuffer();
          cid = await demoCid(buf);
          blobCache.set(cid, file);
          await new Promise<void>((resolve) => {
            const steps = [10, 35, 65, 88, 100];
            let s = 0;
            const tick = () => {
              setUploadItems((its) => its.map((it, i) => (i === idx ? { ...it, progress: steps[s] } : it)));
              if (++s < steps.length) setTimeout(tick, 60);
              else resolve();
            };
            setTimeout(tick, 40);
          });
          pinned = true;
        }
        let objs = listObjects(bucket);
        const prev = objs.find((o) => o.key === key);
        const versions =
          policy.versioning && prev
            ? [{ cid: prev.cid, size: prev.size, contentType: prev.contentType, lastModified: prev.lastModified }, ...(prev.versions ?? [])]
            : undefined;
        const obj: S3Object = { key, cid, size: file.size, contentType: file.type || guessType(file.name), lastModified: Date.now(), pinned, versions };
        objs = [...objs.filter((o) => o.key !== key), obj];
        persist(bucket, objs);
        setUploadItems((its) => its.map((it, i) => (i === idx ? { ...it, status: "done" } : it)));
        toast("success", `Повторно загружен: ${file.name}`);
      } catch (e) {
        setUploadItems((its) =>
          its.map((it, i) => (i === idx ? { ...it, status: "error", error: (e as Error).message } : it)),
        );
      }
    },
    [client, currentBucket, persist, prefix, settings, toast, uploadItems],
  );

  const createFolder = useCallback(
    (name: string) => {
      const clean = name.trim().replace(/^\/+|\/+$/g, "");
      if (!clean || !currentBucket) return;
      const key = `${prefix}${clean}/.keep`;
      const objs = listObjects(currentBucket);
      if (objs.some((o) => o.key === key)) return;
      persist(currentBucket, [
        ...objs,
        {
          key,
          cid: "",
          size: 0,
          contentType: "application/x-directory",
          lastModified: Date.now(),
          pinned: false,
        },
      ]);
      logActivity({ kind: "createFolder", bucket: currentBucket, summary: `Создана папка «${prefix}${clean}/»` });
      toast("success", `Папка «${clean}» создана`);
    },
    [currentBucket, persist, prefix, toast],
  );

  // ---- object actions ---------------------------------------------------
  const deleteObject = useCallback(
    (key: string) => {
      if (!currentBucket) return;
      const all = listObjects(currentBucket);
      const removed = all.find((o) => o.key === key);
      persist(currentBucket, all.filter((o) => o.key !== key));
      if (selectedKey === key) setSelectedKey(null);
      if (removed) {
        logActivity({
          kind: "delete",
          bucket: currentBucket,
          summary: `Удалён объект «${key}»`,
          undo: { type: "restoreObjects", bucket: currentBucket, objects: [removed] },
        });
      }
      toast("info", "Объект удалён");
    },
    [currentBucket, persist, selectedKey, toast],
  );

  const togglePin = useCallback(
    async (obj: S3Object) => {
      if (!currentBucket) return;
      const next = !obj.pinned;
      if (settings.mode === "live" && obj.cid) {
        try {
          if (next) await client.pin(obj.cid);
          else await client.unpin(obj.cid);
        } catch (e) {
          toast("error", `Pin: ${(e as Error).message}`);
          return;
        }
      }
      const objs = listObjects(currentBucket).map((o) =>
        o.key === obj.key ? { ...o, pinned: next } : o,
      );
      persist(currentBucket, objs);
      logActivity({
        kind: "pin",
        bucket: currentBucket,
        summary: `${next ? "Закреплён" : "Откреплён"} «${obj.key}»`,
      });
      toast(next ? "success" : "info", next ? "Закреплено (pinned)" : "Откреплено");
    },
    [client, currentBucket, persist, settings.mode, toast],
  );

  const foldersAt = useCallback(
    (pfx: string): string[] =>
      deriveEntries(objects, pfx)
        .filter((e) => e.kind === "folder")
        .map((e) => (e as { name: string }).name),
    [objects],
  );

  const downloadByCid = useCallback(
    async (cid: string, filename: string) => {
      try {
        let blob: Blob | undefined = blobCache.get(cid);
        if (!blob && settings.mode === "live" && cid) {
          const res = await fetch(client.catUrl(cid));
          if (!res.ok) throw new Error(`HTTP ${res.status}`);
          blob = await res.blob();
        }
        if (!blob) {
          toast("info", "Демо-объект без реального содержимого");
          return;
        }
        const a = document.createElement("a");
        const url = URL.createObjectURL(blob);
        a.href = url;
        a.download = filename;
        a.click();
        URL.revokeObjectURL(url);
      } catch (e) {
        toast("error", `Скачивание не удалось: ${(e as Error).message}`);
      }
    },
    [client, settings.mode, toast],
  );

  const download = useCallback(
    (obj: S3Object) => downloadByCid(obj.cid, obj.key.split("/").pop() || "download"),
    [downloadByCid],
  );

  const restoreVersion = useCallback(
    (key: string, versionCid: string) => {
      if (!currentBucket) return;
      const objs = listObjects(currentBucket);
      const idx = objs.findIndex((o) => o.key === key);
      if (idx === -1) return;
      const o = objs[idx];
      const ver = (o.versions ?? []).find((v) => v.cid === versionCid);
      if (!ver) return;
      // Push the current content into history, promote the chosen version.
      const rest = (o.versions ?? []).filter((v) => v.cid !== versionCid);
      const nextVersions = [
        { cid: o.cid, size: o.size, contentType: o.contentType, lastModified: o.lastModified },
        ...rest,
      ];
      const restored: S3Object = {
        ...o,
        cid: ver.cid,
        size: ver.size,
        contentType: ver.contentType,
        lastModified: Date.now(),
        versions: nextVersions,
      };
      const next = [...objs];
      next[idx] = restored;
      persist(currentBucket, next);
      logActivity({ kind: "restore", bucket: currentBucket, summary: `Восстановлена версия «${key}»` });
      toast("success", "Версия восстановлена");
    },
    [currentBucket, persist, toast],
  );

  const shareLink = useCallback(
    (obj: S3Object) => {
      if (!obj.cid) {
        toast("info", "У объекта нет CID");
        return;
      }
      const url = client.ipfsUrl(obj.cid);
      navigator.clipboard?.writeText(url).then(
        () =>
          toast(
            "success",
            settings.mode === "live" ? "Ссылка на шлюз скопирована" : "Ссылка скопирована (демо-CID)",
          ),
        () => toast("error", "Не удалось скопировать"),
      );
    },
    [client, settings.mode, toast],
  );

  // ---- rename -----------------------------------------------------------
  const renameObject = useCallback(
    (oldKey: string, newBasename: string) => {
      if (!currentBucket) return;
      const newBaseTrimmed = newBasename.trim().replace(/\//g, "");
      if (!newBaseTrimmed) return;
      const prefix = oldKey.includes("/") ? oldKey.slice(0, oldKey.lastIndexOf("/") + 1) : "";
      const newKey = prefix + newBaseTrimmed;
      const objs = listObjects(currentBucket);
      if (objs.some((o) => o.key === newKey)) {
        toast("error", `Объект «${newKey}» уже существует`);
        return;
      }
      const next = objs.map((o) => (o.key === oldKey ? { ...o, key: newKey } : o));
      persist(currentBucket, next);
      if (selectedKey === oldKey) setSelectedKey(newKey);
      logActivity({
        kind: "rename",
        bucket: currentBucket,
        summary: `«${oldKey}» → «${newKey}»`,
        undo: { type: "renameBack", bucket: currentBucket, fromKey: newKey, toKey: oldKey },
      });
      toast("success", `Переименовано: ${newBaseTrimmed}`);
    },
    [currentBucket, persist, selectedKey, toast],
  );

  const toggleSelect = useCallback((key: string) => {
    setSelected((s) => {
      const n = new Set(s);
      n.has(key) ? n.delete(key) : n.add(key);
      return n;
    });
  }, []);

  const toggleSelectAll = useCallback((keys: string[]) => {
    setSelected((s) => {
      const allSelected = keys.length > 0 && keys.every((k) => s.has(k));
      return allSelected ? new Set() : new Set(keys);
    });
  }, []);

  const clearSelection = useCallback(() => setSelected(new Set()), []);

  const bulkDelete = useCallback(() => {
    if (!currentBucket || !selected.size) return;
    const all = listObjects(currentBucket);
    const removed = all.filter((o) => selected.has(o.key));
    persist(currentBucket, all.filter((o) => !selected.has(o.key)));
    logActivity({
      kind: "bulkDelete",
      bucket: currentBucket,
      summary: `Удалено объектов: ${removed.length}`,
      undo: { type: "restoreObjects", bucket: currentBucket, objects: removed },
    });
    toast("info", `Удалено объектов: ${selected.size}`);
    setSelected(new Set());
    setSelectedKey(null);
  }, [currentBucket, persist, selected, toast]);

  const bulkDownload = useCallback(async () => {
    if (!currentBucket || !selected.size) return;
    const objs = listObjects(currentBucket).filter((o) => selected.has(o.key));
    for (const o of objs) {
      await download(o);
    }
  }, [currentBucket, download, selected]);

  const copyCid = useCallback(
    (cid: string) => {
      navigator.clipboard?.writeText(cid).then(
        () => toast("success", "CID скопирован"),
        () => toast("error", "Не удалось скопировать"),
      );
    },
    [toast],
  );

  const applySettings = useCallback(
    (s: Settings) => {
      setSettings(s);
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
      setShowSettings(false);
    },
    [],
  );

  // ---- manifest export / import ----------------------------------------
  const exportManifest = useCallback(
    (bucket: string) => {
      const data = {
        format: "ipfrs-s3-manifest/1",
        bucket,
        exportedAt: new Date().toISOString(),
        objects: listObjects(bucket),
      };
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
      const a = document.createElement("a");
      const url = URL.createObjectURL(blob);
      a.href = url;
      a.download = `${bucket}-manifest.json`;
      a.click();
      URL.revokeObjectURL(url);
      toast("success", "Манифест экспортирован");
    },
    [toast],
  );

  const importManifest = useCallback(
    async (bucket: string, file: File) => {
      try {
        const data = JSON.parse(await file.text());
        const incoming = (Array.isArray(data) ? data : data.objects) as S3Object[];
        if (!Array.isArray(incoming)) throw new Error("нет массива objects");
        const valid = incoming.filter(
          (o) => o && typeof o.key === "string" && typeof o.cid === "string",
        );
        const map = new Map(listObjects(bucket).map((o) => [o.key, o]));
        for (const o of valid) map.set(o.key, o);
        persist(bucket, [...map.values()]);
        logActivity({ kind: "import", bucket, summary: `Импортировано объектов: ${valid.length}` });
        toast("success", `Импортировано объектов: ${valid.length}`);
      } catch (e) {
        toast("error", `Импорт не удался: ${(e as Error).message}`);
      }
    },
    [persist, toast],
  );

  // ---- activity log / undo ---------------------------------------------
  const undoActivity = useCallback(
    (entry: ActivityEntry) => {
      const u = entry.undo;
      if (u) {
        if (u.type === "restoreObjects") {
          const map = new Map(listObjects(u.bucket).map((o) => [o.key, o]));
          for (const o of u.objects) map.set(o.key, o);
          persist(u.bucket, [...map.values()]);
          toast("success", "Отменено: объекты восстановлены");
        } else if (u.type === "renameBack") {
          const objs = listObjects(u.bucket).map((o) =>
            o.key === u.fromKey ? { ...o, key: u.toKey } : o,
          );
          persist(u.bucket, objs);
          toast("success", "Отменено: имя восстановлено");
        } else if (u.type === "restoreBucket") {
          if (!buckets.some((b) => b.name === u.bucket)) {
            const nb = [...buckets, { name: u.bucket, createdAt: u.createdAt }];
            setBuckets(nb);
            saveBuckets(nb);
          }
          saveObjects(u.bucket, u.objects);
          if (u.bucket === currentBucket) setObjects(u.objects);
          toast("success", "Отменено: бакет восстановлен");
        }
      }
      removeActivity(entry.id);
      setActivity(getActivity());
    },
    [buckets, currentBucket, persist, toast],
  );

  const toggleTheme = useCallback(() => {
    const el = document.documentElement;
    const next = el.getAttribute("data-theme") === "dark" ? "light" : "dark";
    el.setAttribute("data-theme", next);
    localStorage.setItem(THEME_KEY, next);
  }, []);

  // ---- keyboard shortcuts -----------------------------------------------
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      const isEditing =
        tag === "INPUT" || tag === "TEXTAREA" || (e.target as HTMLElement).isContentEditable;

      // ⌘K / Ctrl+K — focus search (allowed even when editing)
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        const el = document.querySelector<HTMLInputElement>(".search input");
        if (el) { el.focus(); el.select(); }
        return;
      }

      if (isEditing) return;

      // Escape — close modals / drawer / clear search in priority order
      if (e.key === "Escape") {
        if (showKnowledge) { setShowKnowledge(false); return; }
        if (inspect)       { setInspect(null);       return; }
        if (dagObj)        { setDagObj(null);         return; }
        if (proofObj)      { setProofObj(null);       return; }
        if (providersObj)  { setProvidersObj(null);   return; }
        if (showSettings)  { setShowSettings(false);  return; }
        if (policyBucket)  { setPolicyBucket(null);   return; }
        if (metricsBucket) { setMetricsBucket(null);  return; }
        if (activity)      { setActivity(null);       return; }
        if (selectedKey)   { setSelectedKey(null);    return; }
        if (query)         { setQuery("");            return; }
        return;
      }

      // Del — delete selected objects (requires confirmation)
      if (e.key === "Delete" && selected.size > 0) {
        e.preventDefault();
        if (confirm(`Удалить ${selected.size} объект(ов)?`)) bulkDelete();
        return;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [
    inspect, dagObj, proofObj, providersObj, showSettings, policyBucket,
    metricsBucket, activity, selectedKey, query, selected, bulkDelete, showKnowledge,
  ]);

  // ---- derived ----------------------------------------------------------
  const entries: BrowserEntry[] = useMemo(() => {
    if (query.trim()) {
      const q = query.trim().toLowerCase();
      return objects
        .filter((o) => !o.key.endsWith("/.keep") && o.key.toLowerCase().includes(q))
        .sort((a, b) => a.key.localeCompare(b.key))
        .map((object) => ({ kind: "object", name: object.key, object }));
    }
    return deriveEntries(objects, prefix);
  }, [objects, prefix, query]);

  const selectedObj = useMemo(
    () => objects.find((o) => o.key === selectedKey) ?? null,
    [objects, selectedKey],
  );
  const stats = useMemo(() => bucketStats(objects.filter((o) => !o.key.endsWith("/.keep"))), [objects]);

  // Smart-search ranking (async: reads cached text content).
  useEffect(() => {
    let alive = true;
    if (smart && query.trim()) {
      smartSearch(query, objects).then((r) => {
        if (alive) setRanked(r);
      });
    } else {
      setRanked([]);
    }
    return () => {
      alive = false;
    };
  }, [smart, query, objects]);

  // ---- drag & drop ------------------------------------------------------
  const onDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      setDragging(false);
      if (e.dataTransfer.files.length) uploadFiles(e.dataTransfer.files);
    },
    [uploadFiles],
  );

  return (
    <div
      className="app"
      onDragOver={(e) => {
        e.preventDefault();
        if (!dragging) setDragging(true);
      }}
      onDragLeave={(e) => {
        if (e.currentTarget === e.target) setDragging(false);
      }}
      onDrop={onDrop}
    >
      <Sidebar
        buckets={buckets}
        current={currentBucket}
        conn={conn}
        info={info}
        mode={settings.mode}
        onSelect={selectBucket}
        onCreate={createBucket}
        onDelete={deleteBucket}
        onPolicy={(name) => setPolicyBucket(name)}
        onActivity={() => setActivity(getActivity())}
        onOpenSettings={() => setShowSettings(true)}
        onToggleTheme={toggleTheme}
      />

      <main className="main">
        {currentBucket ? (
          <>
            <Toolbar
              bucket={currentBucket}
              prefix={prefix}
              query={query}
              view={view}
              smart={smart}
              stats={stats}
              canBack={histIdx > 0}
              canForward={histIdx < hist.length - 1}
              foldersAt={foldersAt}
              onBack={back}
              onForward={forward}
              onNavigate={(p) => navigateTo(currentBucket, p)}
              onQuery={setQuery}
              onSmart={setSmart}
              onSemantic={setSemantic}
              semantic={semantic}
              onUpload={() => fileInput.current?.click()}
              onNewFolder={createFolder}
              onRefresh={refresh}
              onMetrics={() => setMetricsBucket(currentBucket)}
              onKnowledge={() => setShowKnowledge(true)}
              onView={setView}
            />
            {smart && query.trim() ? (
              <SmartResults
                query={query}
                results={ranked}
                onOpen={(k) => setSelectedKey(k)}
                onDownload={download}
              />
            ) : semantic && query.trim() ? (
              <SemanticSearchPanel
                query={query}
                objects={objects}
                mode={settings.mode}
                client={client}
                onOpen={(k) => setSelectedKey(k)}
                onDownload={download}
              />
            ) : (
            <ObjectList
              entries={entries}
              view={view}
              selectedKey={selectedKey}
              selected={selected}
              searching={!!query.trim()}
              onOpenFolder={(p) => navigateTo(currentBucket, p)}
              onOpenObject={(k) => setSelectedKey(k)}
              onToggle={toggleSelect}
              onToggleAll={toggleSelectAll}
              onClearSelection={clearSelection}
              onBulkDelete={bulkDelete}
              onBulkDownload={bulkDownload}
              onDownload={download}
              onDelete={deleteObject}
              onCopy={copyCid}
              onShare={shareLink}
              onPin={togglePin}
              onUpload={() => fileInput.current?.click()}
              onRename={renameObject}
            />
            )}
          </>
        ) : (
          <div className="empty-state big">
            <p>Нет бакетов. Создайте первый в боковой панели.</p>
          </div>
        )}
      </main>

      {selectedObj && (
        <DetailsDrawer
          object={selectedObj}
          mode={settings.mode}
          client={client}
          onClose={() => setSelectedKey(null)}
          onDownload={download}
          onDownloadVersion={downloadByCid}
          onCopy={copyCid}
          onShare={shareLink}
          onPin={togglePin}
          onRestore={restoreVersion}
          onInspect={(o) => setInspect(o)}
          onDag={(o) => setDagObj(o)}
          onProof={(o) => setProofObj(o)}
          onProviders={(o) => setProvidersObj(o)}
          onDelete={deleteObject}
        />
      )}

      {inspect && (
        <BlockInspector
          object={inspect}
          mode={settings.mode}
          client={client}
          onClose={() => setInspect(null)}
          onCopy={copyCid}
        />
      )}
      {dagObj && (
        <DagExplorer
          object={dagObj}
          mode={settings.mode}
          client={client}
          onClose={() => setDagObj(null)}
          onCopy={copyCid}
        />
      )}
      {proofObj && (
        <ProvenanceModal
          object={proofObj}
          mode={settings.mode}
          client={client}
          onClose={() => setProofObj(null)}
        />
      )}
      {providersObj && (
        <ProvidersModal
          object={providersObj}
          mode={settings.mode}
          client={client}
          onClose={() => setProvidersObj(null)}
          onCopy={copyCid}
        />
      )}

      {showSettings && (
        <SettingsModal
          settings={settings}
          onApply={applySettings}
          onClose={() => setShowSettings(false)}
        />
      )}
      {policyBucket && (
        <BucketPolicyModal
          bucket={policyBucket}
          usedBytes={stats.size}
          onClose={() => setPolicyBucket(null)}
          onSaved={() => toast("success", "Политики сохранены")}
        />
      )}
      {metricsBucket && (
        <BucketMetricsModal
          bucket={metricsBucket}
          onClose={() => setMetricsBucket(null)}
          onExport={exportManifest}
          onImport={importManifest}
        />
      )}
      {showKnowledge && (
        <KnowledgeModal
          mode={settings.mode}
          client={client}
          onClose={() => setShowKnowledge(false)}
        />
      )}
      {activity && (
        <ActivityPanel
          entries={activity}
          onUndo={undoActivity}
          onClear={() => {
            clearActivity();
            setActivity([]);
          }}
          onClose={() => setActivity(null)}
        />
      )}

      <Toasts toasts={toasts} />
      {(dragging || uploadItems.length > 0) && (
      <UploadOverlay dragging={dragging} items={uploadItems} onCancel={cancelUploads} onRetry={retryUpload} />
      )}

      <input
        ref={fileInput}
        type="file"
        multiple
        hidden
        onChange={(e) => {
          if (e.target.files) uploadFiles(e.target.files);
          e.target.value = "";
        }}
      />
    </div>
  );
}
