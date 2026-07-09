import { useState } from "react";
import type { GatewayInfo, Settings } from "../types";
import { IpfrsClient } from "../lib/ipfrs";
import { IconCheck, IconClose } from "./icons";

interface Props {
  settings: Settings;
  onApply: (s: Settings) => void;
  onClose: () => void;
}

type TestState =
  | { status: "idle" }
  | { status: "testing" }
  | { status: "ok"; info: GatewayInfo }
  | { status: "fail"; message: string };

export function SettingsModal({ settings, onApply, onClose }: Props) {
  const [local, setLocal] = useState<Settings>(settings);
  const [test, setTest] = useState<TestState>({ status: "idle" });

  const runTest = async () => {
    setTest({ status: "testing" });
    try {
      const info = await new IpfrsClient(local.gateway).info();
      setTest({ status: "ok", info });
    } catch (e) {
      setTest({ status: "fail", message: (e as Error).message });
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>Настройки подключения</h3>
          <button className="icon-btn" onClick={onClose}>
            <IconClose size={18} />
          </button>
        </div>

        <div className="modal-body">
          <label className="field-label">Режим хранилища</label>
          <div className="seg">
            <button
              className={local.mode === "demo" ? "active" : ""}
              onClick={() => setLocal({ ...local, mode: "demo" })}
            >
              Демо (локальный манифест)
            </button>
            <button
              className={local.mode === "live" ? "active" : ""}
              onClick={() => setLocal({ ...local, mode: "live" })}
            >
              Live · IPFRS gateway
            </button>
          </div>
          <p className="field-hint">
            {local.mode === "demo"
              ? "Объекты и CID хранятся в браузере (localStorage). Загрузки получают детерминированный псевдо-CID (SHA-256) и работают офлайн."
              : "Загрузка идёт через POST /api/v0/add на шлюз IPFRS; CID — настоящий. Требуется запущенный gateway и разрешённый CORS."}
          </p>

          <label className="field-label">Адрес шлюза</label>
          <div className="gw-row">
            <input
              className="gw-input"
              value={local.gateway}
              disabled={local.mode !== "live"}
              onChange={(e) => setLocal({ ...local, gateway: e.target.value })}
              placeholder="http://127.0.0.1:8080"
            />
            <button
              className="btn ghost"
              disabled={local.mode !== "live" || test.status === "testing"}
              onClick={runTest}
            >
              {test.status === "testing" ? "Проверка…" : "Проверить"}
            </button>
          </div>

          {test.status === "ok" && (
            <div className="test-result ok">
              <IconCheck size={16} /> Подключено · v{test.info.version ?? "?"}
              {test.info.peers != null ? ` · пиров: ${test.info.peers}` : ""}
            </div>
          )}
          {test.status === "fail" && (
            <div className="test-result fail">Не удалось подключиться: {test.message}</div>
          )}

          <label className="check">
            <input
              type="checkbox"
              checked={local.pinOnUpload}
              disabled={local.mode !== "live"}
              onChange={(e) => setLocal({ ...local, pinOnUpload: e.target.checked })}
            />
            <span>Закреплять (pin) объекты при загрузке</span>
          </label>
        </div>

        <div className="modal-foot">
          <button className="btn ghost" onClick={onClose}>
            Отмена
          </button>
          <button className="btn primary" onClick={() => onApply(local)}>
            Сохранить
          </button>
        </div>
      </div>
    </div>
  );
}
