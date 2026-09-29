# Data Model: Установщик и обновления

## Состояние обновления (в памяти, `AppState`)

| Поле | Тип | Смысл |
|------|-----|-------|
| `phase` | `idle` · `checking` · `latest` · `downloading` · `ready` · `failed` · `off` | что происходит сейчас |
| `version` | строка? | версия найденного обновления (`downloading`, `ready`) |
| `progress` | 0–1? | доля скачанного (`downloading`) |
| `notes` | строка? | «Что нового» — тело релиза |
| `error` | строка? | причина и следующий шаг по-русски (`failed`) |
| `checkedAt` | мс? | время последней удачной проверки |

Переходы:

```text
off ──(updates=on)──▶ idle
idle ──(старт / раз в 24 ч / «Проверить сейчас»)──▶ checking
checking ──▶ latest | downloading | failed
downloading ──▶ ready | failed
ready ──(закрытие окна / «Перезапустить»)──▶ установка (процесс завершается)
failed ──(следующая проверка)──▶ checking
любое ──(updates=off)──▶ off   (ready остаётся ready: скачанное ставится при закрытии)
```

Скачанные байты (`Update` + `Vec<u8>`) живут рядом, в `PendingUpdate`, до установки или выхода.

## Настройки (таблица `setting`, как в фиче 001)

| Ключ | Значения | По умолчанию |
|------|----------|--------------|
| `updates` | `on` · `off` | `on` |
| `update_version` | версия скачанного обновления | — |
| `update_notes` | «Что нового» скачанного обновления | — |

`set_setting_cmd` принимает `updates`; `update_*` пишет только Rust.

## Входящая папка (в памяти, `AppState.incoming`)

`Option<String>` — путь из аргументов запуска; фронт забирает один раз (`take_incoming`), дальше — событие
`open://folder` от второго запуска.

## Релиз (GitHub Releases)

`latest.json`: `version`, `notes` (тело релиза), `pub_date`, `platforms.windows-x86_64.{signature,url}` → NSIS
`Sorter_<версия>_x64-setup.exe`. Формирует `tauri-action`.
