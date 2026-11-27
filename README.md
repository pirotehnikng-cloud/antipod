<p align="center">
  <img src="assets/banner.svg" alt="AntiPod Banner" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/pirotehnikng-cloud/antipod/releases"><img src="https://img.shields.io/github/v/release/pirotehnikng-cloud/antipod?style=flat-square&color=2ea043" alt="Release"></a>
  <a href="https://github.com/pirotehnikng-cloud/antipod/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/pirotehnikng-cloud/antipod/ci.yml?style=flat-square&label=Windows%20CI" alt="Windows CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-Apache--2.0-58a6ff?style=flat-square" alt="License"></a>
  <img src="https://img.shields.io/badge/Platform-Windows%20x64-0078d4?style=flat-square" alt="Windows">
</p>

**AntiPod** — нативный лаунчер и менеджер профилей на Rust + Tauri v2 для одновременного запуска двух и более изолированных аккаунтов **Google Antigravity** на одном ПК с Windows.

> **Важно:** AntiPod **не удаляет и не модифицирует** установленный Google Antigravity и не затрагивает ваш основной профиль. Программа запускает существующий бинарник в изолированном окружении.

## Возможности

- **Изоляция без сброса сессий:** отдельные `--user-data-dir`, виртуальный `USERPROFILE`/`HOME` и `SSH_CONNECTION` уводят OAuth-токены из общего Windows Credential Manager в локальную папку профиля.
- **Портативный и классический режимы:** запуск без установки из любой папки или флешки — при наличии файла `portable.flag` (или `profiles.toml`) рядом с `.exe` все настройки и профили хранятся локально.
- **Win32 Job Objects + мониторинг RAM:** чистое завершение всего дерева процессов (Electron + `language_server.exe`) и отображение потребления ОЗУ по каждому профилю в GUI и системном трее.
- **Кнопка Chrome OAuth:** запуск привязанного профиля Google Chrome (`--profile-directory` или `incognito`) прямо из карточки профиля.
- **Общие проекты и тулчейны:** NTFS Junctions для `Projects` и `.ssh` (права администратора не требуются) + проброс `.gitconfig`, `.cargo` и `.rustup`.
- **Защита Git через worktree:** автоматическое создание `git worktree`, если два аккаунта открывают один репозиторий одновременно.
- **Автозапуск с Windows:** тихий старт в системный трей (`--minimized`) с динамическим подменю профилей.

## Установка и запуск

Скачай нужный вариант со страницы [Releases](https://github.com/pirotehnikng-cloud/antipod/releases):

1. **Портативный GUI (с окном, без установки)** — `AntiPod-v1.0.2-windows-x64-portable.zip`
   - Распакуй архив в обычную папку (например, `C:\Tools\AntiPod`) и запусти `AntiPod.exe` двойным кликом.
   - Внутри архива уже лежит файл `portable.flag` — благодаря ему `profiles.toml`, `state.json` и папка `AntiPodProfiles` создаются прямо рядом с `AntiPod.exe`, не засоряя `%APPDATA%`.
2. **Установщик GUI (Windows)** — `AntiPod_1.0.2_x64-setup.exe` (NSIS) или `.msi`
   - Стандартная установка без прав администратора; конфиг хранится в `%APPDATA%\antipod\profiles.toml`.
3. **Портативный CLI (консоль)** — `antipod-cli-v1.0.2-windows-x64.zip`
   - Консольная утилита без графического окна. Запускай `antipod.exe` через PowerShell или Терминал (тоже поддерживает `portable.flag`).

При желании проверь целостность скачанного файла по `SHA256SUMS.txt`:

```powershell
Get-FileHash .\AntiPod-v1.0.2-windows-x64-portable.zip -Algorithm SHA256
```

## CLI-команды

```powershell
.\antipod.exe init            # Создать стартовый profiles.toml
.\antipod.exe list            # Список настроенных профилей
.\antipod.exe launch work     # Запустить профиль work
.\antipod.exe chrome work     # Открыть привязанный профиль Chrome для OAuth
.\antipod.exe stop work       # Завершить дерево процессов профиля work
```

## Дисклеймер

AntiPod — независимый open-source проект, не связанный с Google LLC и не одобренный ею. Инструмент предназначен для разделения личного и рабочего аккаунтов и предоставляется «как есть» на условиях лицензии Apache-2.0.
