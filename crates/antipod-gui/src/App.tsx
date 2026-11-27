import { useEffect, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type LaunchMode = "ide" | "app";

interface ProfileStatus {
  name: string;
  mode: LaunchMode;
  pid: number | null;
  ram_mb: number | null;
  process_count: number | null;
  titlebar_color: string;
  default_project: string;
  chrome_profile: string | null;
  browser_path: string | null;
  mostlogin_profile_id: string | null;
  proxy: string | null;
  proxy_display: string | null;
}

interface ProxyTestResult {
  ip: string;
  country: string;
  country_code: string;
  city: string;
  latency_ms: number;
}

interface MostLoginSettings {
  api_url: string;
  api_token: string | null;
}

interface MostLoginProfileInfo {
  id: string;
  name: string;
}

function formatBrowserLabel(
  p: ProfileStatus,
  mlProfiles: MostLoginProfileInfo[]
): string {
  if (p.mostlogin_profile_id) {
    const matched = mlProfiles.find((m) => m.id === p.mostlogin_profile_id);
    if (matched) {
      return `MostLogin: ${matched.name}`;
    }
    const shortId =
      p.mostlogin_profile_id.length > 12
        ? `${p.mostlogin_profile_id.slice(0, 12)}...`
        : p.mostlogin_profile_id;
    return `MostLogin (${shortId})`;
  }
  if (p.browser_path) {
    const exeName = p.browser_path.split(/[/\\]/).pop();
    return exeName || p.browser_path;
  }
  if (p.proxy_display) {
    return "Авто (изолированный под прокси)";
  }
  return p.chrome_profile ? `Авто (${p.chrome_profile})` : "Авто (Chrome/Edge)";
}

export default function App() {
  const [profiles, setProfiles] = useState<ProfileStatus[]>([]);
  const [autostart, setAutostart] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [testingKey, setTestingKey] = useState<string | null>(null);

  const [mlApiUrl, setMlApiUrl] = useState("http://127.0.0.1:30898");
  const [mlApiToken, setMlApiToken] = useState("");
  const [mlProfiles, setMlProfiles] = useState<MostLoginProfileInfo[]>([]);
  const [mlLoading, setMlLoading] = useState(false);

  const [newName, setNewName] = useState("");
  const [newMode, setNewMode] = useState<LaunchMode>("ide");
  const [newColor, setNewColor] = useState("#2b3a55");
  const [newProject, setNewProject] = useState("C:\\Projects");
  const [newChromeProfile, setNewChromeProfile] = useState("Profile 1");
  const [newBrowserPath, setNewBrowserPath] = useState("");
  const [newMostLoginId, setNewMostLoginId] = useState("");
  const [newProxy, setNewProxy] = useState("");

  async function refresh() {
    try {
      const [list, auto] = await Promise.all([
        invoke<ProfileStatus[]>("list_profiles"),
        invoke<boolean>("get_autostart"),
      ]);
      setProfiles(list);
      setAutostart(auto);
    } catch (err) {
      setStatusMsg(`Ошибка загрузки: ${String(err)}`);
    }
  }

  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), 3000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const settings = await invoke<MostLoginSettings>(
          "get_mostlogin_settings"
        );
        setMlApiUrl(settings.api_url || "http://127.0.0.1:30898");
        setMlApiToken(settings.api_token ?? "");
        if (settings.api_token) {
          const loaded = await invoke<MostLoginProfileInfo[]>(
            "fetch_mostlogin_profiles",
            {
              apiUrl: settings.api_url,
              apiToken: settings.api_token,
            }
          );
          setMlProfiles(loaded);
        }
      } catch {
        // MostLogin может быть не запущен при старте AntiPod — не мешаем работе
      }
    })();
  }, []);

  const totalRamMb = profiles.reduce((acc, p) => acc + (p.ram_mb ?? 0), 0);

  async function handleLaunch(name: string) {
    setBusy(true);
    setStatusMsg("");
    try {
      const pid = await invoke<number>("launch_profile", { name });
      setStatusMsg(`Профиль "${name}" запущен (PID ${pid})`);
      await refresh();
    } catch (err) {
      setStatusMsg(`Не удалось запустить "${name}": ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleStop(name: string) {
    setBusy(true);
    try {
      await invoke("stop_profile", { name });
      setStatusMsg(`Профиль "${name}" остановлен`);
      await refresh();
    } catch (err) {
      setStatusMsg(`Ошибка остановки: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleOpenBrowser(p: ProfileStatus) {
    setBusy(true);
    try {
      const pid = await invoke<number>("open_chrome_oauth", { name: p.name });
      if (p.mostlogin_profile_id) {
        const pidPart = pid > 0 ? ` (PID ${pid})` : "";
        setStatusMsg(
          `Открыт профиль MostLogin для "${p.name}"${pidPart}`
        );
      } else {
        setStatusMsg(`Открыт браузер для профиля "${p.name}"`);
      }
    } catch (err) {
      setStatusMsg(`Ошибка запуска браузера: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleCloseMostLogin(name: string) {
    setBusy(true);
    try {
      await invoke("close_mostlogin_window", { name });
      setStatusMsg(`Окно MostLogin для "${name}" закрыто`);
    } catch (err) {
      setStatusMsg(`Не удалось закрыть окно MostLogin: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveMostLogin() {
    setMlLoading(true);
    try {
      await invoke("save_mostlogin_config", {
        apiUrl: mlApiUrl.trim(),
        apiToken: mlApiToken.trim() || null,
      });
      setStatusMsg("Настройки MostLogin Local API сохранены");
    } catch (err) {
      setStatusMsg(`Ошибка сохранения MostLogin API: ${String(err)}`);
    } finally {
      setMlLoading(false);
    }
  }

  async function handleLoadMostLoginProfiles() {
    setMlLoading(true);
    setStatusMsg("Загружаем список профилей из MostLogin...");
    try {
      const cleanUrl = mlApiUrl.trim() || "http://127.0.0.1:30898";
      const cleanToken = mlApiToken.trim() || null;
      await invoke("save_mostlogin_config", {
        apiUrl: cleanUrl,
        apiToken: cleanToken,
      });
      const list = await invoke<MostLoginProfileInfo[]>(
        "fetch_mostlogin_profiles",
        {
          apiUrl: cleanUrl,
          apiToken: cleanToken,
        }
      );
      setMlProfiles(list);
      setStatusMsg(
        list.length > 0
          ? `Загружено профилей MostLogin: ${list.length}. Выбери нужный в списке ниже.`
          : "MostLogin ответил успешно, но список профилей пуст."
      );
    } catch (err) {
      setStatusMsg(`Ошибка MostLogin API: ${String(err)}`);
    } finally {
      setMlLoading(false);
    }
  }

  async function handleTestProxy(rawProxy: string, key: string, label: string) {
    const trimmed = rawProxy.trim();
    if (!trimmed) {
      setStatusMsg("Сначала укажи адрес прокси для проверки");
      return;
    }
    setTestingKey(key);
    setStatusMsg(`Проверяем прокси (${label})...`);
    try {
      const res = await invoke<ProxyTestResult>("test_proxy_connection", {
        proxy: trimmed,
      });
      const cityPart = res.city ? `, ${res.city}` : "";
      setStatusMsg(
        `Прокси (${label}) работает: ${res.ip} · ${res.country} (${res.country_code})${cityPart} · ${res.latency_ms} мс`
      );
    } catch (err) {
      setStatusMsg(`Прокси (${label}) не прошёл проверку: ${String(err)}`);
    } finally {
      setTestingKey(null);
    }
  }

  function handleEditProfile(p: ProfileStatus) {
    setNewName(p.name);
    setNewMode(p.mode);
    setNewColor(p.titlebar_color);
    setNewProject(p.default_project);
    setNewChromeProfile(p.chrome_profile ?? "");
    setNewBrowserPath(p.browser_path ?? "");
    setNewMostLoginId(p.mostlogin_profile_id ?? "");
    setNewProxy(p.proxy ?? "");
    setStatusMsg(
      `Редактирование профиля "${p.name}" — внеси изменения в форме ниже и нажми «Сохранить»`
    );
  }

  async function handlePickFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setNewProject(selected);
    }
  }

  async function handlePickBrowser() {
    const selected = await open({
      directory: false,
      multiple: false,
      filters: [{ name: "Программы (.exe)", extensions: ["exe"] }],
    });
    if (typeof selected === "string") {
      setNewBrowserPath(selected);
    }
  }

  async function handleAddProfile(e: FormEvent) {
    e.preventDefault();
    if (!newName.trim()) return;
    setBusy(true);
    try {
      await invoke("save_profile", {
        input: {
          name: newName.trim(),
          mode: newMode,
          titlebarColor: newColor,
          defaultProject: newProject,
          chromeProfile: newChromeProfile.trim() || null,
          browserPath: newBrowserPath.trim() || null,
          mostloginProfileId: newMostLoginId.trim() || null,
          proxy: newProxy.trim() || null,
        },
      });
      setStatusMsg(`Профиль "${newName.trim()}" сохранён`);
      setNewName("");
      setNewBrowserPath("");
      setNewMostLoginId("");
      setNewProxy("");
      await refresh();
    } catch (err) {
      setStatusMsg(`Ошибка сохранения: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(name: string) {
    setBusy(true);
    try {
      await invoke("delete_profile", { name });
      setStatusMsg(`Профиль "${name}" удалён`);
      await refresh();
    } catch (err) {
      setStatusMsg(`Ошибка удаления: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleAutostart() {
    try {
      const next = !autostart;
      await invoke("set_autostart", { enabled: next });
      setAutostart(next);
    } catch (err) {
      setStatusMsg(`Ошибка автозапуска: ${String(err)}`);
    }
  }

  async function handleOpenReleases() {
    try {
      await invoke("open_releases");
    } catch (err) {
      setStatusMsg(`Не удалось открыть страницу релизов: ${String(err)}`);
    }
  }

  return (
    <div className="container">
      <header className="header">
        <div>
          <h1>AntiPod</h1>
          <p className="subtitle">
            Изолированные профили Google Antigravity · Активное ОЗУ:{" "}
            <strong>{totalRamMb} MB</strong>
          </p>
        </div>
        <div className="header-actions">
          <label className="toggle">
            <input
              type="checkbox"
              checked={autostart}
              onChange={handleToggleAutostart}
            />
            <span>Автозапуск</span>
          </label>
          <button onClick={handleOpenReleases} disabled={busy}>
            Релизы
          </button>
        </div>
      </header>

      {statusMsg && <div className="status">{statusMsg}</div>}

      <section className="grid">
        {profiles.map((p) => {
          const isRunning = p.pid !== null;
          const isTestingThis = testingKey === `card:${p.name}`;
          return (
            <div className="card" key={p.name}>
              <div className="card-head">
                <div className="card-title">
                  <span
                    className="color-dot"
                    style={{ backgroundColor: p.titlebar_color }}
                  />
                  <strong>{p.name}</strong>
                  <span className="badge">{p.mode.toUpperCase()}</span>
                </div>
                <span className={isRunning ? "pill running" : "pill stopped"}>
                  {isRunning
                    ? `${p.ram_mb ?? 0} MB · ${p.process_count ?? 1} proc`
                    : "Остановлен"}
                </span>
              </div>

              <div className="card-meta">
                <div className="card-path" title={p.default_project}>
                  {p.default_project}
                </div>
                <div
                  className="card-sub"
                  title={
                    p.mostlogin_profile_id
                      ? `MostLogin profileId: ${p.mostlogin_profile_id}`
                      : p.browser_path ??
                        "Автопоиск: Chrome → Edge → Brave → Firefox"
                  }
                >
                  Браузер: {formatBrowserLabel(p, mlProfiles)}
                  {isRunning ? ` · PID ${p.pid}` : ""}
                </div>
                {p.proxy_display && (
                  <div
                    className="card-sub"
                    title="Локальный мост для Antigravity и Chromium-браузеров"
                  >
                    Прокси: {p.proxy_display}
                  </div>
                )}
              </div>

              <div className="card-actions">
                {isRunning ? (
                  <button
                    className="btn-danger"
                    onClick={() => handleStop(p.name)}
                    disabled={busy}
                  >
                    Стоп
                  </button>
                ) : (
                  <button
                    className="btn-primary"
                    onClick={() => handleLaunch(p.name)}
                    disabled={busy}
                  >
                    Запуск
                  </button>
                )}
                <button
                  onClick={() => handleOpenBrowser(p)}
                  disabled={busy}
                  title={
                    p.mostlogin_profile_id
                      ? "Открыть привязанный профиль через MostLogin Local API"
                      : "Открыть выбранный браузер (или Chrome/Edge по умолчанию)"
                  }
                >
                  {p.mostlogin_profile_id ? "MostLogin" : "Браузер"}
                </button>
                {p.mostlogin_profile_id && (
                  <button
                    className="btn-ghost"
                    onClick={() => handleCloseMostLogin(p.name)}
                    disabled={busy}
                    title="Закрыть окно этого профиля в MostLogin"
                  >
                    Закр. ML
                  </button>
                )}
                {p.proxy && (
                  <button
                    onClick={() =>
                      handleTestProxy(p.proxy!, `card:${p.name}`, p.name)
                    }
                    disabled={busy || testingKey !== null}
                    title="Проверить подключение, внешний IP и страну прокси"
                  >
                    {isTestingThis ? "Проверка..." : "Тест прокси"}
                  </button>
                )}
                <button
                  className="btn-ghost"
                  onClick={() => handleEditProfile(p)}
                  disabled={busy}
                  title="Редактировать профиль"
                >
                  Ред.
                </button>
                <button
                  className="btn-ghost"
                  onClick={() => handleDelete(p.name)}
                  disabled={busy}
                  title="Удалить профиль"
                >
                  ×
                </button>
              </div>
            </div>
          );
        })}
      </section>

      <section className="add-form">
        <h2>MostLogin Local API</h2>
        <div className="form-row">
          <input
            type="text"
            value={mlApiUrl}
            onChange={(e) => setMlApiUrl(e.target.value)}
            placeholder="http://127.0.0.1:30898"
          />
          <input
            type="password"
            value={mlApiToken}
            onChange={(e) => setMlApiToken(e.target.value)}
            placeholder="API-токен (например, Bearer xxx... или просто ключ)"
          />
          <button
            type="button"
            onClick={handleSaveMostLogin}
            disabled={mlLoading}
          >
            Сохранить API
          </button>
          <button
            type="button"
            className="btn-primary"
            onClick={handleLoadMostLoginProfiles}
            disabled={mlLoading}
          >
            {mlLoading ? "Загрузка..." : "Загрузить профили"}
          </button>
        </div>
      </section>

      <form className="add-form" onSubmit={handleAddProfile}>
        <h2>Добавить или обновить профиль</h2>
        <div className="form-row">
          <input
            type="text"
            placeholder="Имя (например, client-1)"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            required
          />
          <select
            value={newMode}
            onChange={(e) => setNewMode(e.target.value as LaunchMode)}
          >
            <option value="ide">IDE</option>
            <option value="app">Standalone App</option>
          </select>
          <input
            type="text"
            value={newChromeProfile}
            onChange={(e) => setNewChromeProfile(e.target.value)}
            placeholder="Профиль Chrome (опц.)"
          />
          <input
            type="color"
            value={newColor}
            onChange={(e) => setNewColor(e.target.value)}
            title="Цвет заголовка окна"
          />
        </div>
        <div className="form-row">
          <input
            type="text"
            value={newMostLoginId}
            onChange={(e) => setNewMostLoginId(e.target.value)}
            placeholder="MostLogin profileId (опц.) — запускает профиль через Local API"
          />
          {mlProfiles.length > 0 && (
            <select
              value={newMostLoginId}
              onChange={(e) => setNewMostLoginId(e.target.value)}
              title="Выбрать профиль из загруженного списка MostLogin"
            >
              <option value="">-- Выбрать из MostLogin --</option>
              {mlProfiles.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name} ({m.id.slice(0, 8)}...)
                </option>
              ))}
            </select>
          )}
          {newMostLoginId && (
            <button
              type="button"
              className="btn-ghost"
              onClick={() => setNewMostLoginId("")}
              title="Отвязать профиль MostLogin"
            >
              ×
            </button>
          )}
        </div>
        <div className="form-row">
          <input
            type="text"
            value={newBrowserPath}
            onChange={(e) => setNewBrowserPath(e.target.value)}
            placeholder="Браузер по умолчанию (опц.): путь к .exe, если не используется MostLogin API"
          />
          <button type="button" onClick={handlePickBrowser}>
            Выбрать .exe
          </button>
          {newBrowserPath && (
            <button
              type="button"
              className="btn-ghost"
              onClick={() => setNewBrowserPath("")}
              title="Сбросить на автопоиск браузера"
            >
              ×
            </button>
          )}
        </div>
        <div className="form-row">
          <input
            type="text"
            value={newProxy}
            onChange={(e) => setNewProxy(e.target.value)}
            placeholder="Прокси (опц.): http://user:pass@ip:port, socks5://user:pass@ip:port или ip:port:user:pass"
          />
          <button
            type="button"
            onClick={() =>
              handleTestProxy(newProxy, "form", newName.trim() || "форма")
            }
            disabled={!newProxy.trim() || testingKey !== null}
            title="Проверить прокси до сохранения профиля"
          >
            {testingKey === "form" ? "Проверка..." : "Тест прокси"}
          </button>
        </div>
        <div className="form-row">
          <input
            type="text"
            value={newProject}
            onChange={(e) => setNewProject(e.target.value)}
            placeholder="Путь к папке проекта"
          />
          <button type="button" onClick={handlePickFolder}>
            Выбрать папку
          </button>
          <button type="submit" className="btn-primary" disabled={busy}>
            Сохранить
          </button>
        </div>
      </form>
    </div>
  );
}
