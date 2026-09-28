# Implementation Plan: Sorter — раскладка видео по папкам

**Branch**: `001-video-sorter` | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-video-sorter/spec.md`

## Summary

Portable Tauri-приложение: колода видео раскладывается по стопкам-папкам клавишами, пачкой со стола или
перетаскиванием. Rust отвечает за всё, что касается файлов и тяжёлой работы: журнал ходов в SQLite до
перемещения, безопасный перенос (включая между дисками и в корзину Windows), проявку через Media Foundation
(кадры, сведения), эмбеддинги DINOv2 через ONNX Runtime + DirectML, слежение за папками. React рисует колоду,
стол и экраны по `.planning/DESIGN.md`. Видео — `<video>` через свой протокол `media://` с Range, с «плёночным»
запасным режимом для форматов, которые WebView2 не играет. Детали решений — [research.md](research.md).

## Technical Context

**Language/Version**: Rust (stable, edition 2021), TypeScript 5.8

**Primary Dependencies**: Tauri 2 (+ plugin-dialog, plugin-opener, plugin-window-state), React 19, Vite 7,
Tailwind 4; Rust — `rusqlite` 0.32 (bundled), `windows` 0.62 (Media Foundation, файловые API), `image` (JPEG),
`ort` 2.0.0-rc.13 (`load-dynamic`, `directml`), `trash` 5.2, `notify` 8.2 + `notify-debouncer-full`, `serde`

**Storage**: SQLite `%APPDATA%\com.stillmvd.sorter\sorter.db` (WAL, `synchronous=FULL`); кэш кадров JPEG и
распакованный runtime (onnxruntime.dll, DirectML.dll, модель) там же

**Testing**: `cargo test` — модуль ходов/журнала/восстановления на временных папках (в т.ч. сбой на каждом
шаге), подбор имени, клавиши, kNN; ручная приёмка по [quickstart.md](quickstart.md) на копии `G:\vk videos`

**Target Platform**: Windows 11 x64, WebView2

**Project Type**: desktop-app (Tauri: `src/` + `src-tauri/`)

**Performance Goals**: смена карты ≤ 0,3 с (p95) после хода; первые 20 карт проявлены ≤ 15 с; вектор карты
≤ 100 мс на GPU; интерфейс 60 fps при повороте карты и прокрутке стола

**Constraints**: офлайн, ни одного сетевого запроса; один exe; ни одного потерянного файла (принцип I);
кэш и данные только в `%APPDATA%`

**Scale/Scope**: колода до ~5 000 видео, 10–30 стопок, журнал без ограничения; 7 экранов из холста

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Принцип | Как соблюдён | Статус |
|---------|--------------|--------|
| I. Файлы в безопасности | журнал `Move/MoveItem` пишется транзакцией до операций; шаги `planned/copied/done`; восстановление при запуске; перенос между дисками через временное имя и сверку; корзина только через `trash`, отмена через restore; суффикс вместо перезаписи; тесты Rust до подключения UI | ✅ |
| II. Одно решение — одно нажатие | [keyboard.md](contracts/keyboard.md): клавиша/Enter/Delete; проявка фоновая с приоритетом ближайших карт; три `<video>` с предзагрузкой (R8) | ✅ |
| III. Portable, offline, приватно | runtime и модель вшиты в exe и распаковываются в AppData; протокол `media://` вместо сокета; вычисления локально; в папках пользователя кроме временного `.sorter-part` при переносе ничего не создаётся | ✅ (см. Complexity) |
| IV. Дизайн семейства | токены/экраны из `.planning/DESIGN.md`; «Корзина» и перетаскивание — уточнить на холсте до вёрстки | ✅ |
| V. Нативно и понятно | системный диалог папки, Ctrl+Z, тексты на «ты», ошибки с причиной и шагом (коды в [ipc.md](contracts/ipc.md)) | ✅ |
| VI. Простота кода | без state-менеджеров, без ffmpeg, свой перенос вместо `fs_extra`, каркас Demo Prep и заголовок Echo Studio | ✅ |

Повторная проверка после Phase 1: модель данных и контракты не добавили нарушений.

## Project Structure

### Documentation (this feature)

```text
specs/001-video-sorter/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── ipc.md
│   └── keyboard.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
src-tauri/
├── Cargo.toml
├── tauri.conf.json          # decorations: false, id com.stillmvd.sorter, протокол media://
├── resources/               # onnxruntime.dll, DirectML.dll, dinov2-small.onnx (вшиваются include_bytes!)
└── src/
    ├── main.rs / lib.rs     # запуск, регистрация команд и протокола
    ├── db.rs                # схема, миграции, запросы
    ├── moves.rs             # ход, отмена, восстановление, перенос между дисками, корзина, подбор имени
    ├── deck.rs              # колода: скан, порядок, defer, слежение
    ├── piles.rs             # стопки: скан стола, клавиши, создать/переименовать, слежение
    ├── develop.rs           # очередь проявки, Media Foundation: кадры и сведения
    ├── hints.rs             # runtime ORT, эмбеддинги, kNN, примеры
    ├── media.rs             # протокол media:// с Range
    └── commands.rs          # команды из contracts/ipc.md

src/
├── main.tsx, App.tsx        # оболочка, маршрутизация по состоянию (первый запуск / проявка / колода / стол / пусто)
├── tokens.css, styles.css   # из DESIGN.md, глобальный --scroll-inset
├── fonts/                   # Kockers Sans 300/400/500/700 woff2
├── lib/ipc.ts               # типизированные обёртки команд и событий
├── lib/keys.ts              # раскладка клавиш, KeyboardEvent.code
└── components/
    ├── Titlebar.tsx, WindowControls.tsx   # из Echo Studio
    ├── ui/                  # Button, Segment, SearchField, Toggle, Kbd
    ├── deck/                # Card (ориентация, поворот, 3 video), FilmStrip, LastMove, HintBox, PilesRow, Pile
    ├── table/               # ContactSheet, StripRow
    ├── screens/             # Start, Develop, Piles, Journal, Done, Settings
    └── dnd.ts               # перетаскивание на стопку
```

**Structure Decision**: одна Tauri-программа, как Demo Prep. Логика файлов целиком в `moves.rs` без UI-зависимостей,
чтобы тестироваться `cargo test` на временных папках.

## Порядок работ (для /speckit-tasks)

1. Каркас: Tauri + React по Demo Prep, токены, шрифты, заголовок окна, SQLite-схема.
2. **Ядро ходов** (`moves.rs`) с тестами: перенос, суффикс, между дисками, корзина, отмена, восстановление после сбоя.
3. Колода P1: первый запуск, колода/стол, карта с `<video>` через `media://`, клавиши, поиск, Tab, Ctrl+Z,
   «Последний ход», ряд стопок, перетаскивание.
4. Замер форматов тестовой колоды (доля HEVC) → «плёночный» режим.
5. Проявка P2: Media Foundation, кэш, экран проявки, плёнка под картой, слежение за колодой.
6. Стол P3: контактный лист, отметки, пачка, новая стопка Shift+Enter.
7. Подсказки P4: runtime ORT, DINOv2, примеры (включая холодный старт по готовым стопкам), блок подсказки.
8. Стопки P5, журнал и итоги P6, настройки.
9. Холст: «Корзина», перетаскивание, настройки — до вёрстки соответствующих частей.
10. Релиз: `pnpm tauri build --no-bundle`, проверка по quickstart.md.

## Complexity Tracking

| Отступление | Зачем | Почему проще не подошло |
|-------------|-------|-------------------------|
| Временный файл `.<имя>.sorter-part` в папке стопки при переносе между дисками | атомарность: стопка никогда не содержит недописанный файл под настоящим именем | копия сразу под итоговым именем оставит битый файл при сбое |
| +≈ 70 МБ runtime и модели внутри exe | «один exe» и офлайн (принцип III) | отдельная папка рядом с exe ломает portable-копирование одним файлом |
