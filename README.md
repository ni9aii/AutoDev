# AutoDev Pipeline

Автоматический пайплайн разработки: ревью → план → исполнение → CI → релиз.

## Архитектура

**Hermes Agent (оркестратор) + Claude Code (исполнитель)**

- Hermes: планирование, агрегация, принятие решений, простые патчи
- Claude Code: все задачи кодирования — фиксы, рефакторинг, тесты, CI debug

## Возможности

- **4 параллельных ревьюера**: Code, Security, Architecture, DevOps
- **Агрегация находок**: классификация Do Now / Defer
- **Автоматическое исполнение**: простые фиксы через Hermes, сложные через Claude Code
- **CI интеграция**: проверка GitHub Actions статуса
- **Релиз**: создание git тега и GitHub Release

## Установка

```bash
# Клонирование
git clone https://github.com/ni9aii/AutoDev.git
cd AutoDev

# Сборка
cargo build --release

# Установка бинарников в PATH
cargo install --path .
```

## Требования

- Rust 1.70+
- Claude Code CLI (`npm install -g @anthropic-ai/claude-code`)
- GitHub PAT (для CI проверки и релизов)

## Использование

### Полный пайплайн

```bash
run-pipeline /path/to/project full
```

### Только ревью

```bash
run-pipeline /path/to/project review
```

### Ревью + планирование

```bash
run-pipeline /path/to/project plan
```

### Релиз

```bash
run-pipeline /path/to/project release --version v0.2.0
```

## Переменные окружения

- `GITHUB_TOKEN` или `GITHUB_PAT` — для GitHub API
- `AUTO_DEV_VERSION` — версия для релиза (fallback)

## Структура проекта

```
.
├── src/
│   ├── lib.rs              # Общие модули (log, git, markdown, test_runner)
│   └── bin/
│       ├── run_pipeline.rs # Основной пайплайн
│       ├── ci_check.rs     # Проверка CI статуса
│       └── review_aggregator.rs # Агрегация ревью
├── .github/workflows/
│   └── ci.yml              # CI конфигурация
├── Cargo.toml
└── README.md
```

## Лицензия

MIT License — см. [LICENSE](LICENSE)
