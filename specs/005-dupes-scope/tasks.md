# Tasks: Область поиска дублей

**Input**: `specs/005-dupes-scope/` — spec, plan, research, data-model, contracts/ipc.md, contracts/ui.md, quickstart;
формы — `.planning/DESIGN.md` («003/004/005 — выбор»), разметка — `.planning/sketches/00{3,4,5}-*/`

**Tests**: `cargo test --release --lib` там, где есть логика без UI (набор поиска, состав отдельного поиска, фильтр
групп, ход из внешней папки, этапы статуса) — как в фичах 001–004. UI — приёмка через CDP по quickstart.

## Phase 1: Setup

- [X] T001 Копия для хода из внешней папки: `G:\sorter-test\ext` — 3 файла из `G:\vk videos`, среди них `1525005239349.mp4`; оригинал не трогать
- [X] T002 Замер до правок (FR-024): игнорируемый bench-тест `bench_dupes_stage` в `src-tauri/src/develop.rs` — копия базы с колодой `G:\sorter-test\big` и столом `big-table`, 200 файлов этапа «Дубли» с разбивкой времени: `files()`/`next_file`, `cached_sha`/`refresh_exact`, `print::print`, `near::embed`, `index.candidates`+`compare`; плюс время до 500 кадров на свежей базе в dev с зондом `hangprobe.ps1`. Числа — в `specs/005-dupes-scope/research.md` R-6 («Замер до»)

## Phase 2: Foundational

- [X] T003 `src-tauri/src/commands.rs`: `dupes_enabled`, `dupes_scope` в `USER_KEYS` и `SETTING_KEYS`; `dupes_enabled` = `"1"` или ключ удаляется («`1` / нет ключа»), `dupes_scope` ∈ {`deck`, `all`} (иное — `BAD_SETTING`); `dupe_search` — в `SETTING_KEYS` (читается в `get_state`), но не в `USER_KEYS`; после записи — `dupes://changed` и пробуждение цикла (`wake`)
- [X] T004 `src-tauri/src/dupes/mod.rs`: `enum Scope { Off, Deck, All, Search(SearchSpec) }`, `Scope::current(conn)` (`dupe_search.state ∈ {running, done}` → `Search`, иначе `dupes_enabled`/`dupes_scope`, нет ключа = `Off`, нет `dupes_scope` = `Deck`), `Scope::deck_only(conn)` для плашек (без `Search`); `scope_files(conn, &Scope)` вместо `files()` — `Off` пусто, `Deck` карты колоды, `All` + стопки рекурсивно, `Search` — через `dupes::search` (T030); `files()` удалить, все вызовы (`exact_pairs`, `printed`, `near::next_file`) перевести на `scope_files(conn, &Scope::current(conn)?)`
- [X] T005 `src-tauri/src/dupes/mod.rs`: `groups(conn)` берёт только пары, у которых оба пути в наборе `Scope::current` (сравнение без регистра); `dupes_for` — набор `Scope::deck_only`, при `Off` — `[]`; `Groups` + `scope: &'static str` (`off`/`deck`/`all`/`search`) и `search: Option<SearchSummary>`; `GroupItem.where` + `"folder"` и поле `folder` (каталог файла), когда файл не в колоде и не в стопке стола
- [X] T006 Тесты в `src-tauri/src/dupes/mod.rs`: набор `Off`/`Deck`/`All` по настройкам (переписать `exact_pairs_hash_only_same_size_and_skip_dismissed`, `pile_subfolders_take_part_and_are_named` под включённый `all`); `groups` при `deck` прячет группу со стопкой и показывает её при `all`; `dupes_for` при `Off` пуст
- [X] T007 `src/lib/ipc.ts`: настройки `dupes_enabled`, `dupes_scope`, `dupe_search`; `Developing` без `pair`, + `scope`, `search`; `DupeGroups` + `scope`, `search: SearchSummary | null`; `GroupItem.where` + `"folder"`, `folder?`; команды `dupesEstimate`, `searchPreview`, `searchStart`, `searchClear` (contracts/ipc.md)

**Checkpoint**: `cargo test` зелёный; поиск по умолчанию выключен, при `dupes_enabled=1` всё работает как раньше в границах набора.

## Phase 3: User Story 1 — Выключил дубли — раскладываю сразу (P1) 🎯 MVP

**Goal**: по умолчанию разбор — только кадры (и подсказки), стопки не читаются, плашек нет.

**Independent Test**: свежая база, колода `big`: этапы «Файлы», «Кадры», по концу кадров — «Колода готова»; `fingerprint` не растёт.

- [X] T008 [US1] `src-tauri/src/develop.rs`: `status()` — этап `dupes` только при наборе ≠ `Off` (`printed` = (0, 0) при `Off`), поле `scope`; `refresh_exact` в цикле `spawn` — только когда кадры колоды готовы (`next()` пуст) и набор ≠ `Off`, первая итерация не считает SHA (FR-022); `Printer.step` не вызывается при `Off`
- [X] T009 [US1] Тест `src-tauri/src/develop.rs`: `status` при `Off` после кадров — `done` без этапа `dupes`; при `deck` — `dupes`, пока есть файлы без отпечатка
- [X] T010 [P] [US1] `src/components/screens/DupesStep.tsx` (новое): шаг «Дубли» по стенду 003 Шаг A — карточка `raised` r20 с номером 4, подпись «Дубли», значение «Искать копии», тумблер `Toggle` справа; выключено — «Выключено — колода откроется сразу после кадров.»; пишет `dupes_enabled`
- [X] T011 [US1] `src/components/screens/Start.tsx`: четвёртый шаг `DupesStep` под «Подсказками»
- [X] T012 [US1] `src/components/screens/Develop.tsx`: этап «Дубли» в списке только при `p.scope !== "off"`; веса `percent()` без доли дублей при `off`; итоги — плитка «групп дублей» только при включённом поиске
- [X] T013 [US1] `src/components/deck/DeckScreen.tsx`, `src/lib/useDupes.ts`: при `dupes_enabled` выключенном плашки не запрашиваются; счётчик «Дубли N» в сегменте режима — только при включённом
- [X] T014 [US1] Приёмка US1 через CDP: пункт 1 quickstart

## Phase 4: User Story 2 — Включил дубли и выбрал, где искать (P1)

**Goal**: область «Внутри колоды» / «Колода и стол», видно везде, сужение мгновенно, расширение — через шлюз.

**Independent Test**: `small-60` + `big-table`: «Внутри колоды» — группы только из колоды; «Колода и стол» — ещё пары `1525005239349*` из стопок.

- [X] T015 [US2] `src-tauri/src/commands.rs`: `dupes_estimate(scope)` async → `{ files, videos, photos, minutes }` — файлы набора без готового отпечатка × `dupes_rate_video_ms` / `dupes_rate_photo_ms` (константы из замера T002, по умолчанию); `src-tauri/src/develop.rs` по концу этапа «Дубли» пишет фактические средние в эти ключи
- [X] T016 [US2] `src/components/screens/DupesStep.tsx`: область — стенд 003 Область B: блок `cosmic` r16 padding 4, две строки r12 padding 8/10 (выбранная — `raised`), радиоточка 16 (кольцо 1,5 `dim` / кольцо 5 `fg`), «Внутри колоды — Копии среди файлов этой папки», «Колода и стол — Ещё и с тем, что уже разложено: N стопки»; видна только при включённом; пишет `dupes_scope`; чип «+N мин» (капсула 22 `strong` 12 bold, Время B) справа от «Искать копии» из `dupesEstimate`, скрыт без колоды и при 0; подписи из contracts/ui.md
- [X] T017 [US2] `src/components/screens/Settings.tsx`: плитка «Поиск дублей» с тем же `DupesStep` (без номера)
- [X] T018 [US2] `src/App.tsx`: цель шлюза `develop: "deck" | "dupes" | "search" | null`; включение поиска или расширение `deck → all` во время раскладки (из настроек и экрана «Дубли») → `develop = "dupes"` с запомненными прежними `dupes_enabled`/`dupes_scope`; сужение — без шлюза
- [X] T019 [US2] `src/components/screens/Develop.tsx`: чип по стенду 005 Где ищу C — «Разбор колоды · дубли внутри колоды» / «· дубли в колоде и на столе», при `off` — «Разбор колоды»; цель `dupes`: «Раскладывать Enter» → колода на той же карте, Esc — вернуть прежние настройки и в колоду (не на старт)
- [X] T020 [US2] Приёмка US2 через CDP: пункт 2 quickstart

## Phase 5: User Story 3 — Экран «Дубли» говорит, что поиск выключен (P1)

**Goal**: честное состояние «выключен» и включение с выбором области.

**Independent Test**: поиск выключен → Ctrl 3 → карточка «Поиск дублей **выключен**» → Enter → окно области → шлюз → группы.

- [X] T021 [P] [US3] `src/components/dupes/DupesOff.tsx` (новое): стенд 004 Выключен B — карточка `raised` r24 ширина 540 padding 30/32 по центру, значок copy с чертой в круге 52 `strong`, h2 28 «Поиск дублей **выключен**», текст 14 `dim`, кнопки «Включить поиск Enter» (primary 48) и «Найти дубли… F» (secondary, значок `Search`), пояснение 13 `dim` (тексты — DESIGN.md «004 — выбор»); без слов «дублей нет»
- [X] T022 [P] [US3] `src/components/dupes/EnableDupes.tsx` (новое): стенд 004 Включить A — затемнение `rgb(8 8 10 / 52%)` под заголовком окна, окно `cosmic` r28 ширина 560 padding 26/28/24, «Где искать **дубли?**», две карточки-области `raised` r20 (выбранная — кольцо 1,5 `fg`) с числом файлов из `dupesEstimate`, «Займёт около N мин», «Отмена Esc», «Начать поиск Enter»; ← → / 1 2 — выбор области; фокус в окне, Esc закрывает
- [X] T023 [US3] `src/components/deck/DeckScreen.tsx`: режим «Дубли» при выключенном поиске — заголовок «Дубли **не ищу**», чип «Поиск дублей выключен», `DupesOff`; Enter → `EnableDupes`; «Начать поиск» → `dupes_enabled=1`, `dupes_scope`, цель шлюза `dupes` (T018)
- [X] T024 [US3] `src/components/deck/DeckScreen.tsx`: при включённом — строка под заголовком (стенд 004 Где ищу B) «Ищу **внутри колоды**» / «Ищу **в колоде и на столе**» 13 `dim` + капсула 24 «Изменить» (`raised`, 12 bold) → `EnableDupes` с текущей областью; чип «Отпечатано N из M»; кнопка «Найти дубли… F» (secondary 38, значок `Search`) в действиях шапки перед фильтром видов
- [X] T025 [US3] `src/components/dupes/DupesScreen.tsx`: пустой список при включённом поиске — «Дублей не нашлось.» (без «или отпечатки ещё считаются» — шлюз держит до конца)
- [X] T026 [US3] Приёмка US3 через CDP: пункт 3 quickstart

## Phase 6: User Story 4 — Анимация поиска вместо картинок (P2)

**Goal**: на этапе «Дубли» — «Сито», без кадров файлов.

**Independent Test**: этап «Дубли» на `small-60`: сетка, без картинок; Space — замирает и гаснет.

- [X] T027 [P] [US4] `src/components/fx/Sieve.tsx` (новое) + правила в `src/styles.css`: стенд 005 Анимация A — сетка 10×5 плиток 40×56 r9 промежуток 12 (`#19191c`, кольцо `#26262a`), волна `#303036` (задержка столбец·0,12 с + строка·0,06 с, период 4 с), три пары (сдвиг 0/2/4 с, период 6 с) с ореолом и SVG-дугой `stroke-dashoffset`; ключевые кадры `sv-scan`, `sv-match`, `sv-line` из `.planning/sketches/005-dupes-anim/stand.css`; проп `paused` → `animation-play-state: paused` и прозрачность .5; `prefers-reduced-motion` — без анимации
- [X] T028 [US4] `src/components/screens/Develop.tsx`: на этапе `dupes` живая область — `Sieve` с подписью «СРАВНИВАЮ», без счёта; удалить `pair`, `lastPair`, пару в `useSynced`; `src-tauri/src/develop.rs` — убрать `Status.pair`, `named()`, параметр `pair` у `Report::tick`; `src-tauri/src/dupes/near.rs` — убрать `Printer.last`
- [X] T029 [US4] Приёмка US4 через CDP: пункт 4 quickstart

## Phase 7: User Story 5 — Отдельный поиск среди стопок и любых папок (P2)

**Goal**: «Найти дубли…» — состав, шлюз, итоги, убрать копию из внешней папки с отменой.

**Independent Test**: Друзья + Мемы + `G:\sorter-test\ext` → группа с `ext\1525005239349.mp4` «в папке …» → убрать → Ctrl Z.

- [X] T030 [US5] `src-tauri/src/dupes/search.rs` (новое): `SearchSpec { piles, deck, folders, state, skipped }` (serde, ключ `dupe_search`); `load`/`save`/`clear`; `files(conn, &spec)` — отмеченные стопки рекурсивно + колода (если `deck`) + папки рекурсивно тем же `walk` (скрытые/системные и `.`/`$`/`@` пропускаются); папка, вложенная в другую папку состава, стопку стола или колоду, отбрасывается; пути без регистра — один раз; недоступная папка — в `skipped`; `preview(conn, piles, deck, folders)` → `{ total, roots: [{ path, files, error }] }` с ошибками «Нет доступа к папке.» / «Диск не найден.»
- [X] T031 [US5] Тесты `src-tauri/src/dupes/search.rs`: вложенная папка и папка внутри стопки не удваивают файлы; скрытая подпапка пропущена; несуществующая папка — в `skipped`, остальные считаются; одна папка — пары внутри неё; `Scope::current` = `Search` при `running`/`done`
- [X] T032 [US5] `src-tauri/src/commands.rs`: `search_preview` (async, `state.read()`), `search_start` (sync: состав не пуст, иначе `EMPTY_SEARCH` «Отметь хотя бы одну стопку, колоду или папку.»; пишет `state = running`, будит цикл), `search_clear` (удаляет ключ, `dupes://changed`); регистрация в `src-tauri/src/lib.rs`
- [X] T033 [US5] `src-tauri/src/develop.rs`: при `Scope::Search` — этапы `files` → `dupes` → `done` без кадров колоды; по концу (все файлы состава отпечатаны, `refresh_exact` по составу) — `state = done`; `Status.search = true`
- [X] T034 [US5] Тест `src-tauri/src/moves.rs`: `trash_copies` для файла во временной «внешней» папке (не колода, не стол) — ход в журнале до выполнения, файл в «Корзине» (через `RenameFn`-заглушку, как в существующих тестах), `undo_last` возвращает в ту же папку под тем же именем
- [X] T035 [P] [US5] `src/components/dupes/FindDupes.tsx` (новое): стенд 004 Найти B — панель 430 справа от заголовка окна до низа (отступ 8), `cosmic` r28 padding 24/24/20, затемнение 30 %; «Найти **дубли**», пояснение; «Стопки стола» строками (Стопки A: блок `raised` r18 padding 4, строки 40 r12, флажок 20 r6, «Все стопки» с «минусом» при частичном), «Колода · по желанию» строкой, «Другие папки · с подпапками» строками (Папки A: значок папки, путь, число, ×; недоступная — alert, зачёркнута, «нет доступа — пропущу») + пунктирная капсула «Добавить папку… Ctrl O» (`open({ directory: true })`); низ «Сравню **N файлов**» из `searchPreview` (пересчёт при каждом изменении), «Отмена Esc», «Искать Enter» (неактивна при пустом составе); по умолчанию отмечены все стопки
- [X] T036 [P] [US5] `src/components/dupes/SearchBanner.tsx` (новое): стенд 004 Итоги A — `raised` r20 с кольцом 1,5 `strong`, padding 10/10/10/16, значок `Search`, «Отдельный поиск · N групп» 14 bold, состав и число файлов 12 `dim`, пропущенные папки, «Новый поиск F», «Закрыть результаты Esc» (`strong`)
- [X] T037 [US5] `src/components/deck/DeckScreen.tsx`: F — `FindDupes` (в любом состоянии поиска, и при выключенном); «Искать» → `searchStart`, цель шлюза `search`; при `groups.search?.state === "done"` — чип «Отдельный поиск», заголовок «Дубли, **N групп**», `SearchBanner`; «Закрыть результаты» → `searchClear`; «Новый поиск» → `FindDupes` с прежним составом
- [X] T038 [US5] `src/components/dupes/DupesScreen.tsx`, `src/components/dupes/GroupCompare.tsx`: место копии `folder` — «в папке <каталог>» цветом `fg`; «Заменить» для группы без лучшей копии в колоде скрыто
- [X] T039 [US5] `src/components/screens/Develop.tsx`: цель `search` — заголовок «Ищу **дубли**», чип «Поиск дублей · <состав>», этапы «Файлы», «Дубли», подпись «Итоги откроются, когда поиск закончится.», «К дублям Enter» (по концу) → экран «Дубли», «Отменить поиск Esc» → `searchClear` и экран «Дубли»
- [X] T040 [US5] Приёмка US5 через CDP: пункт 5 quickstart, включая повтор состава (SC-006)

## Phase 8: User Story 6 — Быстрее снимать отпечатки (P3)

**Goal**: этап «Дубли» на `big` ≤ ½ времени из T002, кадры не ждут SHA, окно не подвисает.

**Independent Test**: повтор замера T002 после правок.

- [X] T041 [US6] `src-tauri/src/dupes/near.rs`: `Printer` строит список файлов набора один раз на проход (очередь), обновляет при смене `PRAGMA data_version` или набора; `next_file` больше не вызывает `scope_files` и не грузит всю `fingerprint` на каждый файл
- [X] T042 [US6] (не нужен по замеру, R-6) `src-tauri/src/dupes/index.rs`: `Index::load` — только записи набора (пути из очереди T041), `sync` — так же
- [X] T043 [US6] `src-tauri/src/dupes/near.rs`, `src-tauri/src/develop.rs`: съём отпечатков `print::print` в `workers()` потоков с пониженным приоритетом (MF на каждом потоке — `mf::start`), `embed` и сравнение — в потоке цикла по порядку очереди; пауза останавливает выдачу новых файлов
- [X] T044 [US6] Пункты по итогам замера T002, если узкое место другое (записать в research.md R-6 «Решение после замера»)
- [X] T045 [US6] Замер после: тот же bench T002 и dev на `big` — время этапа, время до 500 кадров с включённым и выключенным поиском, зонд подвисаний; числа — в research.md R-6 («Замер после»), сверка с SC-005

## Phase 9: Polish

- [X] T046 Проверки: `pnpm exec tsc --noEmit` → `pnpm build` → `cargo test --release --lib` (`CARGO_TARGET_DIR=target-test` при запущенном dev)
- [X] T047 Адверсариальная проверка диффа (`Agent`, `model: "sonnet"`): сверка со спекой и выбором стендов, обе темы, фокус, узкое окно; подтверждённое исправить
- [X] T048 Приёмка через CDP: пункты 6–7 quickstart (обе темы, ширина 1100)
- [ ] T049 `HANDOFF.md`, отметки задач; коммиты по фазам после одобрения пользователя (`feat: …` по-русски, без упоминаний ассистента)

## Dependencies & Execution Order

- T002 (замер) — до любых правок кода дублей.
- Phase 2 блокирует все истории. T004 → T005 → T006; T007 можно параллельно с T005–T006.
- US1 (Phase 3) → US2 (Phase 4, нужен `DupesStep` из T010) → US3 (Phase 5, нужна цель шлюза T018).
- US4 (Phase 6) — после Phase 2, независима от US2–US3 (кроме общего `Develop.tsx` — делать после T019).
- US5 (Phase 7) — после US3 (экран «Дубли» и шлюз), T030 нужен для `Scope::Search` в T004 (заглушка до T030).
- US6 (Phase 8) — после US5 (очередь работает и для отдельного поиска).

## Parallel Opportunities

- T010 ∥ T008–T009 (фронт и Rust).
- T021 ∥ T022 (разные новые файлы).
- T027 ∥ T030–T031.
- T035 ∥ T036 ∥ T034.

## Implementation Strategy

MVP — Phase 1–3: поиск выключен по умолчанию, большая колода открывается после кадров (главная боль). Затем US2–US3
(включение и честный экран «Дубли»), US4 (анимация), US5 (отдельный поиск), US6 (скорость по замеру). Коммит после
каждой фазы — только после одобрения пользователя.
