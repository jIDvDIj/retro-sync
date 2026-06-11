import { useEffect, useState } from "react";

import { healthCheck } from "./lib/ipc";
import "./App.css";

function App() {
  const [backendStatus, setBackendStatus] = useState("conectando ao backend…");

  useEffect(() => {
    healthCheck()
      .then((health) => setBackendStatus(`backend pronto (v${health.version})`))
      .catch(() => setBackendStatus("backend indisponível"));
  }, []);

  return (
    <main className="container">
      <h1>RetroSync</h1>
      <p className="status">{backendStatus}</p>
    </main>
  );
}

export default App;
