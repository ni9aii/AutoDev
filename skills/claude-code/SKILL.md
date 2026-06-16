# AutoDev — Claude Code Skill

Automated development pipeline: parallel review → aggregate → execute → verify.

## Invocation

```
/autodev [review|plan|execute|full] <project-name> [project-path]
```

- `project-name` — имя проекта, совпадает с директорией в `dev-notes`
- `project-path` — путь к репо (по умолчанию: текущая директория)

## Environment

```bash
DEV_NOTES_ROOT=~/obsidian-vault/dev-notes   # путь к dev-notes
```

Бинари (`review-aggregator`, `ci-check`) читают `DEV_NOTES_ROOT` автоматически.
Убедись, что они собраны и на `$PATH`:

```bash
cd ~/code/AutoDev && cargo build --release
export PATH="$PATH:$HOME/code/AutoDev/target/release"
```

## Phases

### `review` — параллельные ревьюеры

Запускает 4 субагента параллельно через Workflow tool. Каждый читает
исходники и сохраняет отчёт в:

```
$DEV_NOTES_ROOT/<project>/reviews/<YYYY-MM-DD>-<role>-review-report.md
```

Роли: `code`, `security`, `architecture`, `devops`.

**Промпт для каждого субагента:**

```
Ты <role> Reviewer. Твоя задача — проверить проект по пути <project-path>.

Прочитай все исходные файлы. Составь отчёт в формате:

### [CRITICAL] Заголовок находки
Описание проблемы.
File: `path/to/file.rs`
Line: 42

### [IMPORTANT] Заголовок
...

### [MINOR] Заголовок
...

Сохрани отчёт в: <output-path>
```

### `plan` — агрегация

После ревью запускает `review-aggregator`:

```bash
review-aggregator \
  --dev-notes \
  --project <project-name> \
  --dev-notes-root $DEV_NOTES_ROOT
```

Результат — план в `$DEV_NOTES_ROOT/<project>/plans/<timestamp>-plan.md`
с секциями "Do Now" и "Defer".

### `execute` — исполнение фиксов

Читает последний план из `$DEV_NOTES_ROOT/<project>/plans/`.
Для каждого фикса из секции "Do Now":

- **Простые** (1-2 файла, ≤20 строк) — напрямую через Read/Edit
- **Сложные** — через субагент (`Agent` tool)

### `full` — полный пайплайн

`review` → `plan` → `execute` → `verify`

### `verify` — проверка

```bash
ci-check <project-path> --dev-notes --project <project-name> --dev-notes-root $DEV_NOTES_ROOT
```

## Execution instructions

При вызове `/autodev`:

1. Установи `DEV_NOTES_ROOT` (спроси у пользователя если неизвестно)
2. Определи `project-path` (текущая директория если не указана)
3. Выполни нужную фазу согласно разделам выше
4. Для фазы `review` — используй **Workflow tool** с 4 параллельными субагентами
5. Для фазы `execute` — читай план и применяй фиксы итеративно, коммитя после каждого

### Git и CI — обязательные правила

- **После каждого батча коммитов — сразу делай `git push`**. Не накапливай неотправленные коммиты.
- После пуша — проверяй статус CI: `gh run list --limit 5 --repo <owner>/<repo>`.
- Если CI упал — разбери причину до следующего батча.
- В коммитах **не добавлять** строку `Co-Authored-By: Claude ...`.

### Шаблон Workflow для review-фазы

```javascript
export const meta = {
  name: 'autodev-review',
  description: 'Parallel code review: 4 reviewers → dev-notes',
  phases: [{ title: 'Review' }],
}

const ROLES = ['code', 'security', 'architecture', 'devops']
const date = args.date   // передаётся из вызывающего контекста
const projectPath = args.projectPath
const projectName = args.projectName
const devNotesRoot = args.devNotesRoot

await parallel(ROLES.map(role => () => agent(
  `Ты ${role} reviewer. Прочитай проект по пути ${projectPath}.
  Составь отчёт с находками по формату:
  ### [CRITICAL/IMPORTANT/MINOR] Заголовок
  Описание. File: \`path\`. Line: N.
  Сохрани в: ${devNotesRoot}/${projectName}/reviews/${date}-${role}-review-report.md`,
  { label: `review:${role}`, phase: 'Review' }
)))
```
