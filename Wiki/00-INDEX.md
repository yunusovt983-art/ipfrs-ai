---
title: 00-INDEX
type: navigation
summary: Полный каталог Wiki «Как функционирует IPFRS» с однострочными резюме и привязкой к крейтам
tags: [ipfrs, ddd, index, navigation]
updated: 2026-06-19
---

# INDEX — Каталог Wiki «Как функционирует IPFRS»

> Каждая страница привязана к крейту-источнику и снабжена однострочным резюме.
> Порядок чтения = порядок номеров. Реальность vs модель сведена в [[11-RealityCheck]].

---

## Навигация и основы

| Стр. | Резюме | Источник |
|------|--------|----------|
| [[README]] | Точка входа, навигация по ролям | — |
| [[00-INDEX]] | Этот каталог | — |
| [[01-DomainOverview]] | Что такое IPFRS, ядро домена, единый язык, ключевые инварианты | весь workspace |
| [[02-StrategicDesign]] | 7 bounded contexts, карта контекстов, отношения (ACL/Shared Kernel/Conformist) | `crates/*` |

## Фундамент

| Стр. | Резюме | Источник |
|------|--------|----------|
| [[03-SharedKernel]] | `Cid` как универсальный токен границы; `Block`, `Ipld`, DAG, хеши, кодеки | `ipfrs-core` |

## Bounded Contexts (домены)

| Стр. | Резюме | Источник |
|------|--------|----------|
| [[04-StorageContext]] | Репозиторий `BlockStore`, декораторы (`Bloom→Cache→Sled`), GC, пины, тиринг | `ipfrs-storage` |
| [[05-NetworkContext]] | `NetworkNode` + libp2p swarm, Kademlia DHT, репутация; Tier A vs Tier B | `ipfrs-network` |
| [[06-SemanticContext]] | `VectorIndex` (HNSW), `DiskANNIndex` (Vamana), `SemanticRouter`, семантический DHT | `ipfrs-semantic` |
| [[07-TensorLogicContext]] | Нейро-символика: 20+ движков вывода + автоград + FedAvg, CID-адресуемые правила | `ipfrs-tensorlogic` |
| [[08-TransportContext]] | Агрегат `Session`, Bitswap, TensorSwap, GraphSync, приоритетные want-list | `ipfrs-transport` |

## Прикладной слой и потоки

| Стр. | Резюме | Источник |
|------|--------|----------|
| [[09-ApplicationLayer]] | `Node` как Facade над 5 контекстами; шлюз (gRPC/GraphQL/WS/HTTP), Auth, TLS | `ipfrs`, `ipfrs-interface` |
| [[10-DataFlows]] | Сквозные сценарии: ADD, GET (cache→DHT→backfill), SEARCH, INFER, FedAvg | все контексты |
| [[11-RealityCheck]] | Что реально работает, что заглушка (⚠️); расхождения старых вики с кодом | все контексты |

---

## Карта зависимостей контекстов (упрощённо)

```
                        ┌──────────────────────────┐
                        │   ipfrs-interface        │  ← внешние API (gRPC/GraphQL/WS/HTTP)
                        │   (Open Host Service)    │
                        └────────────┬─────────────┘
                                     │
                        ┌────────────▼───────────────┐
                        │       ipfrs (Node)         │  ← Application Facade
                        │   оркестрация контекстов   │
                        └─┬───────┬────────┬───────┬─┘
              ┌───────────┘       │        │       └────────────┐
     ┌────────▼───────┐ ┌─────────▼─ ─┐ ┌──▼───────────┐ ┌──────▼──────────┐
     │ ipfrs-storage  │ │ipfrs-network│ │ipfrs-semantic│ │ipfrs-tensorlogic│
     │  (Storage)     │ │ (Network)   │ │  (Semantic)  │ │  (TensorLogic)  │
     └────────┬───────┘ └──────────┬──┘ └──┬───────────┘ └──────┬──────────┘
              │                    │       │                    │
              │            ┌───────▼───────▼────┐               │
              │            │  ipfrs-transport   │               │
              │            │   (block exchange) │               │
              │            └─────────┬──────────┘               │
              └────────────┬─────────┴────────────┬─────────────┘
                           │                      │
                  ┌────────▼──────────────────────▼──────────┐
                  │          ipfrs-core (Shared Kernel)      │
                  │   Cid · Block · Ipld · DAG · hash/codec  │
                  └──────────────────────────────────────────┘
```

Все стрелки сходятся вниз к `ipfrs-core`: **CID — единственный тип, на котором
сходятся все контексты**. Подробнее — [[02-StrategicDesign]], [[03-SharedKernel]].

---

**Связанные**: [[README]] | [[01-DomainOverview]] | [[02-StrategicDesign]]
