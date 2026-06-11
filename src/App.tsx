import { useEffect, useState } from "react";

import { ConnectDrive } from "./components/ConnectDrive";
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
      <ConnectDrive />
      <footer className="status">{backendStatus}</footer>
    </main>
  );
}

export default App;
