#!/usr/bin/env bash
# Notas de uma versão a partir dos commits desde a tag anterior, agrupadas pelo
# tipo do Conventional Commits. Uso: scripts/release-notes.sh v0.2.0
set -euo pipefail

tag=$1
prev=$(git describe --tags --abbrev=0 "$tag^" 2>/dev/null || true)
range=${prev:+$prev..}$tag

# Assuntos do tipo pedido, sem o prefixo `tipo(escopo):`.
section() {
  local title=$1 pattern=$2 lines
  lines=$(git log --format='%s' "$range" \
    | grep -E "^($pattern)(\([^)]*\))?!?: " \
    | sed -E 's/^[a-z]+(\([^)]*\))?!?: /- /' || true)
  if [ -n "$lines" ]; then
    printf '## %s\n\n%s\n\n' "$title" "$lines"
  fi
}

section "Novidades" "feat"
section "Correções" "fix"
section "Desempenho" "perf"

cat <<EOF
## Baixar

- **Windows:** \`padmap-$tag-windows-x64.exe\`. Um arquivo só, sem instalar nada.
  Como não é assinado, o Windows pode mostrar "O Windows protegeu o computador":
  clique em "Mais informações" e em "Executar assim mesmo".
- **Linux:** \`padmap-$tag-linux-x64.tar.gz\`. Precisa de escrita em \`/dev/uinput\`.
- **macOS (Apple Silicon):** \`padmap-$tag-macos-arm64.tar.gz\`. Precisa da
  permissão de Acessibilidade; na primeira vez, abra com clique direito → Abrir.
EOF

if [ -n "$prev" ]; then
  echo
  echo "Todas as mudanças: https://github.com/walter-souza/padmap/compare/$prev...$tag"
fi
