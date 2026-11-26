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
}

export default function App() {
  const [profiles, setProfiles] = useState<ProfileStatus[]>([]);
  const [autostart, setAutostart] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string>("");
  const [busy, setBusy] = useState(false);

  const [newName, setNewName] = useState("");
  const [newMode, setNewMode] = useState<LaunchMode>("ide");
  const [newColor, setNewColor] = useState("#2b3a55");
  const [newProject, setNewProject] = useState("C:\\Projects");
  const [newChromeProfile, setNewChromeProfile] = useState("Profile 1");

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

  async function handleOpenChrome(name: string) {
    try {
      await invoke<number>("open_chrome_oauth", { name });
      setStatusMsg(`Открыт Chrome для OAuth профиля "${name}"`);
    } catch (err) {
      setStatusMsg(`Ошибка запуска Chrome: ${String(err)}`);
    }
  }

  async function handlePickFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setNewProject(selected);
    }
  }

  async function handleAddProfile(e: FormEvent) {
    e.preventDefault();
    if (!newName.trim()) return;
    setBusy(true);
    try {
      await invoke("save_profile", {
        name: newName.trim(),
        mode: newMode,
        titlebarColor: newColor,
        defaultProject: newProject,
        chromeProfile: newChromeProfile.trim() || null,
      });
      setNewName("");
      setStatusMsg(`Профиль "${newName.trim()}" сохранён`);
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
                <div className="card-sub">
                  Chrome: {p.chrome_profile ?? "Default"}
                  {isRunning ? ` · PID ${p.pid}` : ""}
                </div>
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
                  onClick={() => handleOpenChrome(p.name)}
                  disabled={busy}
                  title={`Открыть Chrome (${p.chrome_profile ?? "Default"}) для OAuth`}
                >
                  Chrome OAuth
                </button>
                <button
                  className="btn-ghost"
                  onClick={() => handleDelete(p.name)}
                  disabled={busy}
                >
                  ×
                </button>
              </div>
            </div>
          );
        })}
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
            placeholder="Chrome (Profile 1 / incognito)"
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
