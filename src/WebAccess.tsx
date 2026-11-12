import { useEffect, useState, type ReactNode } from "react";
import { AUTH_REQUIRED_EVENT } from "./transport";
export async function webLogout() {
  const res = await fetch("/auth/logout", { method: "POST", credentials: "same-origin", headers: { "Content-Type": "application/json" }, body: "{}" });
  if (!res.ok) throw new Error("退出失败，请重试");
  window.dispatchEvent(new Event(AUTH_REQUIRED_EVENT));
}
export default function WebAccess({ children }: { children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    const expired = () => { setAuthenticated(false); setReady(true); };
    window.addEventListener(AUTH_REQUIRED_EVENT, expired);
    fetch("/auth/status", { credentials: "same-origin", cache: "no-store" })
      .then(async r => { if (!r.ok) throw new Error(); return r.json(); })
      .then(data => { if (active) setAuthenticated(data.authenticated === true); })
      .catch(() => { if (active) setError("无法连接服务器，请检查部署是否启动后刷新页面。"); })
      .finally(() => { if (active) setReady(true); });
    return () => { active = false; window.removeEventListener(AUTH_REQUIRED_EVENT, expired); };
  }, []);
  if (!ready) return <div className="p-8 text-on">正在连接个人服务器…</div>;
  if (authenticated) return <>{children}</>;
  return <main className="min-h-dvh bg-bg text-on flex items-center justify-center p-6">
    <form className="w-full max-w-sm rounded-2xl bg-s2 p-6 space-y-4" onSubmit={async e => {
      e.preventDefault(); setBusy(true); setError("");
      try {
        const r = await fetch("/auth/login", { method: "POST", credentials: "same-origin",
          headers: { "Content-Type": "application/json" }, body: JSON.stringify({ password }) });
        if (!r.ok) throw new Error(r.status === 429 ? "尝试过于频繁，请稍后再试。" : "访问密码错误或服务暂不可用。");
        setPassword(""); setAuthenticated(true);
      } catch (e) { setError(e instanceof Error ? e.message : "连接失败"); }
      finally { setBusy(false); }
    }}>
      <h1 className="text-xl font-bold">HGhub 个人网页版</h1>
      <p className="text-sm text-mut">请输入部署时设置的访问密码。这不是上游网站的账号密码。</p>
      <label className="block text-sm">访问密码
        <input className="mt-2 w-full rounded-lg bg-s3 p-3" type="password" autoComplete="current-password"
          required maxLength={1024} value={password} onChange={e => setPassword(e.target.value)} />
      </label>
      {error && <p role="alert" className="text-sm text-danger">{error}</p>}
      <button className="w-full rounded-lg bg-primary text-on-primary p-3 disabled:opacity-50" disabled={busy}>{busy ? "验证中…" : "进入"}</button>
      <p className="text-xs text-mut">仅限个人、非商业使用。公网访问请先配置 HTTPS。</p>
    </form>
  </main>;
}
