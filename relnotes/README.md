# Release notes manuais

Destaques escritos à mão para cada versão, combinados automaticamente com as notas
geradas pela API do GitHub durante a release (`.github/workflows/release.yml`).

## Como usar

1. Antes de mergear na `main` o PR que vai gerar a versão `X.Y.*`, crie aqui um arquivo
   `vX.Y.md` (só `major.minor` — patches reaproveitam o mesmo arquivo).
2. Escreva em Markdown os destaques da versão: contexto humano, mudanças de comportamento,
   instruções de migração — o que a lista automática de PRs/commits não conta.
3. O workflow de release monta o corpo da release nesta ordem:
   1. conteúdo de `relnotes/vX.Y.md` (se existir);
   2. notas geradas pela API do GitHub (lista de PRs e contribuidores) — com fallback
      para o changelog de Conventional Commits se a API falhar.

O arquivo é opcional: sem ele, a release sai só com as notas automáticas.

## Exemplo (`v0.2.md`)

```markdown
## Destaques

Suporte inicial ao Android: o app agora roda no celular e sincroniza saves
via Storage Access Framework, com detecção automática de emulador na pasta
concedida.
```
