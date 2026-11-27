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
  const [interceptOauth, setInterceptOauth] = useState(true);
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
      const [list, auto, intercept, notice] = await Promise.all([
        invoke<ProfileStatus[]>("list_profiles"),
        invoke<boolean>("get_autostart"),
        invoke<boolean>("get_intercept_oauth"),
        invoke<string | null>("poll_oauth_notice"),
      ]);
      setProfiles(list);
      setAutostart(auto);
      setInterceptOauth(intercept);
      if (notice) {
        setStatusMsg(notice);
      }
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
      setBusy
