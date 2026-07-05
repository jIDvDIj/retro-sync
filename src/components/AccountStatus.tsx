import { useTranslation } from "react-i18next";

import { Badge } from "./ui/Badge";
import { Button } from "./ui/Button";

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
        {deviceName ? <Badge tone="brand">{deviceName}</Badge> : null}
        <Button variant="secondary" size="sm" onClick={onDisconnect}>
          {t("account.disconnect")}
        </Button>
      </div>
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
