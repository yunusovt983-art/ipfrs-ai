import type { S3Object } from "../types";
import type { HighlightRange, Ranked } from "../lib/search";
import { fileCategory, humanSize, relTime } from "../lib/format";
import {
  IconArchive,
  IconCode,
  IconData,
  IconDoc,
  IconDownload,
  IconFile,
  IconImage,
  IconModel,
  IconSearch,
} from "./icons";

const GLYPH = {
  image: IconImage,
  code: IconCode,
  data: IconData,
  model: IconModel,
  doc: IconDoc,
  archive: IconArchive,
  file: IconFile,
} as const;

function Glyph({ name, type }: { name: string; type: string }) {
  const cat = fileCategory(name, type) as keyof typeof GLYPH;
  const Cmp = GLYPH[cat] ?? IconFile;
  return (
    <span className={"glyph cat-" + cat}>
      <Cmp size={18} />
    </span>
  );
}

/** Render a string with highlighted ranges. */
function Highlighted({ text, ranges }: { text: string; ranges?: HighlightRange[] }) {
  if (!ranges || !ranges.length) return <>{text}</>;
  const parts: React.ReactNode[] = [];
  let cursor = 0;
  for (const r of ranges) {
    if (r.start > cursor) parts.push(<span key={cursor}>{text.slice(cursor, r.start)}</span>);
    parts.push(
      <mark key={r.start} className="hl">
        {text.slice(r.start, r.end)}
      </mark>,
    );
    cursor = r.end;
  }
  if (cursor < text.length) parts.push(<span key={cursor}>{text.slice(cursor)}</span>);
  return <>{parts}</>;
}

interface Props {
  query: string;
  results: Ranked[];
  onOpen: (key: string) => void;
  onDownload: (o: S3Object) => void;
}

export function SmartResults({ query, results, onOpen, onDownload }: Props) {
  if (!query.trim()) {
    return (
      <div className="empty-state">
        <div className="empty-ic">
          <IconSearch size={30} />
        </div>
        <p>Умный поиск по содержимому и метаданным</p>
        <span className="hint">
          Ранжирует объекты по релевантности; для текстовых файлов ищет по содержимому.
        </span>
      </div>
    );
  }
  if (!results.length) {
    return (
      <div className="empty-state">
        <p>По запросу «{query}» ничего не найдено</p>
      </div>
    );
  }
  return (
    <div className="smart-results">
      <div className="smart-head">
        <span>{results.length} результатов</span>
        <span className="smart-head-hint">· ранжировано по релевантности (лексика + n-gram)</span>
      </div>
      {results.map((r) => (
        <div key={r.object.key} className="smart-row" onClick={() => onOpen(r.object.key)}>
          <Glyph name={r.object.key} type={r.object.contentType} />
          <div className="sr-main">
            <div className="sr-name">
              <span className="sr-key" title={r.object.key}>
                <Highlighted text={r.object.key} ranges={r.highlights} />
              </span>
              <span className={"sr-where" + (r.where === "содержимое" ? " content" : "")}>
                {r.where}
              </span>
            </div>
            {r.snippet && (
              <div className="sr-snip" title={r.snippet}>
                <Highlighted text={r.snippet} ranges={r.snippetHighlight} />
              </div>
            )}
            <div className="sr-meta">
              {humanSize(r.object.size)} · {relTime(r.object.lastModified)}
            </div>
          </div>
          <div className="sr-score">
            <div className="sr-bar">
              <i style={{ width: `${Math.round(r.score * 100)}%` }} />
            </div>
            <span>{Math.round(r.score * 100)}</span>
          </div>
          <button
            className="icon-btn ghost"
            title="Скачать"
            onClick={(e) => {
              e.stopPropagation();
              onDownload(r.object);
            }}
          >
            <IconDownload size={16} />
          </button>
        </div>
      ))}
    </div>
  );
}
