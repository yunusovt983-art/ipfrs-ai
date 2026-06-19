---
title: Стабилизация (v0.2.1 Patch релиз)
summary: Задачи Неделя 1-2 — включить CI/CD, зафиксить баги, документация, подготовка к релизу
tags: [stabilization, release, ci-cd, testing]
---

# Стабилизация (Неделя 1-2)

> Подготовить v0.2.1 patch релиз с исправлениями багов, CI/CD и улучшениями процесса.

---

## Чеклист

### Неделя 1: Исправления & CI/CD

- [ ] **Баги #1-3 исправлены** (JWT, TLS, Backpressure)
  - [ ] Все unit тесты проходят
  - [ ] Code review завершен
  - [ ] Коммиты запушены в `fix/*` ветки

- [ ] **CI/CD включен**
  - [ ] Переместить `ipfrs_source/.github/workflows/ci.yml.disabled` → `ci.yml`
  - [ ] Тест локально: `cargo build -p ipfrs --all-features`
  - [ ] Тест локально: `cargo nextest run --all-features`
  - [ ] GitHub Actions запущен на push
  - [ ] Нет регрессий в тестовом suite

- [ ] **Стандарты документации добавлены**
  - [ ] Создать `SECURITY.md` (смотри template ниже)
  - [ ] Создать `CONTRIBUTING.md` (ссылка на этот RoadMap)
  - [ ] Создать `CODE_OF_CONDUCT.md` (стандарт Rust комьюнити)

- [ ] **GitHub настройки сконфигурированы**
  - [ ] Branch protection: требовать 1 review перед merge
  - [ ] Требовать status checks (CI/CD)
  - [ ] Allow auto-delete head branches on merge

### Неделя 2: Оставшиеся баги & Релиз

- [ ] **Баги #4-6 исправлены** (GC, FedAvg, Arrow)
  - [ ] Все unit тесты проходят
  - [ ] Бенчмарки не показывают регрессии
  - [ ] Code review завершен

- [ ] **Version bump & changelog**
  - [ ] Обновить `ipfrs_source/Cargo.toml`: `version = "0.2.1"`
  - [ ] Обновить `ipfrs_source/CHANGELOG.md`:
    ```markdown
    ## [0.2.1] - 2026-07-03 "Stability Patch"
    
    ### Исправлено
    - JWT: Заменить MD5 на HS256 HMAC (#1)
    - TLS: Реализовать реальную генерацию сертификатов, не stub (#2)
    - Backpressure: Освобождать семафор permits при уменьшении окна (#3)
    - GC: Применить параметр min_age (#4)
    - FedAvg: Исправить timeout при min_peers > 0 (#5)
    - Arrow: Оптимизировать сериализацию (убрать лишние копии) (#6)
    
    ### Добавлено
    - Политика безопасности (SECURITY.md)
    - Гайд для контрибьюторов (CONTRIBUTING.md)
    - Code of Conduct
    - GitHub Actions CI/CD включен
    ```

- [ ] **Tag & релиз**
  - [ ] Создать annotated tag: `git tag -a v0.2.1 -m "Stability patch release"`
  - [ ] Push tag: `git push origin v0.2.1`
  - [ ] GitHub Releases: Создать из tag (автоматически заполнится из CHANGELOG.md)

- [ ] **Публикация артефактов (опционально)**
  - [ ] `cargo publish` в crates.io (если upstream разрешит)
  - [ ] npm package update для `@cool-japan/ipfrs-node` (если опубликован)
  - [ ] PyPI update для `ipfrs` (если опубликован)

---

## Шаблоны файлов

### SECURITY.md

```markdown
# Политика безопасности

## Отчёт об уязвимостях

**НЕ открывайте публичный GitHub issue для уязвимостей безопасности.**

Отправьте отчёт на: **[ваш-email]** с темой `[SECURITY] IPFRS: <название уязвимости>`

Включите:
- Описание уязвимости
- Затронутые версии (e.g., 0.2.0, 0.2.1)
- Шаги воспроизведения
- Потенциальное влияние
- Предложенное исправление (опционально)

## Поддерживаемые версии

| Версия | Статус | Security Updates |
|--------|--------|------------------|
| 0.2.1 | Текущая | ✅ Да |
| 0.2.0 | Предыдущая | ✅ Да (backports) |
| 0.1.x | EOL | ❌ Нет |

## Известные ограничения & исправления

### v0.2.0 (Исправлено в v0.2.1)
- ⚠️ JWT authentication использовал MD5 вместо HS256 → **ИСПРАВЛЕНО**
- ⚠️ TLS certificate generator возвращал stub cert → **ИСПРАВЛЕНО**
- ⚠️ Backpressure window decrease не освобождал permits → **ИСПРАВЛЕНО**
- ⚠️ GC `min_age` параметр был игнорируется → **ИСПРАВЛЕНО**
- ⚠️ FedAvg всегда тайм-аутил при `min_peers > 0` → **ИСПРАВЛЕНО**

## Лучшие практики безопасности для пользователей

При запуске IPFRS в production:

1. **Всегда использовать HTTPS/TLS** (включить с rustls)
2. **Ротировать peer identities** регулярно (PeerIdentityManager)
3. **Включить encryption at rest** для чувствительных данных (ChaCha20-Poly1305 или AES-256-GCM)
4. **Ограничить peer соединения** через ConnectionManager
5. **Мониторить GossipSub mesh health** для eclipse атак
6. **Использовать circuit breaker** для failed peer соединений
7. **Включить Prometheus metrics** для observability
```

### CONTRIBUTING.md

```markdown
# Вклад в IPFRS

Спасибо за интерес к вкладу! Этот проект использует Apache 2.0 лицензию и приветствует community contributions.

## Начало работы

1. **Прочитать RoadMap:** [../RoadMap/README.md](../RoadMap/README.md)
2. **Понять архитектуру:** Смотри `Wiki_Arch_Claude/` для 15 подробных статей
3. **Настроить локальную разработку:**
   ```bash
   cd ipfrs_source
   cargo build --all-features
   cargo test --all-features
   ```

## Типы контрибуции

### Bug Fixes (Easiest)
- Выбрать из [01-Critical-Bugs.md](../RoadMap/01-Critical-Bugs.md)
- Следовать шаблону PR в каждом описании бага
- Включить unit test
- Ссылка на GitHub issue в PR

### Документация
- Улучшить статьи `Wiki_Arch_Claude/`
- Писать туториалы в `ipfrs_source/book/src/`
- Исправлять typos / примеры

### Фичи
- Проверить [04-Features.md](../RoadMap/04-Features.md) для одобренных идей
- Обсудить большие фичи в GitHub Discussions сначала
- Включить тесты & документацию

### Тестирование & Бенчмарки
- Улучшить test coverage (запусти `cargo tarpaulin`)
- Добавить бенчмарки (`cargo bench`)
- Профилировать performance с `cargo flamegraph`

## PR Процесс

1. Создать ветку: `git checkout -b fix/your-issue-name`
2. Сделать изменения + тесты + документация
3. Коммитить с ясным сообщением: `git commit -m "fix: описание"`
4. Push: `git push -u origin fix/your-issue-name`
5. Открыть PR на GitHub с:
   - Ясным заголовком
   - Ссылка на issue (`Fixes #123`)
   - Описание изменений
   - Чеклист (тест, док, lint)

### PR Чеклист

```markdown
## Описание
[Описать что вы чините/добавляете]

## Тип изменения
- [ ] Bug fix
- [ ] Новая фича
- [ ] Документация
- [ ] Улучшение производительности

## Тестирование
- [ ] Unit тесты добавлены/обновлены
- [ ] Существующие тесты проходят: `cargo test --all-features`
- [ ] Нет clippy warnings: `cargo clippy -- -D warnings`

## Документация
- [ ] Комментарии кода добавлены (если non-obvious)
- [ ] `Wiki_Arch_Claude/` статьи обновлены
- [ ] `CHANGELOG.md` запись добавлена

## Связанный Issue
Fixes #<issue-number>
```

## Стиль кода

- Следовать [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Использовать `cargo fmt` перед коммитом
- Адресовать все `cargo clippy` warnings
- Комментарии должны объяснять **почему**, не **что** (код самодокументируется)

## Заметки об архитектуре

IPFRS использует **Domain-Driven Design** с 5 bounded contexts:
- **Storage** (Sled, blocks, GC)
- **Network** (libp2p, DHT, peers)
- **Semantic** (HNSW, vector search)
- **TensorLogic** (inference engines, Datalog)
- **Transport** (Bitswap, sessions)

Смотри `Wiki_Arch_Claude/03-BoundedContexts.md` для деталей.

## Upstream контрибуции

Это форк [cool-japan/ipfrs](https://github.com/cool-japan/ipfrs). **Bug fixes и non-breaking фичи должны быть отправлены upstream!**

Смотри [05-Upstream-Contribution.md](../RoadMap/05-Upstream-Contribution.md) для деталей.

## Code Review

Мейнтейнеры будут проверять:
- [ ] Корректность & безопасность
- [ ] Test coverage
- [ ] Влияние на производительность
- [ ] Качество документации
- [ ] Backward compatibility

## Комьюнити

- **Вопросы?** Открыть GitHub Discussion
- **Нашли security issue?** Смотри `SECURITY.md`
- **Хотите обсудить идеи?** GitHub Discussions или email

---

**Спасибо за вклад в IPFRS! 🚀**
```

---

## Тестовый чеклист

### Перед релизом

```bash
# Полный test suite
cargo test --all-features --all --verbose

# Clippy (нет warnings)
cargo clippy --all-features --all-targets -- -D warnings

# Format check
cargo fmt --check

# Security audit
cargo audit

# Бенчмарки (нет регрессий)
cargo bench --all-features

# Code coverage (целевой: >80%)
cargo tarpaulin --all-features --timeout 300 --out Html

# Documentation build
cd ipfrs_source/book && mdbook build

# Try building без фич
cargo build --no-default-features
```

### CI/CD Проверка

```bash
# Симулировать GitHub Actions локально
act push  # Требует act: https://github.com/nektos/act
```

---

## Шаблон Release Notes

```markdown
# IPFRS v0.2.1 - Stability Patch

**Дата релиза:** 3 июля 2026

## Обзор

v0.2.1 — это patch релиз, который исправляет 6 критических и high-priority багов, найденные
при анализе кода, включает CI/CD, и устанавливает политики безопасности и контрибуции.

### Скачать

- **Исходный код:** [GitHub Release](https://github.com/yunusovt983-art/ipfrs-ai/releases/tag/v0.2.1)
- **Crates.io:** `cargo add ipfrs@0.2.1`
- **npm:** `npm install @cool-japan/ipfrs-node@0.2.1`

## Исправления безопасности

| Приоритет | Issue | Исправление |
|-----------|-------|------------|
| 🔴 CRITICAL | JWT использует MD5 | Теперь использует HS256 HMAC |
| 🔴 CRITICAL | TLS stub cert | Теперь генерирует реальные self-signed серты |
| 🟠 HIGH | Backpressure | Permits теперь освобождаются при уменьшении окна |
| 🟠 HIGH | GC min_age | Параметр теперь уважается |
| 🟡 MEDIUM | FedAvg timeout | Исправлена async логика сбора |
| 🟡 MEDIUM | Arrow copies | Использование памяти оптимизировано |

## Что нового

- Политика безопасности (`SECURITY.md`)
- Гайд для контрибьюторов (`CONTRIBUTING.md`)
- Code of Conduct
- GitHub Actions CI/CD включен
- Full test coverage сохранено

## Гайд обновления

Нет breaking changes. Просто обновить зависимость:

```toml
[dependencies]
ipfrs = "0.2.1"
```

## Чеклист для v0.3.0

- [ ] Release v0.2.1 stable
- [ ] Продолжить 0.3.0 разработку (Intelligence Release)
- [ ] Community feedback & issues
```

---

## Таймлайн

| Дата | Задача | Статус |
|------|--------|--------|
| День 1-2 | Зафиксить баги #1-2 | ⬜ |
| День 3-4 | Зафиксить баг #3 | ⬜ |
| День 5 | Включить CI/CD, документация | ⬜ |
| День 6-7 | Зафиксить баги #4-6 | ⬜ |
| День 8 | Релиз v0.2.1 | ⬜ |

**Смотри также:** [TIMELINE.md](TIMELINE.md) для детального неделя-за-неделей плана.
