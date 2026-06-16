# FEATURE-002 — Tela de configurações

---

## Passo 1 — Tela de login

A tela de login exibe uma mensagem informando que o RetroSync não tem acesso aos dados
pessoais do usuário — apenas pode ver e modificar os arquivos que ele mesmo criou no
Google Drive. Essa mensagem deve ser visível antes de o usuário iniciar o login.

O nome do dispositivo é solicitado na mesma tela, antes de concluir a autenticação. O
campo é obrigatório — o botão de login só é habilitado após o preenchimento
(ex: "PC Gamer", "Notebook").

O nome é gravado junto com os metadados de sync no Drive e exibido na UI como
identificador do dispositivo atual. Assim, quando um conflito ocorrer, o usuário sabe
exatamente de qual dispositivo cada versão do save é proveniente.

---

## Passo 2 — Nome do dispositivo nas configurações

O nome do dispositivo definido no login pode ser alterado na tela de configurações, sem
necessidade de refazer o login.

---

## Passo 3 — Categorias de sync por emulador

Permitir que o usuário escolha quais categorias sincronizar para cada emulador configurado:
saves, savestates e/ou config. Por padrão todas ativas.

O usuário pode, por exemplo, desativar a categoria config para que as configurações do
emulador (resolução, controles) não sejam compartilhadas entre dispositivos diferentes.

---

## Passo 4 — Sync automático por gatilho

Permitir ligar/desligar cada gatilho de sync automático individualmente:

| Gatilho | Descrição | Padrão |
|---|---|---|
| `startup` | Sync ao abrir o RetroSync | ativado |
| `emulator-start` | Download antes de abrir o emulador | ativado |
| `emulator-stop` | Upload ao fechar o emulador | ativado |

Usuários que preferem controle manual podem desativar todos os gatilhos automáticos sem
perder o botão de sync manual.

---

## Passo 5 — Nível de notificações nativas

Controlar quais eventos geram notificação nativa do SO:

| Nível | Notifica |
|---|---|
| `all` | Sync concluído, erros, emulador detectado |
| `errors_only` | Apenas erros de sync |
| `none` | Nenhuma notificação |

Syncs automáticos frequentes (ao abrir/fechar emulador) podem gerar notificações
invasivas — esta configuração permite reduzir o ruído.

---

## Passo 6 — Primeiro sync

Quando um arquivo existe tanto no dispositivo quanto no Drive e nunca foi sincronizado
antes, o Drive sempre vence. O arquivo local é salvo automaticamente em uma pasta de
backup antes de ser sobrescrito.

Após o sync, a UI exibe uma sinalização informando que backups foram criados, com um botão
que abre a pasta de backup diretamente no gerenciador de arquivos do SO.

---

## Passo 7 — Resolução de conflito

Quando ambos os lados (local e Drive) foram modificados desde o último sync, o sync
daquele emulador é pausado e o usuário recebe uma notificação nativa do SO informando o
conflito. Na tela principal, o card do emulador afetado exibe um botão "Resolver conflito"
que abre um modal com os detalhes dos dois lados (data, tamanho e nome do dispositivo de
origem). O usuário escolhe qual versão manter e o sync é desbloqueado.

Enquanto houver conflito pendente, o sync daquele emulador fica bloqueado — o botão de
sync manual e os gatilhos automáticos não executam para ele. Emuladores sem conflito
pendente não exibem o botão e funcionam normalmente.
