import { invoke as nativeInvoke } from "@tauri-apps/api/core";
export const isWeb = import.meta.env.VITE_WEB === "true";
export const AUTH_REQUIRED_EVENT = "hghub-auth-required";
export async function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isWeb) return nativeInvoke<T>(cmd, args);
  if (["debug_log", "playback_log_write", "set_orientation"].includes(cmd)) return undefined as T;
  if (cmd === "playback_log_status") return { enabled: false, path: null, error: null } as T;
  if (cmd === "open_url") {
    const url = new URL(String(args?.url ?? ""));
    if (url.protocol !== "https:" || url.username || url.password) throw new Error("只允许打开 HTTPS 链接");
    window.open(url.href, "_blank", "noopener,noreferrer");
    return undefined as T;
  }
  const res = await fetch(`/api/${encodeURIComponent(cmd)}`, {
    method: "POST", credentials: "same-origin",
    headers: { "Content-Type": "application/json" }, body: JSON.stringify(args ?? {}),
  });
  if (res.status === 401) {
    window.dispatchEvent(new Event(AUTH_REQUIRED_EVENT));
    throw new Error("访问已过期，请重新输入访问密码");
  }
  const data = await res.json().catch(() => null);
  if (!res.ok) throw new Error(typeof data?.error === "string" ? data.error : `请求失败（HTTP ${res.status}）`);
  return data as T;
}
