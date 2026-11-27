<div align="center">

<img src="assets/banner.svg" alt="AntiPod Banner" width="100%" />
🌐 **Язык / Language:** **Русский** | [English](README.md)
# AntiPod

**Нативный лаунчер и менеджер профилей на Rust + Tauri v2 для изолированного запуска нескольких аккаунтов Google Antigravity на Windows**


[![Downloads](https://img.shields.io/github/downloads/pirotehnikng-cloud/antipod/total?label=Downloads&color=0078d4&logo=windows)](../../releases/latest)
[![Release](https://img.shields.io/github/v/release/pirotehnikng-cloud/antipod?color=2e8540&label=Release)](https://github.com/pirotehnikng-cloud/antipod/releases)
[![Windows Build](https://img.shields.io/github/actions/workflow/status/pirotehnikng-cloud/antipod/release.yml?label=Windows%20Build)](https://github.com/pirotehnikng-cloud/antipod/actions)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11%20(x64)-0078D6?logo=windows)](https://github.com/pirotehnikng-cloud/antipod/releases)
[![License](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)

[Скачать релиз для Windows](https://github.com/pirotehnikng-cloud/antipod/releases/latest) · [Возможности релиза](#-возможности-релиза-v131) · [Быстрый старт](#-быстрый-старт) · [CLI-утилита](#-консольная-утилита-antipod-cli)

</div>

---

**AntiPod** позволяет одновременно запускать несколько независимых окон **Google Antigravity** (в режиме **IDE** или **Standalone App**) под разными Google-аккаунтами без конфликтов сессий, куки, кэша и сетевых подключений. Написан на чистом Rust с минимальным потреблением оперативной памяти и поддержкой полностью портативного режима.

<div align="center">

### Первые тесты: одновременный запуск двух изолированных окон Antigravity

![Первые тесты AntiPod — два независимых окна Google Antigravity](assets/first-test.png)

*Два параллельно запущенных инстанса Google Antigravity с раздельными `user-data-dir`, окружением и сессиями под управлением AntiPod.*

</div>

---

## ✨ Возможности релиза `v1.3.1`

Нажми на нужный раздел ниже, чтобы раскрыть подробное описание и настройки:

<details open>
<summary><b>1. Полная изоляция профилей и режимы запуска (IDE и Standalone App)</b></summary>
<br>

- **Раздельные директории данных:** каждому профилю выделяется собственный каталог внутри `AntiPodProfiles/<имя_профиля>/` (`user-data`, `home`, при необходимости `extensions` и `chrome-data`).
- **Два режима работы:**
  - `ide` — полноценный редактор **Antigravity IDE** с независимыми настройками, темой и папкой проекта.
  - `app` — автономное окно **Antigravity Standalone App** для работы с диалогами агентов.
- **Визуальная маркировка окон:** настройка индивидуального цвета заголовка (`titlebar_color`) в `settings.json` каждого профиля, чтобы не путать открытые аккаунты.
- **Автоматический Git Worktree:** при открытии одного и того же Git-репозитория в двух разных профилях AntiPod автоматически создаёт отдельный `git worktree` для второго профиля, предотвращая блокировки `.git/index.lock`.
- **Контроль ресурсов и Job Objects:** мониторинг ОЗУ и количества дочерних процессов в реальном времени по каждому профилю; кнопка **Стоп** гарантированно завершает всё дерево процессов профиля.

</details>

<details>
<summary><b>2. Встроенный локальный прокси-мост (HTTP / HTTPS / SOCKS5 с авторизацией)</b></summary>
<br>

- **Обход ограничений Chromium на прокси с паролем:** Chromium-движок Antigravity не поддерживает авторизацию по логину и паролю в флаге `--proxy-server`. AntiPod поднимает на `127.0.0.1` отдельный локальный TCP-мост для каждого профиля и сам добавляет заголовки `Proxy-Authorization` или выполняет SOCKS5-рукопожатие.
- **Защита от утечек DNS:** передаёт в Antigravity и изолированный браузер правила `--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1`, направляя DNS-запросы через туннель.
- **Поддерживаемые форматы ввода:**
  - `http://user:pass@ip:port`
  - `https://user:pass@ip:port`
  - `socks5://user:pass@ip:port` (или `socks5h://...`)
  - `ip:port:user:pass` (короткий формат, автоматически распознаётся как HTTP)
  - `http://ip:port` или `socks5://ip:port` (без авторизации)
- **Кнопка «Тест прокси»:** проверяет доступность прокси до и после сохранения профиля (в форме и прямо на карточке), отлавливает ошибку неверного пароля (`HTTP 407`) и выводит **внешний IP, страну, город и пинг в мс**.

</details>

<details>
<summary><b>3. Интеграция с MostLogin Local API и выбор своего браузера</b></summary>
<br>

- **Нативное управление MostLogin (`http://127.0.0.1:30898`):**
  - Сохранение адреса Local API и `Authorization`-токена прямо в интерфейсе AntiPod.
  - Кнопка **Загрузить профили** автоматически запрашивает список профилей из MostLogin (`/api/profile/getProfiles`) и заполняет выпадающий список привязки.
  - Кнопки **MostLogin** и **Закр. ML** на карточке профиля открывают (`/api/browser/openBrowser`) и закрывают (`/api/browser/closeProfiles`) конкретный профиль по его `profileId`.
- **Выбор любого стороннего `.exe` браузера:** в настройках профиля можно указать путь к любому антидетект-браузеру или портативному браузеру. Сторонние лаунчеры запускаются чисто, без конфликтующих флагов `--user-data-dir`.
- **Умный автопоиск браузера:** если путь не задан, AntiPod автоматически ищет установленный браузер в порядке `Google Chrome → Microsoft Edge → Brave → Firefox`.

</details>

<details>
<summary><b>4. Автоматический перехват OAuth-ссылок в буфер обмена и MostLogin</b></summary>
<br>

- **Без смены системного браузера Windows:** при включённом переключателе **Перехват OAuth** в шапке окна AntiPod прописывает себя в `workbench.externalBrowser` внутри настроек профиля `IDE` и передаёт имя профиля через переменную среды `ANTIPOD_PROFILE`.
- **Мгновенная обработка ссылки входа:** при нажатии кнопки авторизации Google внутри Antigravity IDE ссылка перехватывается фоновым процессом `AntiPod.exe` (без открытия второго окна GUI):
  1. Автоматически копируется в буфер обмена Windows.
  2. Если к профилю привязан **MostLogin**, открывается новой вкладкой прямо в активном окне профиля (через CDP-порт `/json/new` либо запуск профиля через Local API).
  3. Уведомление об успешном перехвате отображается в строке статуса AntiPod.

</details>

<details>
<summary><b>5. Портативный режим, системный трей и автозапуск</b></summary>
<br>

- **Portable-режим:** создайте пустой файл `portable.flag` (или положите готовый `profiles.toml`) рядом с `AntiPod.exe` — все профили, настройки и `state.json` будут храниться локально в папке с программой, без записи в `%APPDATA%`.
- **Работа в системном трее:** закрытие окна сворачивает AntiPod в трей Windows, где доступны быстрый запуск/остановка отдельных профилей с отображением их ОЗУ, остановка всех инстансов сразу и переключение автозапуска с Windows (`--minimized`).

</details>

---

## 🚀 Быстрый старт

1. Скачайте свежий архив **`AntiPod-v1.3.1-windows-x64-portable.zip`** со страницы [Releases](https://github.com/pirotehnikng-cloud/antipod/releases/latest) и распакуйте в удобную папку (например, `C:\AntiPod`).
2. *(Опционально)* Для портативного режима убедитесь, что рядом с `AntiPod.exe` лежит файл `portable.flag`.
3. Запустите `AntiPod.exe`.
4. *(Для пользователей MostLogin)* В блоке **MostLogin Local API** укажите адрес `http://127.0.0.1:30898`, вставьте ваш API-токен (например, `Bearer <ваш_токен>`) и нажмите **Загрузить профили**.
5. В форме **Добавить или обновить профиль** укажите имя аккаунта, выберите режим (`IDE` или `Standalone App`), привяжите профиль MostLogin (или укажите `.exe` браузера), вставьте прокси и проверьте его кнопкой **Тест прокси**, затем нажмите **Сохранить**.
6. Нажмите **MostLogin** (или **Браузер**) для открытия окна браузера и **Запуск** для старта изолированного Google Antigravity.

---

## 💻 Консольная утилита (`antipod-cli`)

В состав проекта входит CLI-утилита `antipod.exe` для управления профилями из терминала и PowerShell-скриптов:

<details>
<summary><b>Показать команды CLI</b></summary>
<br>

```powershell
# Создать стартовый конфигурационный файл profiles.toml
antipod init

# Вывести таблицу всех профилей, привязанных браузеров и прокси
antipod list

# Запустить изолированный профиль Antigravity (с автоматическим поднятием прокси-моста)
antipod launch work

# Открыть привязанный профиль MostLogin или браузер для OAuth
antipod browser work

# Показать список профилей из MostLogin Local API
antipod mostlogin-list

# Проверить прокси по имени профиля или по прямому адресу
antipod test-proxy work
antipod test-proxy "socks5://user:pass@1.2.3.4:1080"

# Остановить дерево процессов профиля
antipod stop work
