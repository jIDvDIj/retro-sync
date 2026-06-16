# 09 — Nome do dispositivo nas configurações

> Implementa o **Passo 2** de [FEATURE-002](./features/feature-002-configuracoes-prompt.md).

## O quê

O nome do dispositivo definido no login pode ser **alterado** numa tela de configurações, sem
refazer a autenticação. Um botão "⚙ Configurações" no header (visível quando conectado) abre um
**modal** com a seção "Dispositivo" — campo de nome + "Salvar nome".

## Por quê

O nome inicial é dado no login, mas o usuário pode renomear a máquina ou ter errado na primeira
vez. Exigir reconexão (refazer OAuth) para isso seria atrito desnecessário, já que o nome é só um
rótulo persistido localmente e republicado no Drive no próximo sync.

## Como

Nenhuma mudança de backend: reaproveita `set_device_name` (Passo 1). O trabalho é de frontend e
de **arquitetura de estado**:

- **`hooks/useSettings.ts`** (novo): carrega `getSettings()` no nível do App e expõe `reload()`.
  As settings passam a ser propriedade do App e compartilhadas entre o header (que exibe o
  dispositivo) e o modal (que o edita) — uma única fonte, sempre em sincronia.
- **`components/SettingsModal.tsx`** (novo): modal extensível (crescerá nos passos 3–5). A seção
  "Dispositivo" salva via `setDeviceName` e chama `onSaved` (= `reload` do App), de modo que a
  etiqueta no header reflete a mudança imediatamente.
- **`ConnectDrive.tsx`**: deixou de carregar settings sozinho. Agora recebe `deviceName` por prop
  (do App) e um `onAfterConnect` que dispara o reload após o login. O campo de login pré-preenche
  com o nome salvo sem sobrescrever o que estiver sendo digitado.
- **`App.tsx`**: usa `useSettings`, renderiza o botão de configurações (quando conectado) e o
  `SettingsModal`.

## Arquivos

| Arquivo | Mudança |
| --- | --- |
| `src/hooks/useSettings.ts` | **Novo** — settings no nível do App + `reload` |
| `src/components/SettingsModal.tsx` | **Novo** — modal de configurações (seção Dispositivo) |
| `src/components/ConnectDrive.tsx` | `deviceName`/`onAfterConnect` por prop; não carrega settings |
| `src/App.tsx` | Hook de settings, botão "Configurações", render do modal |
| `src/App.css` | Estilos de `.header-actions`, `.modal*`, `.settings-section` |

## Decisões

- **Settings elevadas ao App** em vez de cada componente carregar as suas: evita estado
  duplicado e o bug de o header mostrar um nome enquanto o modal salva outro. `reload` após cada
  escrita é a forma mais simples de manter a UI coerente (poucos dados, custo irrelevante).
- **Modal único e extensível**: os passos 3, 4 e 5 adicionam seções neste mesmo modal, em vez de
  espalhar telas — uma "Tela de configurações" central, como pede a FEATURE-002.
