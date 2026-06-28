import { useTranslation } from "react-i18next";

interface Props {
  email: string | null;
  deviceName: string | null;
  onDisconnect: () => void;
  error: string | null;
}

/**
 * Indicador de conta conectada no cabeçalho da tela principal. Desconectar aqui
 * leva o usuário de volta à tela de login.
 */
export function AccountStatus({ email, deviceName, onDisconnect, error }: Props) {
  const { t } = useTranslation();
  return (
    <div className="connect-drive">
      <div className="connected">
        <span className="account">
          <span className="dot dot-on" />
          {email ?? t("account.connected")}
        </span>
        {deviceName ? <span className="device-tag">{deviceName}</span> : null}
        <button className="secondary" onClick={onDisconnect}>
          {t("account.disconnect")}
        </button>
      </div>
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
