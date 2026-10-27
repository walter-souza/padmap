# padmap

Mapeia qualquer controle (Xbox, PlayStation, Switch ou genérico) para o
teclado. Windows primeiro, com Linux e macOS já previstos na arquitetura.

> Nome provisório. Estado: primeira versão com interface gráfica, ainda em testes.

## Como funciona

1. O SDL3 lê o controle e entrega botões e eixos num layout padrão
   (`south` é A no Xbox, Cruz no PlayStation).
2. Um **perfil** em TOML diz qual tecla cada botão ou eixo aciona.
3. O motor de mapeamento gera "pressiona/solta tecla".
4. Um backend nativo injeta as teclas: `SendInput` no Windows, `uinput` no
   Linux e `CGEvent` no macOS.

Detalhes em [docs/arquitetura.md](docs/arquitetura.md).

## Interface gráfica

```sh
cargo run --release -p padmap-gui
```

![Tela principal no modo claro](docs/tela-principal.png)

- **Controle ao vivo:** o desenho acende os botões, move os sticks e enche os
  gatilhos conforme você mexe no controle. A etiqueta da tecla que está sendo
  enviada fica destacada. O controle conectado aparece no topo.
- **Mapear:** cada botão tem uma etiqueta ligada a ele por uma linha. Clique na
  etiqueta (ou no próprio botão do desenho), segure a tecla ou as teclas juntas
  (`Ctrl+S`, `A+B`) e solte: a combinação vale ao soltar tudo. Para um clique
  do mouse, use os botões Esquerdo, Direito ou Meio do modal (segure Ctrl,
  Shift ou Alt antes para combinar). O clique fica apertado enquanto o botão do
  controle estiver apertado, então dá para arrastar. Botão direito na etiqueta,
  ou "Deixar livre" no modal, deixa o botão livre.
- **Sticks:** os dois círculos embaixo mostram em amarelo o setor que está
  acionando uma tecla. Clique num deles (ou em LS / RS) para abrir o
  **Mapeamento do Stick**, que tem dois modos (seletor no topo):
  - **Teclas:** o círculo do stick dividido em setores, cada um com sua tecla.
    Arraste as alças para mudar as divisas, clique num setor e aperte a tecla,
    botão direito deixa o setor livre. Também tem presets de 4 e 8 direções, a
    zona morta do stick e o disparo compartilhado (setores vizinhos acionam
    juntos na diagonal).
  - **Mouse:** o stick move o cursor. Ajuste a velocidade (pixels por segundo
    com o stick no fim do curso), a precisão (curva: mais precisão deixa o
    começo do curso lento, para mirar), a zona morta e a inversão do Y, ou use
    um dos presets. A área de teste mostra um cursor andando com o stick, na
    velocidade real, sem precisar ativar o mapeamento. Os setores ficam
    guardados para quando você voltar às teclas.

  Nos dois modos dá para trocar os eixos que o stick usa.
- **Testar sem risco:** com "Simulação" ligada, "Ativar mapeamento" só mostra
  no log as teclas que seriam enviadas. Desligue para enviar de verdade. O log
  abre pelo botão "Log" no rodapé.
- **Perfis:** Novo, Abrir e Salvar no topo; renomear e "Salvar como…" no menu
  do nome do perfil.
- **Modo claro e escuro:** o botão de sol e lua no topo troca o tema, e a
  escolha fica salva para a próxima vez.

![Tela principal no modo escuro](docs/tela-principal-escura.png)

A sensibilidade dos gatilhos e a histerese continuam no perfil
(`axis_threshold` e `release_ratio`).

O mapeamento continua funcionando com a janela minimizada ou com o jogo em primeiro plano.

## Linha de comando

```sh
cargo build --release

padmap-cli list                              # controles conectados
padmap-cli monitor                           # mostra os eventos ao mexer no controle
padmap-cli check profiles/default.toml       # valida um perfil
padmap-cli run profiles/default.toml         # mapeia de verdade
padmap-cli run --dry-run profiles/default.toml  # só mostra as teclas no log
```

## Perfil

```toml
name = "Meu perfil"

[settings]
axis_threshold = 0.5   # quanto do curso do eixo para acionar a tecla
release_ratio = 0.8    # solta abaixo de 0.5 * 0.8 (evita "piscar")

[buttons]
south = "Space"
start = "Ctrl+S"       # combinações com +

# Ângulos em graus, anti-horário a partir da direita (90 = cima). Cada setor
# vai do seu start até o start do próximo; o último volta ao primeiro.
[sticks.left]
deadzone = 0.5         # opcional; sem ele vale axis_threshold
shared = true          # vizinhos se sobrepõem: a diagonal aciona W e D
sectors = [
    { start = 45.0, key = "W" },
    { start = 135.0, key = "A" },
    { start = 225.0, key = "S" },
    { start = 315.0, key = "D" },
]

# Stick como mouse. Os setores, se houver, ficam guardados.
[sticks.right]
mode = "mouse"
deadzone = 0.15
mouse = { speed = 1200, curve = 2.0, invert_y = false }   # todos opcionais

[axes.right_trigger]
positive = "LShift"
threshold = 0.3
```

Botões: `south east west north back guide start left_stick right_stick
left_shoulder right_shoulder dpad_up dpad_down dpad_left dpad_right misc1
left_paddle1 left_paddle2 right_paddle1 right_paddle2 touchpad`.

Sticks: `left right`. Por padrão usam `left_x/left_y` e `right_x/right_y`;
`x_axis` e `y_axis` trocam os eixos. Perfis antigos com `[axes.left_x]` e
afins continuam abrindo: viram quatro setores com disparo compartilhado. No
modo mouse, `speed` é a velocidade em pixels por segundo com o stick no fim do
curso e `curve` o expoente da resposta (1 é linear; maior é mais preciso no
começo do curso).

Eixos: `left_x left_y right_x right_y left_trigger right_trigger`
(Y negativo é para cima).

Teclas: letras, `0`-`9`, `F1`-`F12`, `Space Enter Escape Tab Backspace
CapsLock Insert Delete Home End PageUp PageDown Up Down Left Right`,
modificadores `Shift/LShift/RShift Ctrl/LCtrl/RCtrl Alt/LAlt/RAlt/AltGr
Win/Meta/Cmd`, pontuação (`Minus Equal Comma Period Slash ...`) e os cliques
do mouse `MouseLeft/Mouse1 MouseRight/Mouse2 MouseMiddle/Mouse3`, que combinam
como qualquer tecla (`Ctrl+MouseLeft`).

## Compilando

Precisa de Rust estável, CMake e um compilador C (o SDL3 é compilado junto,
estaticamente). No Windows, o Visual Studio Build Tools já traz os dois.

## Permissões por sistema

- **Windows:** nada especial. Para enviar teclas a um jogo rodando como
  administrador, o padmap também precisa rodar como administrador.
- **Linux:** acesso de escrita a `/dev/uinput` (ex.: regra udev ou grupo `input`).
- **macOS:** permissão de Acessibilidade para o terminal ou o app.
