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

![Tela principal](docs/tela-principal.png)

- **Controle ao vivo:** o desenho acende os botões, mostra a posição dos sticks
  e o quanto os gatilhos estão apertados. Os controles conectados aparecem acima.
- **Mapear:** clique numa parte do desenho ou em "Capturar" e aperte a tecla
  (ou combinação, como `Ctrl+S`) no teclado. Também dá para digitar o nome da tecla.
- **Testar sem risco:** com "Simulação" marcado, "Ativar mapeamento" só mostra
  no log as teclas que seriam enviadas. Desmarque para enviar de verdade.
- **Perfis:** Novo, Abrir, Salvar e Salvar como, em arquivos `.toml`.
- **Sensibilidade:** ajuste em que ponto sticks e gatilhos acionam e soltam a tecla.

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

[axes.left_x]
negative = "A"
positive = "D"

[axes.right_trigger]
positive = "LShift"
threshold = 0.3
```

Botões: `south east west north back guide start left_stick right_stick
left_shoulder right_shoulder dpad_up dpad_down dpad_left dpad_right misc1
left_paddle1 left_paddle2 right_paddle1 right_paddle2 touchpad`.

Eixos: `left_x left_y right_x right_y left_trigger right_trigger`
(Y negativo é para cima).

Teclas: letras, `0`-`9`, `F1`-`F12`, `Space Enter Escape Tab Backspace
CapsLock Insert Delete Home End PageUp PageDown Up Down Left Right`,
modificadores `Shift/LShift/RShift Ctrl/LCtrl/RCtrl Alt/LAlt/RAlt/AltGr
Win/Meta/Cmd` e pontuação (`Minus Equal Comma Period Slash ...`).

## Compilando

Precisa de Rust estável, CMake e um compilador C (o SDL3 é compilado junto,
estaticamente). No Windows, o Visual Studio Build Tools já traz os dois.

## Permissões por sistema

- **Windows:** nada especial. Para enviar teclas a um jogo rodando como
  administrador, o padmap também precisa rodar como administrador.
- **Linux:** acesso de escrita a `/dev/uinput` (ex.: regra udev ou grupo `input`).
- **macOS:** permissão de Acessibilidade para o terminal ou o app.
