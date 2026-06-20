---
title: Дорожная карта разработки IPFRS
summary: Стратегический план для форка yunusovt983-art/ipfrs-ai — стабилизация, комьюнити, фичи
tags: [roadmap, стратегия, timeline, milestones]
updated: 2026-06-19
---

# Дорожная карта разработки IPFRS

> Стратегический план для форка `yunusovt983-art/ipfrs-ai` на базе анализа кодовой базы Apache 2.0.

## Краткий обзор

```
📊 Статус проекта: v0.2.0 "Network Release" (Production Ready)
🎯 Цель форка: Стабилизация, документация, расширение, контриб upstream
👥 Команда: Вы + community PR'ы
⏱️  Таймлайн: 5-8 недель интенсива, потом поддержка
```

## Навигация

1. **[01-Critical-Bugs.md](01-Critical-Bugs.md)** — Зафиксить 6 критических багов (Неделя 1-2)
2. **[02-Stabilization.md](02-Stabilization.md)** — v0.2.1 релиз, CI/CD (Неделя 1-2)
3. **[03-Community-Docs.md](03-Community-Docs.md)** — Завершить книгу, туториалы, политики (Неделя 3-4)
4. **[04-Features.md](04-Features.md)** — Выбрать 1-2 фичи для v0.3.0 (Неделя 5+)
5. **[05-Upstream-Contribution.md](05-Upstream-Contribution.md)** — Как контрибьютить upstream (ongoing)
6. **[06-GeoInference.md](06-GeoInference.md)** — 🌍 Геораспределённый инференс: 6 фаз, привязка к коду
7. **[TIMELINE.md](TIMELINE.md)** — Неделя-за-неделей план

---

## Ключевые метрики

| Метрика | Целевое значение | Статус |
|---------|-----------------|--------|
| Критические баги зафиксены | 6 / 6 | ⬜ Не начато |
| v0.2.1 релиз | ✅ | ⬜ Не начато |
| CI/CD включен | ✅ | ⬜ Не начато |
| Главы книги | 8 / 8 | ⬜ In progress |
| GitHub Discussions | ✅ | ⬜ Не начато |
| Политика безопасности | ✅ SECURITY.md | ⬜ Не начато |
| Гайд для контрибьюторов | ✅ CONTRIBUTING.md | ⬜ Не начато |
| Upstream PR'ы | 3+ | ⬜ Не начато |

---

## 🚀 Быстрый старт (День 1)

```bash
cd /Volumes/Kingston/cool-japan

# 1. Создать tracking issues
gh issue create -t "Баг: JWT использует MD5 вместо HS256" -b "See RoadMap/01-Critical-Bugs.md"
gh issue create -t "Chore: Включить CI/CD pipeline" -b "See RoadMap/02-Stabilization.md"
gh issue create -t "Docs: Завершить главы mdbook" -b "See RoadMap/03-Community-Docs.md"

# 2. Начать фиксить баги (Неделя 1)
git checkout -b fix/jwt-md5-auth
# Исправить interface/src/auth.rs:449
# ... коммит ...
git push -u origin fix/jwt-md5-auth
gh pr create --title "fix: Заменить JWT MD5 на HS256" --body "Closes #1"

# 3. Отслеживать прогресс
# Обновить TIMELINE.md еженедельно
```

---

## Лицензия и авторство

- **Upstream:** [cool-japan/ipfrs](https://github.com/cool-japan/ipfrs) — Apache 2.0
- **Fork:** [yunusovt983-art/ipfrs-ai](https://github.com/yunusovt983-art/ipfrs-ai) — Apache 2.0
- **Авторы:** TensorLogic Architect (оригинал) + Community (этот форк)

**Примечание:** Все PR'ы к этому форку должны быть отправлены upstream в оригинальный проект где возможно.

---

## Лог решений

| Дата | Решение | Причина |
|------|---------|---------|
| 2026-06-19 | Начать с багов, не фич | Разблокирует v0.2.1, строит доверие комьюнити |
| 2026-06-19 | Fork под Apache 2.0 | Совместимость лицензии, включает upstream PR'ы |
| 2026-06-19 | Архивировать другие Wiki | Оставить Wiki_Arch_Claude источником истины |
| 2026-06-19 | Приоритизировать CI/CD | Предотвращает регрессии, включает safe refactoring |

---

**Последнее обновление:** 2026-06-19  
**Следующий обзор:** 2026-06-26 (чекпоинт Неделя 1)
