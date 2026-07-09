import type { Toast } from "../types";
import { IconCheck, IconClose } from "./icons";

export function Toasts({ toasts }: { toasts: Toast[] }) {
  return (
    <div className="toasts">
      {toasts.map((t) => (
        <div key={t.id} className={"toast " + t.kind}>
          <span className="toast-ic">
            {t.kind === "success" ? <IconCheck size={16} /> : t.kind === "error" ? <IconClose size={16} /> : "•"}
          </span>
          <span>{t.message}</span>
        </div>
      ))}
    </div>
  );
}
