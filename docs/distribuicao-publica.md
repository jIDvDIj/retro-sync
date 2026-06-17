# Distribuição Pública e Confiança do Usuário

Documento que consolida a análise e as decisões sobre como tornar o RetroSync confiável
e acessível para o público em geral, cobrindo desde a verificação OAuth do Google até a
estratégia de distribuição escolhida. Não descreve um passo de implementação concluído —
é um **registro de decisões e pesquisa** para guiar o trabalho futuro.

---

## Contexto

O RetroSync é um app desktop (Tauri v2) distribuído para usuários que não têm relação
técnica com o projeto. Para esses usuários, confiança se traduz em:

1. A tela de login do Google não exibir aviso vermelho de "app não verificado".
2. O instalador Windows não ser bloqueado pelo SmartScreen.
3. A origem do binário ser rastreável (integridade).
4. Existir uma página pública com política de privacidade e canal de contato.

O repositório é **privado** e o app é **software proprietário** (não open source). Esse
fato elimina diversas opções gratuitas e é a principal restrição da análise abaixo.

---

## Tópico 1 — Verificação OAuth do Google

### O problema

Apps que usam OAuth do Google passam por um processo de verificação. Enquanto não
verificados, a tela de consentimento exibe:

> **"O Google não verificou este app"** — aviso em vermelho que exige clique extra do
> usuário para continuar.

Para o RetroSync, que usa o escopo `drive.file` (não-sensível), o processo de
verificação é simplificado e **gratuito**. O escopo sensível (`drive`) exigiria auditoria
de segurança paga (CASA Tier 2).

### Pré-requisitos para verificação

| Requisito | Status |
|---|---|
| Domínio próprio verificado no Google Search Console | Adquirido — em andamento |
| Página `https://<domínio>/privacy` com política de privacidade | Pendente |
| Homepage pública do app (`https://<domínio>`) | Pendente |
| Logotipo do app (mínimo 120×120px) | Pendente |
| Escopo `drive.file` (não-sensível) | ✅ Já implementado |

### O que a verificação resolve

- Elimina o aviso vermelho na tela de consentimento OAuth.
- Aumenta a percepção de legitimidade do app.
- Não tem custo monetário para escopos não-sensíveis.

### O que não resolve

- O aviso do SmartScreen no Windows (assunto separado).
- Integridade do binário baixado.

### Recomendação

É o item de maior impacto por menor esforço. Deve ser feito imediatamente após o domínio
estar configurado e a página de política de privacidade estar no ar.

---

## Tópico 2 — SmartScreen do Windows

### O problema

O Windows Defender SmartScreen exibe um aviso ao instalar executáveis de "editores
desconhecidos":

> **"O Windows protegeu seu PC"** — botão azul de "Mais informações" antes de prosseguir.

O aviso aparece para qualquer `.exe` ou `.msi` sem assinatura de código reconhecida ou
com pouca reputação acumulada.

### Como o SmartScreen decide

O SmartScreen opera por **reputação**: quanto mais usuários baixam e executam o app sem
reclamar, menor a frequência do aviso. É um processo gradual — pode levar semanas ou
meses com volume baixo de downloads.

### Opções analisadas

#### Certificado de assinatura de código (OV — Organization Validation)

- **Custo**: ~$70–$200/ano.
- **Efeito**: muda o aviso de "editor desconhecido" para o nome do publicador. **Não
  elimina** o SmartScreen — apenas ajuda a construir reputação mais rápido.
- **Observação 2026**: a partir de fevereiro de 2026, certificados EV (Extended
  Validation) também **não bypassam mais** o SmartScreen automaticamente. A distinção
  OV/EV perdeu relevância para esse efeito.

#### MSIX assinado com certificado autoassinado

- **Custo**: gratuito.
- **Efeito**: o aviso muda de tom mas não some. Para instalar, o usuário precisa importar
  manualmente o certificado como "Editor Confiável" — inviável para público leigo.

#### SignPath Foundation (assinatura gratuita)

- **Custo**: gratuito.
- **Requisito**: projeto deve ser **open source** com licença OSI aprovada.
- **Status**: **não aplicável** — o RetroSync é software proprietário.

#### Distribuição pela Microsoft Store

- **Efeito**: apps aprovados na Store **não disparam o SmartScreen**. A Microsoft assina
  por conta própria durante o processo de publicação.
- **Custo**: gratuito desde 2025 (veja Tópico 4).
- **Complexidade**: requer pacote MSIX, que o Tauri v2 não gera nativamente.
- **Status**: **opção principal investigada** (veja Tópico 4).

#### Construção de reputação orgânica

- **Custo**: gratuito.
- **Efeito**: com volume crescente de downloads sem reclamações, o SmartScreen passa a
  confiar no executável. Funciona mesmo sem assinatura.
- **Prazo**: incerto — semanas a meses dependendo do volume.
- **Uso prático**: documentar na página de download como clicar em
  "Mais informações → Executar assim mesmo" enquanto a reputação não está consolidada.

### Conclusão sobre SmartScreen

Não existe solução **gratuita e imediata** para eliminar o aviso em software proprietário
distribuído fora da Store. As estratégias se complementam:

1. **Curto prazo**: instruir o usuário na página de download + construção orgânica de
   reputação.
2. **Médio prazo**: publicação na Microsoft Store (elimina o aviso permanentemente).
3. **Longo prazo opcional**: certificado OV pago para acelerar reputação fora da Store.

---

## Tópico 3 — GitHub Attestations (investigado e descartado)

### O que é

`actions/attest-build-provenance` gera uma **atestação SLSA** (Supply-chain Levels for
Software Artifacts) assinada via OIDC e registrada em log público imutável do GitHub.
Permite ao usuário verificar criptograficamente que um binário saiu de um determinado
workflow:

```bash
gh attestation verify RetroSync_x64-setup.exe --repo <owner>/retro-sync
```

### Por que foi investigado

Parecia ser uma opção gratuita de prova de integridade/origem, independente de
assinatura de código paga.

### Por que foi descartado

**Incompatível com repositório privado** por dois motivos:

1. **Verificação exige acesso ao repositório.** O comando `gh attestation verify` precisa
   que o verificador esteja autenticado e tenha permissão de leitura no repo. Um usuário
   final comum não tem esse acesso — o valor de "prova pública" se perde inteiramente.

2. **Geração pode falhar em repo privado.** Em conta pessoal/Pro com repo privado, o step
   `attest-build-provenance` pode não funcionar e quebrar a release. O recurso completo
   geralmente requer GitHub Enterprise.


### Alternativa viável: Cosign / Sigstore

Para prova criptográfica de origem em repo privado, existe o **Cosign** (Sigstore):

- A assinatura vai para o log público **Rekor**, fora do GitHub.
- Qualquer pessoa verifica com `cosign verify-blob` **sem acesso ao repo**.
- **Trade-off**: o certificado OIDC expõe o nome do repositório e do workflow no log
  público — não expõe o código, mas revela que o repo existe. Aceitável se o nome do repo
  pode ser público; inaceitável caso contrário.

---

## Tópico 4 — Microsoft Store (opção escolhida para investigação aprofundada)

### Contexto da escolha

A Microsoft Store é a única opção **gratuita** que elimina o aviso do SmartScreen de
forma definitiva para software proprietário em Windows. Apps aprovados na Store são
assinados pela Microsoft e carregam sua reputação institucional.

### Mudança de custo em 2025–2026

Até meados de 2025, publicar na Store exigia pagamento de taxa de registro:
- Conta individual: $19 (valor aproximado por região)
- Conta empresa: $99

Em **setembro de 2025**, a Microsoft removeu a taxa para desenvolvedores individuais.
Em **maio de 2026**, removeu também para contas empresa. **Hoje, publicar na Store é
completamente gratuito** para qualquer tipo de conta.

### Formato exigido: MSIX

A Store aceita exclusivamente pacotes **MSIX** (o formato moderno de empacotamento
Windows). O Tauri v2, porém, gera apenas `.exe` (NSIS) e `.msi` (WiX) — **MSIX não
é gerado nativamente**.

### Ferramenta comunitária: `tauri-windows-bundle`

O repositório [`Choochmeque/tauri-windows-bundle`](https://github.com/Choochmeque/tauri-windows-bundle)
é uma CLI npm que converte o output do Tauri em pacote MSIX:

**O que faz:**
- Lê `tauri.conf.json` e gera `AppxManifest.xml`, `bundle.config.json` e assets de ícone
  redimensionados.
- Produz `.msix` por arquitetura e um `.msixbundle` multi-arch em
  `src-tauri/target/msix/`.
- Adiciona automaticamente a capability `runFullTrust` — necessária para apps desktop
  nativos fora do sandbox UWP.
- Converte a versão de 3-part (`1.2.3`) para 4-part (`1.2.3.0`) conforme exigido pelo
  manifesto MSIX.

**Pré-requisitos do Partner Center:**
Três valores que devem bater **exatamente** com o cadastro no Partner Center:

| Campo | Exemplo | Onde vem |
|---|---|---|
| `publisher` | `CN=David Silva, O=..., C=BR` | Partner Center → Gerenciar conta → Identidade |
| `publisherDisplayName` | `David Silva` | Partner Center |
| `identityName` | `RetroSync` | Reservado ao criar o app na Store |

No CI, esses valores devem ser secrets (`MSIX_PUBLISHER`, `MSIX_PUBLISHER_DISPLAY_NAME`,
`MSIX_IDENTITY_NAME`).

### Risco principal: acesso ao filesystem dos emuladores

Este é o **ponto crítico** para o RetroSync especificamente.

Apps MSIX vivem em um **contêiner virtual de filesystem**. O `runFullTrust` permite mais
acesso do que apps UWP comuns, mas o acesso a pastas de **outros aplicativos** (como
`%APPDATA%\PPSSPP`, `%APPDATA%\PCSX2`) pode exigir a capability restrita
**`broadFileSystemAccess`**.

| Cenário | Comportamento esperado |
|---|---|
| Ler/escrever arquivos próprios do RetroSync | Permitido com `runFullTrust` |
| Ler `%APPDATA%\PPSSPP\` | **Incerto** — pode ser bloqueado sem `broadFileSystemAccess` |
| Ler `%USERPROFILE%\Documents\PCSX2\` | **Incerto** — mesmo risco |

A capability `broadFileSystemAccess` é **restrita**: exige aprovação manual da Microsoft,
documentação de justificativa e pode atrasar ou até inviabilizar a publicação.

**Este ponto deve ser validado antes de qualquer investimento no pipeline de CI.**

### Compatibilidade com Tauri v2

O `tauri-windows-bundle` não declara suporte explícito ao Tauri v2. A ferramenta menciona
"Tauri apps" genericamente. O Tauri v2 mudou a estrutura de bundle (novo sistema de
capabilities, `tauri.conf.json` com formato atualizado) — há **risco real de
incompatibilidade** que só a execução local pode confirmar.

### Limitação de distribuição: repo privado ≠ releases públicas

Com repositório privado no GitHub, os assets do GitHub Releases **não são
publicamente baixáveis**. O instalador precisará ser hospedado no domínio próprio ou
distribuído exclusivamente pela Store. A Store resolve isso de forma completa — o usuário
baixa diretamente do catálogo da Microsoft.

### Processo de publicação na Store (passos gerais)

1. Criar conta gratuita no [Partner Center](https://partner.microsoft.com/dashboard).
2. Reservar o nome "RetroSync" (pode ser feito até 3 meses antes da publicação).
3. **Validar localmente** (ver seção abaixo) antes de investir no CI.
4. Integrar geração de MSIX no `release.yml` (job dedicado, só plataforma Windows).
5. Submeter o `.msixbundle` pela interface do Partner Center ou via `StoreBroker` (CLI).
6. Aguardar revisão da Microsoft (tipicamente 1–3 dias úteis para versões novas).
7. A Microsoft assina o pacote — SmartScreen passa a confiar automaticamente.

### Validação local recomendada (antes do CI)

**Passo 1 — Testar compatibilidade com Tauri v2:**

No Windows (PowerShell), dentro do repositório:

```powershell
npx @choochmeque/tauri-windows-bundle init
npm run tauri:windows:build
```

Verificar se o `.msixbundle` é gerado em `src-tauri/target/msix/`.

**Passo 2 — Testar acesso ao filesystem dos emuladores:**

Instalar o `.msix` gerado (sem submeter à Store — só localmente para teste) e verificar
se o RetroSync consegue:

```
- Ler %APPDATA%\PPSSPP\PSP\SAVEDATA\
- Ler %APPDATA%\PCSX2\memcards\
```

Se qualquer um falhar com erro de acesso negado, será necessário solicitar
`broadFileSystemAccess` — o que muda o processo de aprovação.

### Proposta de integração no `release.yml` (após validação)

Job separado, dependente do `release` e do `version`, rodando apenas em Windows:

```yaml
msix:
  needs: [version, release]
  runs-on: windows-latest
  permissions:
    contents: write
  steps:
    - uses: actions/checkout@v4

    - uses: actions/setup-node@v4
      with:
        node-version: lts/*
        cache: npm

    - uses: dtolnay/rust-toolchain@stable
      with:
        targets: x86_64-pc-windows-msvc,aarch64-pc-windows-msvc

    - uses: swatinem/rust-cache@v2
      with:
        workspaces: ./src-tauri -> target

    - name: Install dependencies
      run: npm ci

    # Injeta identidade MSIX do Partner Center
    - name: Configurar identidade MSIX
      shell: bash
      run: |
        jq \
          --arg pub "${{ secrets.MSIX_PUBLISHER }}" \
          --arg pubName "${{ secrets.MSIX_PUBLISHER_DISPLAY_NAME }}" \
          --arg id "${{ secrets.MSIX_IDENTITY_NAME }}" \
          '.publisher = $pub | .publisherDisplayName = $pubName | .identityName = $id' \
          src-tauri/gen/windows/bundle.config.json > tmp.json
        mv tmp.json src-tauri/gen/windows/bundle.config.json

    - name: Definir versão do app
      shell: bash
      run: |
        v="${{ needs.version.outputs.version }}"
        jq --arg v "$v" '.version = $v' src-tauri/tauri.conf.json > tauri.conf.tmp
        mv tauri.conf.tmp src-tauri/tauri.conf.json

    - name: Build MSIX
      env:
        RETROSYNC_GOOGLE_CLIENT_ID: ${{ secrets.RETROSYNC_GOOGLE_CLIENT_ID }}
        RETROSYNC_TOKEN_PROXY_URL: ${{ secrets.RETROSYNC_TOKEN_PROXY_URL }}
        RETROSYNC_PROXY_SECRET: ${{ secrets.RETROSYNC_PROXY_SECRET }}
      run: npx tauri:windows:build --arch x64,arm64 --runner npm

    - name: Upload msixbundle para o Release
      uses: softprops/action-gh-release@v2
      with:
        tag_name: ${{ needs.version.outputs.tag }}
        files: src-tauri/target/msix/*.msixbundle
```

Novos secrets necessários no repositório:

| Secret | Descrição |
|---|---|
| `MSIX_PUBLISHER` | String `CN=...` exata do Partner Center |
| `MSIX_PUBLISHER_DISPLAY_NAME` | Nome exibido na Store |
| `MSIX_IDENTITY_NAME` | Nome do pacote reservado na Store |

---

## Resumo geral das estratégias

| Estratégia | Custo | Resolve SmartScreen | Resolve OAuth | Compatível com repo privado | Status |
|---|---|---|---|---|---|
| Verificação OAuth Google | Gratuito | Não | ✅ Sim | ✅ Sim | **Fazer imediatamente** |
| Política de privacidade + domínio | Gratuito | Não | Pré-requisito | ✅ Sim | **Fazer imediatamente** |
| Reputação orgânica (instrução na página) | Gratuito | Parcialmente (com tempo) | Não | ✅ Sim | **Curto prazo** |
| GitHub Attestations | Gratuito | Não | Não | ❌ Não funciona | **Descartado** |
| Cosign / Sigstore | Gratuito | Não | Não | ⚠️ Expõe nome do repo | Opcional futuro |
| Certificado OV pago | $70–$200/ano | Parcialmente | Não | ✅ Sim | Opcional futuro |
| Microsoft Store | Gratuito | ✅ Sim (definitivo) | Não | ✅ Sim | **Investigar — ver abaixo** |

---

## Resumo detalhado: Microsoft Store

### Por que é a escolha principal

A Microsoft Store é a **única opção gratuita** que elimina o aviso do SmartScreen de
forma definitiva para software proprietário no Windows. Ela resolve o problema pela raiz:
a Microsoft assina o pacote e empresta sua reputação institucional ao instalador —
o SmartScreen não questiona apps vindos da Store.

Para o contexto do RetroSync (repo privado, software proprietário, sem orçamento para
certificado EV), todas as outras opções são paliativas: instruir o usuário a clicar em
"Mais informações", esperar reputação orgânica crescer, ou pagar por um certificado que
ainda não elimina o aviso (apenas acelera a reputação). A Store é a solução estrutural.

### Por que ainda não está implementada

Dois bloqueadores técnicos precisam ser validados antes de investir no pipeline:

**Bloqueador 1 — Compatibilidade do `tauri-windows-bundle` com Tauri v2.**
A ferramenta não declara suporte explícito à v2. O Tauri v2 mudou o formato de
`tauri.conf.json` e o sistema de capabilities. Há risco de a ferramenta gerar um
manifesto incorreto ou falhar silenciosamente. Validação: rodar `init` + build localmente
no Windows e verificar se o `.msixbundle` é gerado corretamente.

**Bloqueador 2 — Acesso ao filesystem dos emuladores dentro do contêiner MSIX.**
O RetroSync lê e escreve em pastas de outros aplicativos (`%APPDATA%\PPSSPP`,
`%APPDATA%\PCSX2`, etc.). Apps MSIX com `runFullTrust` têm mais liberdade que apps UWP,
mas o acesso a pastas de terceiros é incerto sem `broadFileSystemAccess`. Essa capability
é **restrita pela Microsoft** — requer justificativa explícita no processo de submissão e
aprovação manual, o que pode atrasar a publicação por dias ou semanas (e pode ser negada).

Se o `runFullTrust` **sozinho** permitir o acesso necessário, a integração é direta e o
processo de aprovação na Store segue o fluxo padrão (1–3 dias). Se não, a capability
`broadFileSystemAccess` entra em jogo e o prazo e a certeza da aprovação se tornam
imprevisíveis.

### Próximos passos concretos

1. **Validar localmente no Windows** (30 min a 1h):
   - `npx @choochmeque/tauri-windows-bundle init` — ver se gera os arquivos com Tauri v2.
   - `npm run tauri:windows:build` — ver se o `.msixbundle` é produzido.
   - Instalar localmente e testar leitura de `%APPDATA%\PPSSPP`.

2. **Se ambos passarem** (~1-2h de CI):
   - Criar conta no Partner Center (gratuito).
   - Reservar o nome "RetroSync".
   - Integrar o job `msix` no `release.yml` conforme proposta acima.
   - Adicionar 3 novos secrets no repositório.
   - Submeter primeira versão para revisão.

3. **Se o filesystem falhar**:
   - Avaliar se `broadFileSystemAccess` é viável (escrever justificativa para a Microsoft).
   - Alternativa: permitir que o usuário configure manualmente os caminhos dos emuladores
     (já existe na UI) e usar apenas caminhos explicitamente fornecidos — pode reduzir o
     escopo de acesso necessário e facilitar a aprovação.

### O que a Store não resolve

- **Aviso OAuth do Google**: a verificação OAuth é independente e deve ser feita de
  qualquer forma (veja Tópico 1).
- **macOS e Linux**: a Store é exclusivamente Windows. Para macOS, o equivalente seria a
  Mac App Store (requer Apple Developer, $99/ano) ou notarização do binário (também paga).
  Para Linux, não há aviso equivalente ao SmartScreen.
- **Integridade criptográfica verificável pelo usuário**: a Store não expõe checksums
  para verificação manual. Para isso, publicar SHA-256 na página de download é
  complementar e gratuito.
