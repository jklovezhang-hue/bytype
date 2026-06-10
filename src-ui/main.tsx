import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Wizard from "./wizard/Wizard";
import { wizardState } from "./wizard/api";
import type { WizardState } from "./wizard/types";
import "./index.css";
import { initTheme } from "./settings/theme";

initTheme();

function Root() {
  const [st, setSt] = useState<WizardState | null>(null);
  useEffect(() => {
    // 出错也不卡死:当作已就绪,进设置界面(用户至少能看/改配置)。
    wizardState()
      .then(setSt)
      .catch(() => setSt({ ready: true, config_exists: true, model_present: true, model_dir: "" }));
  }, []);

  if (st === null) {
    return (
      <div className="h-screen flex items-center justify-center text-neutral-400 text-sm dark:bg-neutral-900">
        启动中…
      </div>
    );
  }
  return st.ready ? <App /> : <Wizard initial={st} />;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);
