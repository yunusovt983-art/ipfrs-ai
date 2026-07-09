import { IconUpload } from "./icons";

interface Props {
  dragging: boolean;
  upload: { done: number; total: number; name: string } | null;
}

export function UploadOverlay({ dragging, upload }: Props) {
  if (upload) {
    const pct = Math.round((upload.done / upload.total) * 100);
    return (
      <div className="upload-toast">
        <div className="ut-head">
          <IconUpload size={16} /> Загрузка {upload.done}/{upload.total}
        </div>
        <div className="ut-name">{upload.name}</div>
        <div className="ut-bar">
          <i style={{ width: `${pct}%` }} />
        </div>
      </div>
    );
  }
  if (dragging) {
    return (
      <div className="drop-overlay">
        <div className="drop-inner">
          <IconUpload size={40} />
          <div>Отпустите файлы для загрузки</div>
        </div>
      </div>
    );
  }
  return null;
}
