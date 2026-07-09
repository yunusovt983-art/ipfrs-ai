import type { BrowserEntry, S3Object } from "../types";
import { fileCategory, humanSize, relTime, shortCid } from "../lib/format";
import {
  IconArchive,
  IconCode,
  IconCopy,
  IconData,
  IconDoc,
  IconDownload,
  IconFile,
  IconFolder,
  IconImage,
  IconLink,
  IconModel,
  IconPin,
  IconTrash,
  IconUpload,
} from "./icons";

interface Props {
  entries: BrowserEntry[];
  view: "list" | "grid";
  selectedKey: string | null;
  selected: Set<string>;
  searching: boolean;
  onOpenFolder: (prefix: string) => void;
  onOpenObject: (key: string) => void;
  onToggle: (key: string) => void;
  onToggleAll: (keys: string[]) => void;
  onClearSelection: () => void;
  onBulkDelete: () => void;
  onBulkDownload: () => void;
  onDownload: (obj: S3Object) => void;
  onDelete: (key: string) => void;
  onCopy: (cid: string) => void;
  onShare: (obj: S3Object) => void;
  onUpload: () => void;
}

const GLYPH = {
  image: IconImage,
  code: IconCode,
  data: IconData,
  model: IconModel,
  doc: IconDoc,
  archive: IconArchive,
  file: IconFile,
} as const;

function Glyph({ name, type, size = 18 }: { name: string; type: string; size?: number }) {
  const cat = fileCategory(name, type) as keyof typeof GLYPH;
  const Cmp = GLYPH[cat] ?? IconFile;
  return (
    <span className={"glyph cat-" + cat}>
      <Cmp size={size} />
    </span>
  );
}

export function ObjectList({
  entries,
  view,
  selectedKey,
  selected,
  searching,
  onOpenFolder,
  onOpenObject,
  onToggle,
  onToggleAll,
  onClearSelection,
  onBulkDelete,
  onBulkDownload,
  onDownload,
  onDelete,
  onCopy,
  onShare,
  onUpload,
}: Props) {
  const objectKeys = entries.filter((e) => e.kind === "object").map((e) => (e as { object: S3Object }).object.key);
  const allChecked = objectKeys.length > 0 && objectKeys.every((k) => selected.has(k));
  const someChecked = objectKeys.some((k) => selected.has(k));

  const bulkBar = selected.size > 0 && (
    <div className="bulk-bar">
      <span className="bulk-count">{selected.size} выбрано</span>
      <button className="btn ghost sm" onClick={onBulkDownload}>
        <IconDownload size={15} /> Скачать
      </button>
      <button className="btn danger sm" onClick={onBulkDelete}>
        <IconTrash size={15} /> Удалить
      </button>
      <button className="btn ghost sm" onClick={onClearSelection}>
        Снять
      </button>
    </div>
  );

  if (!entries.length) {
    return (
      <div className="empty-state">
        <div className="empty-ic">
          <IconUpload size={30} />
        </div>
        <p>{searching ? "Ничего не найдено" : "Папка пуста"}</p>
        {!searching && (
          <button className="btn primary" onClick={onUpload}>
            <IconUpload size={16} /> Загрузить объекты
          </button>
        )}
        {!searching && <span className="hint">или перетащите файлы сюда</span>}
      </div>
    );
  }

  if (view === "grid") {
    return (
      <div className="list-body">
        {bulkBar}
        <div className="grid-view">
          {entries.map((e) =>
            e.kind === "folder" ? (
              <button key={"f" + e.prefix} className="grid-card folder" onClick={() => onOpenFolder(e.prefix)}>
                <span className="glyph cat-folder">
                  <IconFolder size={30} />
                </span>
                <div className="gc-name">{e.name}</div>
                <div className="gc-sub">{e.count} объектов · {humanSize(e.size)}</div>
              </button>
            ) : (
              <div
                key={e.object.key}
                className={"grid-card" + (selectedKey === e.object.key ? " sel" : "") + (selected.has(e.object.key) ? " checked" : "")}
                onClick={() => onOpenObject(e.object.key)}
              >
                <input
                  type="checkbox"
                  className="gc-check"
                  checked={selected.has(e.object.key)}
                  onClick={(ev) => ev.stopPropagation()}
                  onChange={() => onToggle(e.object.key)}
                />
                <Glyph name={e.name} type={e.object.contentType} size={30} />
                <div className="gc-name" title={e.name}>{e.name}</div>
                <div className="gc-sub">{humanSize(e.object.size)}</div>
              </div>
            ),
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="list-body">
      {bulkBar}
      <div className="table-scroll">
        <table className="objects">
          <thead>
            <tr>
              <th className="col-check">
                <input
                  type="checkbox"
                  checked={allChecked}
                  ref={(el) => {
                    if (el) el.indeterminate = !allChecked && someChecked;
                  }}
                  onChange={() => onToggleAll(objectKeys)}
                />
              </th>
              <th className="col-name">Имя</th>
              <th className="col-type">Тип</th>
              <th className="col-size">Размер</th>
              <th className="col-cid">CID</th>
              <th className="col-mod">Изменён</th>
              <th className="col-act" />
            </tr>
          </thead>
          <tbody>
            {entries.map((e) =>
              e.kind === "folder" ? (
                <tr key={"f" + e.prefix} className="folder-row" onClick={() => onOpenFolder(e.prefix)}>
                  <td className="col-check" />
                  <td className="col-name">
                    <span className="glyph cat-folder">
                      <IconFolder size={18} />
                    </span>
                    <span className="name">{e.name}/</span>
                  </td>
                  <td className="col-type muted">Папка</td>
                  <td className="col-size muted">{humanSize(e.size)}</td>
                  <td className="col-cid muted">{e.count} объектов</td>
                  <td className="col-mod muted">—</td>
                  <td className="col-act" />
                </tr>
              ) : (
                <tr
                  key={e.object.key}
                  className={"obj-row" + (selectedKey === e.object.key ? " sel" : "") + (selected.has(e.object.key) ? " checked" : "")}
                  onClick={() => onOpenObject(e.object.key)}
                >
                  <td className="col-check" onClick={(ev) => ev.stopPropagation()}>
                    <input type="checkbox" checked={selected.has(e.object.key)} onChange={() => onToggle(e.object.key)} />
                  </td>
                  <td className="col-name">
                    <Glyph name={e.name} type={e.object.contentType} />
                    <span className="name" title={e.name}>{e.name}</span>
                    {e.object.versions?.length ? (
                      <span className="ver-badge" title={`${e.object.versions.length} прежних версий`}>
                        v{e.object.versions.length + 1}
                      </span>
                    ) : null}
                    {e.object.pinned && <IconPin size={13} className="pin-badge" />}
                  </td>
                  <td className="col-type muted">{e.object.contentType.split("/").pop()}</td>
                  <td className="col-size">{humanSize(e.object.size)}</td>
                  <td className="col-cid">
                    {e.object.cid ? (
                      <button
                        className="cid-chip"
                        title={e.object.cid}
                        onClick={(ev) => {
                          ev.stopPropagation();
                          onCopy(e.object.cid);
                        }}
                      >
                        <span className="mono">{shortCid(e.object.cid)}</span>
                        <IconCopy size={13} />
                      </button>
                    ) : (
                      <span className="muted">—</span>
                    )}
                  </td>
                  <td className="col-mod muted">{relTime(e.object.lastModified)}</td>
                  <td className="col-act">
                    <button
                      className="icon-btn ghost"
                      title="Поделиться"
                      onClick={(ev) => {
                        ev.stopPropagation();
                        onShare(e.object);
                      }}
                    >
                      <IconLink size={16} />
                    </button>
                    <button
                      className="icon-btn ghost"
                      title="Скачать"
                      onClick={(ev) => {
                        ev.stopPropagation();
                        onDownload(e.object);
                      }}
                    >
                      <IconDownload size={16} />
                    </button>
                    <button
                      className="icon-btn ghost del"
                      title="Удалить"
                      onClick={(ev) => {
                        ev.stopPropagation();
                        onDelete(e.object.key);
                      }}
                    >
                      <IconTrash size={16} />
                    </button>
                  </td>
                </tr>
              ),
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
