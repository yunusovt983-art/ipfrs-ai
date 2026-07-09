import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import type {
  Bucket,
  BrowserEntry,
  ConnStatus,
  GatewayInfo,
  S3Object,
  Settings,
  Toast,
} from "./types";
import {
  blobCache,
  bucketStats,
  deleteBucketData,
  deriveEntries,
  listBuckets,
  listObjects,
  saveBuckets,
  saveObjects,
  seedIfEmpty,
} from "./lib/buckets";
import { demoCid, IpfrsClient } from "./lib/ipfrs";
import { guessType } from "./lib/format";
import { Sidebar } from "./components/Sidebar";
import { Toolbar } from "./components/Toolbar";
import { ObjectList } from "./components/ObjectList";
import { DetailsDrawer } from "./components/DetailsDrawer";
import { SettingsModal } from "./components/SettingsModal";
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
  const [upload, setUpload] = useState<{ done: number; total: number; name: string } | null>(null);
  const [dragging, setDragging] = useState(false);
  const [view, setView] = useState<"list" | "grid">("list");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const fileInput = useRef<HTMLInputElement>(null);
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
  const selectBucket = useCallback((name: string) => {
    setCurrentBucket(name);
    setPrefix("");
    setQuery("");
    setSelectedKey(null);
    setSelected(new Set());
    setObjects(listObjects(name));
  }, []);

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
      toast("success", `Бакет «${clean}» создан`);
    },
    [buckets, selectBucket, toast],
  );

  const deleteBucket = useCallback(
    (name: string) => {
      const next = buckets.filter((b) => b.name !== name);
      setBuckets(next);
      saveBuckets(next);
      deleteBucketData(name);
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
      let objs = listObjects(bucket);
      setUpload({ done: 0, total: list.length, name: list[0].name });
      for (let i = 0; i < list.length; i++) {
        const file = list[i];
        setUpload({ done: i, total: list.length, name: file.name });
        const key = prefix + file.name;
        try {
          let cid: string;
          let pinned = false;
          if (settings.mode === "live") {
            const r = await client.add(file);
            cid = r.cid;
            if (settings.pinOnUpload) {
              await client.pin(cid);
              pinned = true;
            }
          } else {
            const buf = await file.arrayBuffer();
            cid = await demoCid(buf);
            blobCache.set(cid, file);
            pinned = true;
          }
          const prev = objs.find((o) => o.key === key);
          const versions = prev
            ? [
                {
                  cid: prev.cid,
                  size: prev.size,
                  contentType: prev.contentType,
                  lastModified: prev.lastModified,
                },
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
        } catch (e) {
          toast("error", `Ошибка загрузки ${file.name}: ${(e as Error).message}`);
        }
      }
      persist(bucket, objs);
      setUpload(null);
      toast("success", `Загружено объектов: ${list.length}`);
    },
    [client, currentBucket, persist, prefix, settings, toast],
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
      toast("success", `Папка «${clean}» создана`);
    },
    [currentBucket, persist, prefix, toast],
  );

  // ---- object actions ---------------------------------------------------
  const deleteObject = useCallback(
    (key: string) => {
      if (!currentBucket) return;
      const objs = listObjects(currentBucket).filter((o) => o.key !== key);
      persist(currentBucket, objs);
      if (selectedKey === key) setSelectedKey(null);
      toast("info", "Объект удалён");
    },
    [currentBucket, persist, selectedKey, toast],
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

  // ---- bulk actions -----------------------------------------------------
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
    const objs = listObjects(currentBucket).filter((o) => !selected.has(o.key));
    persist(currentBucket, objs);
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

  const toggleTheme = useCallback(() => {
    const el = document.documentElement;
    const next = el.getAttribute("data-theme") === "dark" ? "light" : "dark";
    el.setAttribute("data-theme", next);
    localStorage.setItem(THEME_KEY, next);
  }, []);

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
              stats={stats}
              onNavigate={(p) => {
                setPrefix(p);
                setQuery("");
                setSelectedKey(null);
                clearSelection();
              }}
              onQuery={setQuery}
              onUpload={() => fileInput.current?.click()}
              onNewFolder={createFolder}
              onRefresh={refresh}
              onView={setView}
            />
            <ObjectList
              entries={entries}
              view={view}
              selectedKey={selectedKey}
              selected={selected}
              searching={!!query.trim()}
              onOpenFolder={(p) => {
                setPrefix(p);
                setSelectedKey(null);
                clearSelection();
              }}
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
              onUpload={() => fileInput.current?.click()}
            />
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
          onRestore={restoreVersion}
          onDelete={deleteObject}
        />
      )}

      {showSettings && (
        <SettingsModal
          settings={settings}
          onApply={applySettings}
          onClose={() => setShowSettings(false)}
        />
      )}

      <Toasts toasts={toasts} />
      {(dragging || upload) && <UploadOverlay dragging={dragging} upload={upload} />}

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
