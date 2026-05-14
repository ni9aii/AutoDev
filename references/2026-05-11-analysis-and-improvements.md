# Auto-Dev Pipeline — Анализ и предложения по доработке

## Дата: 2026-05-11

## Текущие пробелы скилла

### 1. Нет фазы Release
Скилл заканчивается на Verify (CI check). Нет этапа:
- Создание тега (`git tag -a vX.Y.Z`)
- GitHub Release с описанием из CHANGELOG
- Прикрепление артефактов (firmware binary)
- Обновление статуса в Obsidian

### 2. Нет Do/Defer приоритизации
Агрегатор собирает все findings в один план. Нет разделения:
- **Do now**: low complexity, high value, no dependencies
- **Defer**: architectural changes, cross-module refactoring
- Показывать оба списка пользователю для подтверждения

### 3. Нет стандартного security checklist
Security reviewer каждый раз заново проверяет одно и то же.
Для ESP-IDF проектов должен быть чеклист:
- [ ] NVS encryption enabled
- [ ] HTTP API authentication
- [ ] WDT panic enabled in production
- [ ] No hardcoded credentials
- [ ] Buffer overflow protection
- [ ] Content-Type validation

### 4. Нет Phase-based workflow
Скилл работает как "one-shot". Нет концепции фаз с acceptance criteria.

### 5. CI check не работает без PAT
Нужен fallback через `gh run list` или GitHub API с rate limit handling.

### 6. Нет универсального конфигурирования для нескольких проектов

### 7. Нет интеграции с Obsidian (автообновление статуса)

### 8. Нет единого формата отчётов ревьюеров

## Приоритет доработок

| # | Доработка | Effort | Impact |
|---|-----------|--------|--------|
| 1 | Release Manager фаза | Medium | High |
| 2 | Do/Defer приоритизация | Low | High |
| 3 | Report format standardization | Low | High |
| 4 | Phase-based workflow | Medium | High |
| 5 | Security checklist | Low | Medium |
| 6 | CI check fallback | Low | Medium |
| 7 | Multi-project config | Medium | Medium |
| 8 | Obsidian integration | Low | Medium |
