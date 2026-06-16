# RetroSync

**Seus jogos, do ponto exato onde você parou — em qualquer máquina.**

O RetroSync é um aplicativo para computador que guarda automaticamente seus **saves,
savestates e configurações** de emuladores de retrogames no **Google Drive**. Você joga
no PC de casa, depois abre o mesmo jogo no notebook e continua de onde tinha parado — sem
copiar arquivos na mão, sem pendrive, sem se preocupar em perder progresso.

## A ideia

Quem joga emulador conhece a dor: o save do jogo fica preso numa pasta de uma máquina só.
Trocou de computador, formatou, ou só quer jogar no notebook no fim de semana? Lá se vai a
sincronia — ou começa a bagunça de copiar pastas para um pendrive e torcer para não
sobrescrever a versão certa.

O RetroSync resolve isso rodando discretamente em segundo plano e mantendo tudo guardado e
atualizado na sua conta do Google Drive, automaticamente.

## Como é usar

1. **Conecte sua conta Google.** Um clique, e pronto — o app só acessa o que ele mesmo cria
   no seu Drive, nada mais.
2. **Aponte a pasta do seu emulador.** O RetroSync reconhece sozinho qual emulador é.
3. **Esqueça que ele existe.** A partir daí tudo acontece sozinho. O app fica na bandeja do
   sistema, ao lado do relógio, e sincroniza nos momentos certos.

## O que ele faz por você

- **Sincroniza sozinho, na hora certa.** Quando você abre o emulador, ele baixa os saves
  mais recentes antes do jogo começar. Quando você fecha, envia o progresso novo para o
  Drive. Também sincroniza ao abrir o app e ao sair de vez.

- **Nunca apaga nada.** O RetroSync só adiciona e atualiza — seus arquivos no Drive estão
  seguros. Se duas máquinas mexeram no mesmo save, ele te dá o poder de decisão de qual save manter.

- **Funciona offline.** Sem internet ou com o jogo aberto na hora errada? Ele anota a
  pendência e sincroniza assim que der, sem dar erro nem atrapalhar.

- **Você escolhe o que sincronizar.** Dá para ligar ou desligar saves, savestates e
  configurações para cada emulador, individualmente — e também escolher quais momentos
  disparam a sincronização automática.

- **Avisa sem incomodar.** Notificações nativas mostram quando algo foi sincronizado ou deu
  problema — e você ajusta para receber todas, só os erros, ou nenhuma.

- **Combina com várias máquinas.** Cada computador ganha um nome, então você sempre sabe de
  onde veio cada save.

## Emuladores suportados

- **PPSSPP** (PlayStation Portable)
- **PCSX2** (PlayStation 2)

A estrutura foi pensada para crescer — novos emuladores podem ser adicionados sem mudar o
funcionamento do app.

## Seus dados e sua privacidade

- O RetroSync usa o acesso mínimo ao Google Drive: ele **só enxerga os arquivos que ele
  próprio cria**. O resto do seu Drive permanece invisível para o app.
- Tudo que ele guarda no Drive fica organizado numa pasta dedicada: `RetroSync`, com uma
  subpasta para cada emulador.

## Por dentro

O RetroSync é um app desktop construído com **Tauri**, com a lógica em **Rust** e a
interface em **React**. É leve, roda em segundo plano e foi feito para ser confiável: não
destrói arquivos, lida bem com falhas de rede e mantém tudo organizado.

A documentação técnica completa — arquitetura, decisões de projeto e instruções de
instalação e build — está na pasta [`docs/`](./docs/).
</content>
