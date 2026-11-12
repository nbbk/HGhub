import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import WebAccess from "./WebAccess";
import { AppProvider } from "./store";
import { invoke, isWeb } from "./transport";
import "./index.css";
import { initPlaybackLog, installPlaybackPageEvents } from "./playbackDiagnostics";

installPlaybackPageEvents();
void initPlaybackLog();

/* 运行期错误统一落到 debug.log，便于自查 */
function report(kind: string, msg: string) {
  invoke("debug_log", { msg: `[${kind}] ${msg}` }).catch(() => {});
}
window.addEventListener("error", (e) => {
  report("error", `${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`);
});
window.addEventListener("unhandledrejection", (e) => {
  report("reject", String((e.reason && (e.reason.stack || e.reason.message)) || e.reason));
});

const application = <AppProvider><App /></AppProvider>;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isWeb ? <WebAccess>{application}</WebAccess> : application}
  </React.StrictMode>,
);
