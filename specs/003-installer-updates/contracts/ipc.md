# Contract: команды, события, оболочка

## Команды Tauri

| Команда | Вход | Выход | Поведение |
|---------|------|-------|-----------|
| `update_state` | — | `UpdateState` | текущее состояние (см. data-model) + `current` — версия приложения |
| `update_check` | — | `UpdateState` | «Проверить сейчас»: проверка и, если есть, скачивание в фоне; при `off` — не ходит в сеть |
| `update_install` | — | — | «Перезапустить»: ждёт конец хода (блокировка базы), ставит с перезапуском |
| `set_setting_cmd` | `key: "updates"`, `value: "on"\|"off"` | — | включает/выключает проверку |
| `take_incoming` | — | `string \| null` | папка из аргументов запуска, один раз |

Ошибки — `AppError { code, message }` (фича 001): `UPDATE_NET` «Не удалось проверить — нет сети. Попробую позже.»,
`UPDATE_SIGNATURE` «Обновление отклонено: подпись не совпала. Попробую позже.», `UPDATE_INSTALL` «Обновление
не поставлено — нужно разрешение Windows. Нажми «Перезапустить», чтобы попробовать снова.»

## События

| Событие | Данные | Когда |
|---------|--------|-------|
| `update://state` | `UpdateState` | любая смена фазы; в `downloading` — не чаще раза в 250 мс |
| `open://folder` | `string` | второй запуск Sorter с папкой (Проводник) |

## Аргументы запуска

`sorter.exe "<папка>"` — папка становится колодой (проверки как у «Сменить колоду»). Не папка / нет аргумента —
обычный запуск.

## Реестр (установщик, `SHCTX\Software\Classes`)

| Ключ | Значения |
|------|----------|
| `Directory\shell\Sorter` | `MUIVerb` = «Разобрать с Sorter», `Icon` = `"$INSTDIR\sorter.exe",0`, `MultiSelectModel` = `Single` |
| `Directory\shell\Sorter\command` | по умолчанию `"$INSTDIR\sorter.exe" "%1"` |
| `Directory\Background\shell\Sorter` | как выше |
| `Directory\Background\shell\Sorter\command` | по умолчанию `"$INSTDIR\sorter.exe" "%V"` |

Удаляются в `NSIS_HOOK_PREUNINSTALL`.
