# IPFRS S3 Console — Implementation Plan (живой документ)

> **Обновляется после каждой сессии.** Отражает траекторию движения: что сделано, что в очереди.  
> Последнее обновление: **2026-07-09**

---

## Актуальный статус 7 ключевых пунктов

| Пункт | Статус | Осталось |
|-------|--------|----------|
| **DAG-эксплорер** | ✅ **готово** — рекурсивное дерево (sub-CID, размеры, кодеки) + отдельный «Блок» (hex-dump) | — |
| **Pin / Unpin** | ✅ **готово** — кнопка-тоггл в строке таблицы и grid-карточке; demo меняет флаг, live дёргает `/api/v0/pin/add\|rm` | — |
| **Прогресс мульти-загрузки** | ✅ **готово** — per-file progress bar + общая полоска; demo симуляция; отмена всей очереди | Ретрай отдельного файла; live XHR `onprogress` |
| **Хлебные крошки** | ✅ **готово** — dropdown соседних папок с анимацией, click-outside закрытие, «назад/вперёд» история | — |
| **Bucket-политики** | ✅ **готово** — versioning/autopin/квота на бакет; прогресс-бар использования; МБ/ГБ переключатель; кнопка «Сбросить» | Hard quota (блокировать загрузку); |
| **Семантический поиск** | 🟡 умный (лексика + содержимое файлов) | Вектор-поиск через `/semantic/search` — нужна embedding-модель |
| **Datasets-превью** | 🟡 JSON/CSV первые строки; Parquet показывает заглушку | Колоночный разбор `.parquet` (parquet-wasm ~сотни КБ бандла) |

---

## Хронология сессий

### Сессия 1 — Базовый каркас UI
- Создан Vite + React + TypeScript проект в `/ui`
- Файловая структура: `App.tsx`, `types.ts`, `styles.css`
- Реализованы базовые компоненты: `Sidebar`, `Toolbar`, `ObjectList`, `DetailsDrawer`
- Демо-сид: три бакета (`ml-models`, `datasets`, `site-assets`) с объектами

### Сессия 2 — IPFS-специфика и модальные окна
- Добавлены: `BlockInspector`, `DagExplorer`, `ProvenanceModal`, `ProvidersModal`
- `lib/ipfrs.ts` — клиент к IPFS gateway (info, add, pin/unpin, cat, dag/get, findprovs)
- `lib/buckets.ts` — localStorage persistence + `deriveEntries` (виртуальные папки)
- `lib/inspect.ts` — hex-dump, detectFormat, scanCidLinks
- `lib/search.ts` — smartSearch по содержимому объектов
- `lib/preview.ts` — PreviewPane (изображения, текст, JSON, Parquet, PDF)
- Drag-and-drop загрузка + UploadOverlay

### Сессия 3 — Пачка партиалов (2026-07-09)

#### ✅ Pin / Unpin в ObjectList
- Кнопка `IconPin` в **строке таблицы** (hover → opacity 1, accent цвет при pinned)
- Кнопка `IconPin` в **grid-карточке** (абсолютный оверлей right-top, всегда видна если pinned)
- Проп `onPin` привязан к `togglePin` в `App.tsx` (live: реальный API, demo: флаг)

#### ✅ Прогресс по файлам в UploadOverlay
- `progress?: number` (0–100) в `UploadItem` (`types.ts`)
- Общая полоска (gradient) под хедером панели — агрегированный прогресс
- Per-file bar: синий `accent2` во время загрузки → зелёный `accent` при завершении
- Demo: анимированная симуляция `[10→35→65→88→100]` за ~350 мс

#### ✅ Breadcrumb Dropdown — фикс UX
- Закрытие click-outside: `useEffect` + `document.addEventListener("mousedown")` + cleanup
- Класс `.open` на caret-кнопке пока dropdown открыт
- `z-index: 200`, `max-height: 240px`, overflow-y scroll
- `@keyframes crumbFade` — fade + slide 6 px, 120 ms
- `role="menu"` / `role="menuitem"` для a11y

#### ✅ BucketPolicyModal — расширение
- Прогресс-бар использования квоты (зелёный < 80%, жёлтый ≥ 80%)
- Переключатель единиц **МБ / ГБ** с корректным пересчётом
- Кнопка **«Сбросить»** → defaults
- Inline-предупреждение (amber) при отключении versioning если оно было включено
- Принимает `usedBytes` из `stats.size` из `App.tsx`

#### ✅ Demo seed — полное покрытие всех объектов
- `attachDemoIpfrs` расширена до всех **13 объектов** в 3 бакетах
- Каждый объект получает `dag` + `proof` + `providers`
- `SEED_VERSION = "v4"` — автоматический сброс устаревшего localStorage при смене версии
- Особые случаи: `README.md` и `index.html` — `proof.verified = false` (демо «нет подписи»)

---

## Архитектура UI (текущее состояние)

```
App.tsx
├── Sidebar           — бакеты, статус коннекта, настройки
├── Toolbar           — хлебные крошки + dropdown, поиск, upload, view-toggle
├── ObjectList        — список/сетка объектов, bulk-actions, Pin/Unpin
├── SmartResults      — лексически-ранжированные результаты поиска
├── DetailsDrawer     — метаданные, версии, preview, действия
├── BlockInspector    — hex-dump + CID-ссылки
├── DagExplorer       — интерактивное рекурсивное DAG-дерево
├── ProvenanceModal   — TensorLogic proof-carrying дерево
├── ProvidersModal    — карта пиров с RTT-барами
├── BucketPolicyModal — versioning, autopin, квота + usage bar
├── SettingsModal     — режим demo/live, gateway URL
├── UploadOverlay     — прогресс загрузки (per-file + overall bar)
└── Toasts            — уведомления
```

**Хранилище**  
- `localStorage` — манифесты бакетов, объекты, политики, настройки, тема  
- `blobCache: Map<cid, Blob>` — in-memory для drag-and-drop объектов

**Режимы**

| Режим | Данные | CID | Pin |
|-------|--------|-----|-----|
| demo | localStorage seed | SHA2-256 псевдо-CID | флаг-симуляция |
| live | IPFS gateway | настоящий CID | `/api/v0/pin/add\|rm` |

---

## Очередь следующих задач

### 🔴 Высокий приоритет
- [ ] **DetailsDrawer** — вкладки (Метаданные / Версии / Preview) вместо длинного скролла
- [ ] **Sidebar** — сортировка бакетов (по имени / по дате / по размеру)
- [ ] **Upload** — ретрай отдельного файла из UploadOverlay
- [ ] **Upload live** — реальный XHR progress через `XMLHttpRequest.upload.onprogress`

### 🟡 Средний приоритет
- [ ] **Поиск** — highlight совпадений в SmartResults
- [ ] **ObjectList** — сортировка столбцов (клик по заголовку ↑↓)
- [ ] **ObjectList** — rename объекта (inline edit F2)
- [ ] **BucketPolicyModal** — hard quota (блокировать загрузку при превышении, не только предупреждение)
- [ ] **Parquet preview** — минимум: показать «N row groups, M columns» из footer; полный разбор требует parquet-wasm

### 🟢 Низкий приоритет / упирается в бэкенд
- [ ] **Вектор-семантика** — on-device embedder (char-ngram TF-IDF или `/semantic/search`); польза только в live
- [ ] **Keyboard shortcuts** — `⌘K` поиск, `Del` удалить, `Space` preview
- [ ] **Export** — скачать manifest бакета как JSON/CSV
- [ ] **Drag между бакетами** — переместить объект
- [ ] **Multi-window** — BroadcastChannel синхронизация вкладок

---

## Технический долг

| Проблема | Файл | Приоритет |
|----------|------|-----------|
| `eslint-disable` в `useEffect` (deps) | `App.tsx:108` | low |
| `DetailsDrawer` растёт (600+ строк) → разбить на вкладки | `DetailsDrawer.tsx` | medium |
| `blobCache` не очищается → утечка памяти при большом upload | `buckets.ts` | medium |
| Нет E2E / unit тестов | — | low |

---

## Конфигурация проекта

**Stack**: React 18 · TypeScript · Vite · Vanilla CSS (no Tailwind)  
**Build**: `npm run build` → `dist/` (216 KB JS gzip: 68 KB)  
**Seed version**: `v4` (auto-migration при изменении)

```
ui/
├── src/
│   ├── App.tsx            (~760 строк)
│   ├── types.ts
│   ├── styles.css
│   ├── components/        (15 компонентов)
│   └── lib/               (buckets, ipfrs, format, inspect, preview, search)
├── vite.config.ts
├── tsconfig.json
└── package.json
```
