// Small formatting + mime helpers.

export function humanSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function relTime(ts: number): string {
  const d = Date.now() - ts;
  const s = Math.round(d / 1000);
  if (s < 60) return "только что";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} мин назад`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} ч назад`;
  const days = Math.round(h / 24);
  if (days < 30) return `${days} дн назад`;
  return new Date(ts).toLocaleDateString("ru-RU", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  });
}

export function shortCid(cid: string, head = 8, tail = 6): string {
  if (cid.length <= head + tail + 1) return cid;
  return `${cid.slice(0, head)}…${cid.slice(-tail)}`;
}

const EXT_TYPE: Record<string, string> = {
  png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif",
  webp: "image/webp", svg: "image/svg+xml", pdf: "application/pdf",
  json: "application/json", md: "text/markdown", txt: "text/plain",
  csv: "text/csv", html: "text/html", js: "text/javascript", ts: "text/typescript",
  rs: "text/rust", toml: "text/toml", yaml: "text/yaml", yml: "text/yaml",
  zip: "application/zip", tar: "application/x-tar", gz: "application/gzip",
  mp4: "video/mp4", mp3: "audio/mpeg", wav: "audio/wav",
  safetensors: "application/octet-stream", bin: "application/octet-stream",
  parquet: "application/vnd.apache.parquet",
};

export function guessType(name: string, fallback = "application/octet-stream"): string {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  return EXT_TYPE[ext] ?? fallback;
}

export function isImage(t: string): boolean {
  return t.startsWith("image/");
}
export function isText(t: string): boolean {
  return (
    t.startsWith("text/") ||
    t === "application/json" ||
    t.endsWith("+json") ||
    t.endsWith("+xml")
  );
}

/** Category used to pick a file-type icon + accent. */
export function fileCategory(name: string, type: string): string {
  if (isImage(type)) return "image";
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  if (["rs", "ts", "js", "py", "go", "toml", "yaml", "yml"].includes(ext)) return "code";
  if (["json", "csv", "parquet"].includes(ext)) return "data";
  if (["safetensors", "bin", "gguf", "onnx"].includes(ext)) return "model";
  if (isText(type) || ["md", "txt"].includes(ext)) return "doc";
  if (["zip", "tar", "gz", "7z"].includes(ext)) return "archive";
  return "file";
}
