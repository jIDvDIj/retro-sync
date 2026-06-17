# BUG-002 — Perda silenciosa de save em edição simultânea entre dispositivos

**Status:** ✅ resolvido (v1.1 · Passo 7 — ver [14-resolucao-conflito.md](../14-resolucao-conflito.md))  
**Severidade:** crítica (perda silenciosa de dados)  
**Componente:** `src-tauri/src/sync/conflict.rs`, `src-tauri/src/sync/engine.rs`

> **Resolução:** adotou-se a **Opção 2** (notificar e aguardar decisão do usuário), viabilizada
> pelo identificador de dispositivo do Passo 1. Quando ambos os lados mudaram desde o último sync,
> `decide` retorna `SyncAction::Conflict`: o engine registra o conflito em `sync_conflicts`, emite
> `sync:conflict`, notifica e **bloqueia** o sync do emulador até o usuário escolher no modal
> (mantendo local ou Drive). Ao manter o Drive, o arquivo local preterido vai para backup antes de
> ser sobrescrito; ao manter o local, a versão do Drive é apenas sobrescrita (sem backup do
> RetroSync — o histórico de revisões do Drive é a rede de segurança). Combina o não-destrutivo da
> Opção 3 com a decisão correta da Opção 2. A Opção 1 (sufixo de conflito) foi preterida em favor
> da decisão explícita na UI.

---

## Descrição

Quando dois dispositivos editam o mesmo arquivo de save entre syncs — incluindo o caso em
que um deles estava offline — o sync resolve o conflito silenciosamente por mtime. O
dispositivo que perde a disputa não recebe nenhum aviso, e o save sobrescrito é
irrecuperável na implementação atual.

## Reprodução

1. **Dispositivo A** tem save de Monster Hunter com 100h, sincronizado ao Drive.
   Manifest A: `(local=T₁, drive=T₁)`.

2. **Dispositivo B** baixa o save do Drive. Manifest B: `(local=T₁, drive=T₁)`.

3. Dispositivo B fica **offline** e joga. Save local: `mtime=T₂`. Sync falha → operação
   vai para `pending_ops` (Upload).

4. **Dispositivo A** continua jogando e sincroniza normalmente. Save local: `mtime=T₃`.
   Drive: `T₃`. Manifest A: `(T₃, T₃)`.

5. **Dispositivo B fica online.** Startup sync dispara (Bidirecional). `decide()` recebe:
   - `local=T₂`, `drive=T₃`, `last_synced=(T₁, T₁)`
   - Ambos mudaram desde o último sync → **mtime decide.**

### Caso 1 — B jogou mais recentemente (T₂ > T₃)

- B faz **Upload** → Drive vira `T₂`. Manifest B: `(T₂, T₂)`.
- Próximo sync do Dispositivo A: `local=T₃`, `drive=T₂`, `last_synced=(T₃, T₃)`.
  Drive mudou, local não → **Download**. A sobrescreve seu save `T₃` com `T₂` de B,
  sem aviso.

### Caso 2 — A jogou mais recentemente (T₃ > T₂)

- B faz **Download** → save local de B (`T₂`) é sobrescrito pelo de A (`T₃`).
  Pendência de Upload na fila é resolvida pelo `resolve()`. Dispositivo A não é afetado.

## Causa raiz

`decide()` em `conflict.rs` lida corretamente com o "conflito real" (ambos os lados
mudaram desde o último sync), mas resolve por mtime bruto sem notificar nenhum dos
dispositivos:

```rust
// conflict.rs:38-44 — caminho de conflito real
} else if local > drive {
    SyncAction::Upload   // lado local vence, silenciosamente
} else {
    SyncAction::Download // lado Drive vence, silenciosamente
}
```

A fila offline (`pending_ops`) não interfere na decisão — ela apenas registra a intenção
anterior. O diff do próximo sync re-avalia o estado atual e pode descartar a intenção
enfileirada se a resolução por mtime favorecer o outro lado.

## Impacto

- Qualquer dispositivo pode perder progresso sem aviso em qualquer sync após um período
  offline ou de uso paralelo.
- O Caso 1 é especialmente grave: Dispositivo A perde seu save no próximo sync **sem
  ter feito nada errado** — ele estava online e sincronizado.
- Afeta todos os emuladores e categorias (saves, savestates, config).

---

## Possibilidades de resolução

### Opção 1 — Manter as duas versões (conflito explícito com sufixo)

Quando `decide()` detectar conflito real (ambos mudaram desde `last_synced`), em vez de
escolher, renomear o arquivo local para `save.bin.conflito-<DATA>` e baixar a versão do
Drive normalmente.

O usuário vê os dois arquivos diretamente no emulador e escolhe qual carregar — o contexto
certo para a decisão (ele sabe qual sessão de jogo quer manter).

**Prós:** não-destrutivo, sem UI nova no RetroSync, implementação localizada em `engine.rs`.  
**Contras:** acumula arquivos de conflito se o usuário ignorar; nomes com sufixo podem
confundir emuladores que varrem a pasta de saves.

**Arquivos a modificar:**
| Arquivo | Mudança |
|---|---|
| `src-tauri/src/sync/conflict.rs` | Novo valor `SyncAction::Conflict` |
| `src-tauri/src/sync/engine.rs` | Antes do download, renomear o arquivo local existente |

---

### Opção 2 — Notificar o usuário e aguardar decisão

Ao detectar conflito real, emitir evento `sync:conflict` ao frontend com metadados dos
dois lados (tamanho, data, dispositivo de origem). O sync pausa até o usuário escolher
qual versão manter via UI.

**Prós:** decisão sempre correta; o usuário tem contexto para escolher.  
**Contras:** requer tela nova no frontend; bloqueia o sync (impacto nos gatilhos
automáticos como `emulator-start`); exige identificador de dispositivo que hoje não existe.

**Arquivos a modificar:**
| Arquivo | Mudança |
|---|---|
| `src-tauri/src/sync/conflict.rs` | `SyncAction::Conflict` |
| `src-tauri/src/sync/engine.rs` | Emitir evento e aguardar resposta via canal |
| `src-tauri/src/events.rs` | Novo evento `sync:conflict` |
| `src/types/ipc.ts` | Espelho do payload do evento |
| `src/` (UI) | Tela/modal de resolução de conflito |

---

### Opção 3 — Backup automático antes de sobrescrever (primeiro sync)

Quando `last_synced = None` e o arquivo existe nos dois lados, antes de executar o
`Download`, copiar o arquivo local para `RetroSync/Backups/<emulador>/<data>/`. O usuário
pode recuperar manualmente pelo Drive ou pelo sistema de arquivos.

O backup é restrito ao primeiro sync para aquele arquivo — syncs subsequentes não fazem
backup, pois o usuário já tinha ciência do estado sincronizado anteriormente. Não evita a
sobrescrita, mas elimina a perda irreversível nesse cenário específico.

**Prós:** mudança mínima, sem UI nova, zero risco de regressão; escopo limitado evita
acúmulo de backups desnecessários.  
**Contras:** não resolve o problema — apenas mitiga. Não cobre conflitos em syncs
subsequentes (cobertos pela Opção 1).

**Arquivos a modificar:**
| Arquivo | Mudança |
|---|---|
| `src-tauri/src/sync/engine.rs` | Copiar arquivo local para pasta de backup antes do download destrutivo |
| `src-tauri/src/constants.rs` | Constante para o nome da pasta de backup |

---

### Opção 4 — Histórico de versões no Google Drive

Ao fazer Upload em cima de um arquivo existente, usar a API do Drive para criar uma nova
**revisão** em vez de sobrescrever. O usuário pode restaurar versões antigas diretamente
pelo Google Drive.

**Prós:** histórico completo sem lógica de backup local; transparente para o usuário.  
**Contras:** complexidade adicional na `DriveClient`; consome cota de armazenamento;
revisões antigas expiram nas contas gratuitas; não resolve o problema no lado local.

**Arquivos a modificar:**
| Arquivo | Mudança |
|---|---|
| `src-tauri/src/drive/` | Novo método de upload com revisão (`keepRevisionForever`) |
| `src-tauri/src/sync/engine.rs` | Usar o novo método ao detectar sobrescrita |

---

### Opção 5 — Merge por formato de emulador (descartada)

Detectar qual slot de save mudou em cada dispositivo e mesclar os slots individualmente.
Dependeria do formato binário interno de cada emulador (PPSSPP, PCSX2), quebraria o
princípio de núcleo agnóstico ao emulador e seria frágil a atualizações dos emuladores.

**Conclusão:** complexidade muito alta, risco elevado de corrupção de save. Descartada
para qualquer versão próxima.

---

## Comparação

| Opção | Complexidade | Perda de dados | UI nova | Recomendada |
|---|---|---|---|---|
| 1 — Duas versões | baixa | nenhuma | não | sim (base) |
| 2 — Usuário escolhe | média | nenhuma | sim | futura |
| 3 — Backup automático | baixa | nenhuma (recuperável) | não | sim (complemento) |
| 4 — Histórico Drive | alta | nenhuma | não | avaliar |
| 5 — Merge por formato | muito alta | risco alto | não | descartada |

**Recomendação para v1.0:** combinar **Opção 1 + Opção 3** — sufixo de conflito preserva
ambas as versões localmente; backup restrito ao primeiro sync (`last_synced = None`) garante
recuperação sem acumular arquivos desnecessários em syncs posteriores.
Sem UI nova, não-destrutivo, implementação contida em `engine.rs` e `constants.rs`.

## Relação com outros bugs

- Ver também [BUG-001](./bug-001-perda-save-primeiro-sync.md) — perda de save no primeiro
  sync em dispositivo com jogo já instalado (cenário diferente, mesma raiz: ausência de
  estratégia não-destrutiva na resolução de conflito).
