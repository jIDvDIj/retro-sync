import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./i18n";
import "./styles/tokens.css";

// Aplica a preferência salva (ou dark como padrão) antes do primeiro paint,
// para não piscar o tema errado; o toggle em useTheme mantém isso em sincronia.
document.documentElement.dataset.theme =
  localStorage.getItem("rs-theme") === "light" ? "light" : "dark";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
