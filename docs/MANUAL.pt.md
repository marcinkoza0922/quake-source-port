# Manual do utilizador

Este manual explica como jogar Quake com este port, dando destaque ao que
acrescenta ao Quake original e a outros source ports. Tudo o que é novo é
opcional: com as definições predefinidas, o jogo tem o aspeto e joga-se
como o Quake.

## Primeiros passos

1. Arranja a tua própria cópia do Quake. Indica ao port a pasta que
   contém `id1/` (e os mods ao lado), com `QUAKE_BASEDIR` ou
   `-basedir <pasta>`.
2. Executa `cargo run --release`, ou inicia logo um mod com
   `-game <pasta> +map <mapa>`.
3. A configuração, as gravações e as capturas de ecrã ficam em
   `~/.local/share/quake-source-port/<jogo>/`, uma pasta por mod.

O menu principal tem Single Player, Multiplayer, Options, **Help** (este
manual), Quit e **Pacotes de mapas**, com o tema do Quake a tocar por trás
(Options → Áudio → Música do menu principal). Todos os menus funcionam com
rato, teclado, comando e ecrã tátil; passa o cursor ou seleciona uma opção
para ver uma dica que a explica.

Na ajuda, cima/baixo escolhe uma secção; Page Up/Page Down, esquerda/direita,
os botões de ombro ou a roda do rato sobre o texto fazem-no deslocar.

## Idioma

Options → Jogabilidade → **Idioma** escolhe o idioma dos menus, das mensagens
no ecrã e deste manual: inglês, francês, alemão, italiano, espanhol, russo,
polaco, ucraniano, checo, eslovaco, croata, búlgaro, sueco, finlandês,
dinamarquês, norueguês, neerlandês, grego, húngaro, romeno, português (Portugal
ou Brasil), japonês, coreano, chinês (simplificado ou tradicional), turco,
indonésio, malaio, filipino, vietnamita, sérvio, lituano, letão, estónio,
maltês, albanês, arménio, georgiano, islandês, irlandês, galês, basco, catalão,
galego, latim, esperanto, bielorrusso, macedónio, esloveno ou suaíli.
**Sistema** segue o idioma do teu computador. Os textos de jogo da reedição
de 2021 só estão traduzidos para francês, alemão, italiano, espanhol e
russo; os outros idiomas mostram-nos em inglês. Nos idiomas que não o
inglês, o texto pequeno durante o jogo usa o tipo de letra dos menus, porque
o tipo de letra clássico do Quake só tem letras latinas sem acentos. As
mensagens da consola ficam em inglês, a não ser que **Traduzir consola**
esteja ligado.

## Pacotes de mapas (navegador de mods)

Menu principal → Pacotes de mapas lista todos os mods e pacotes de mapas
instalados, com a sua descrição. Escolhe um para mudar para ele, depois
navega pelos seus mapas e inicia qualquer um diretamente. O navegador regista
os teus melhores abates, segredos e tempos por mapa. Cada mod tem a sua
própria configuração, por isso as atribuições e definições afinadas para um
mod não passam para outro.

**Onde ficam os pacotes de mapas.** O port procura-os na pasta base do Quake
(a que contém `id1/`, definida com `QUAKE_BASEDIR` ou `-basedir`). Todas as
subpastas que contêm dados de jogo (um ficheiro `.pak` ou `.pk3`, um
`progs.dat` ou uma pasta `maps/`) aparecem na lista, por isso instala um
pacote de mapas descompactando-o numa pasta própria ao lado de `id1/`, por
exemplo `<basedir>/ad/` ou `<basedir>/my_maps/maps/*.bsp`. Opcionalmente,
acrescenta um `modinfo.txt` com linhas `title`, `base` e `description` para
controlar como aparece na lista. Os pacotes são detetados quando o jogo
arranca, por isso reinicia depois de acrescentar um. As gravações, capturas
de ecrã e a configuração de cada pacote ficam em
`~/.local/share/quake-source-port/<pasta>/`, não na pasta do pacote.

## Ecrã dividido

Até seis jogadores na mesma máquina, em cooperativo ou deathmatch.

- Multiplayer → New Game → **Jogadores locais** define quantas pessoas
  jogam.
- Cada jogador tem a sua própria vista, HUD, som e dispositivo de entrada
  (teclado e rato para o jogador 1, um comando para cada um dos restantes).
- A disposição do ecrã pode ser empilhada, lado a lado, 2×2 ou 3×2
  (`r_splitscreen_layout`).
- Cada jogador escolhe um nome e as cores da camisola e das calças em
  Multiplayer → Setup.

## Comandos

O port foi pensado para ser jogado com comando.

- **Roda de armas** (`+weaponwheel`): mantém premido para abrandar o tempo e
  escolhe uma arma com qualquer um dos manípulos ou com o rato; larga para
  mudar.
- **Assistência à mira** (Options → Entrada → Comando): abrandamento perto
  dos inimigos e rotação que os segue. Aplica-se só ao manípulo, nunca ao
  rato.
- **Mira com giroscópio** (Options → Entrada → Comando → Giroscópio): um
  interruptor, mais sensibilidades separadas para os eixos de guinada,
  arfagem e rolamento (o rolamento vem desligado e roda a vista como a
  guinada; valores negativos invertem-no), um filtro de ruído e quando o
  giroscópio se aplica (sempre, desligado ou só enquanto `+gyroaction` está
  premido). **Estilo de mira** alterna o manípulo direito entre mira 2D
  (roda a vista continuamente) e **flick stick** (empurra o manípulo para
  virar logo a vista nessa direção e depois roda o manípulo para a fazer
  girar); ambos combinam bem com o giroscópio para uma mira fina.
- **Vibração**, sensibilidade e inversão dos manípulos.
- Os **ícones dos botões** correspondem ao teu comando (Xbox, PlayStation,
  Nintendo) e são detetados automaticamente.
- Reatribui tudo em Options → Controlos, inclusive a partir do comando.
- Controlos táteis com tamanho, opacidade e velocidade da vista ajustáveis.

## Gravar

- As **gravações automáticas** rodam separadamente das gravações manuais e
  rápidas, por isso nunca substituem as tuas. Options → Jogabilidade →
  Definições de gravação automática escolhe quantas manter e quando gravar:
  ao entrar num nível, periodicamente, ao encontrar um segredo ou todos os
  segredos, e ao matar todos os inimigos.
- Os **cartões de gravação** nos menus de carregar e guardar mostram o
  nível, a vida, a armadura, as armas, as chaves e os abates/segredos de
  cada gravação.
- **Ao morrer** (Options → Jogabilidade → Ao morrer), em um jogador, o jogo
  pode perguntar o que fazer num menu em pausa, carregar a gravação mais
  recente, deixar-te escolher uma gravação ou recomeçar o nível.

## Jogabilidade e câmara

Options → Jogabilidade:

- Campo de visão de 60 a 130, balanço da vista, recuo da arma, correr
  sempre.
- **Câmara**: primeira pessoa, terceira pessoa ou sobre o ombro (direito ou
  esquerdo).
- Posição dos **modelos da arma**: centro, direita, esquerda ou escondidos.

## Personalizar: modificadores de jogo e complementos

Um jogo é *normal* ou *personalizado*, e isso escolhe-se quando começa. New
Game (e escolher um nível em Escolher nível ou Pacotes de mapas) mostra uma
janela: **Jogar** inicia o jogo, **Personalizar experiência** abre os
Modificadores de jogo e os Complementos. Enquanto houver algo alterado,
Jogar fica marcado como **PERSONALIZADO** e **Desativar todas as
modificações** (Del, X no comando, ou clique) repõe as regras normais sem
complementos. Os jogos normais gravam ficheiros `.sav` comuns que o
ironwail e o vkQuake conseguem carregar; os jogos personalizados gravam
ficheiros `.qsa` que só este port carrega (tal como os jogos em ecrã
dividido e cooperativo, que os outros ports não conseguem gravar). Durante
um jogo personalizado, Menu principal → Personalizar altera os seus
modificadores e complementos a qualquer momento; as regras de um jogo
normal ficam fixas. Os modificadores funcionam com qualquer mod e
desligam-se em deathmatch.

- Mais difícil: mais dano sofrido, inimigos mais resistentes, menos
  munições e vida apanhadas, monstros a dobrar até ao quádruplo, começar do
  zero em cada nível, sem gravações manuais.
- Mais fácil: menos dano sofrido, inimigos mais fracos, mais munições e
  vida, munições infinitas, regeneração de vida, sem dano de queda, sem
  afogamento, sem dano próprio, saltos duplos ou triplos.

## Demos

`record <nome>` e `stop` gravam uma demo na tua pasta de utilizador;
`playdemo <nome>` reprodu-la. Quando a gravação para, a demo é reescrita em
segundo plano para que o ironwail e o vkQuake também a possam reproduzir. Se
um mod usar efeitos de partículas que os outros ports não conseguem mostrar,
a versão completa é guardada como `<nome>.qsd` (`playdemo <nome>.qsd`).
`demo_convert <nome>` converte uma demo antiga.

## Acessibilidade

Options → Acessibilidade:

- Movimento reduzido e intensidade dos clarões do ecrã.
- Filtros para daltonismo.
- Mira configurável: forma, tamanho, contorno e mudança de cor sobre um
  alvo ou ao matar; marcadores de impacto opcionais.
- **Legendas de sons** e definições das mensagens (Mensagens e legendas),
  tamanho do texto.
- Atacar e correr em modo alternar (em vez de manter).
- Um controlo de deslize da velocidade do jogo para abrandar todo o jogo.
- Mira automática com indicador de fixação e definições de cone e alcance.

## Melhorias

Options → Melhorias. Tudo do lado do cliente e desligado por predefinição,
por isso funciona com qualquer mod e online.

- **Destroços**: vísceras a rolar e estilhaços de barris desfeitos, também
  com monstros de mods; podem bater em portas e elevadores.
- **Sangue melhorado**: salpicos nas superfícies, feridas que seguem os
  corpos, gotas no ar que se espalham onde caem.
- **Efeitos de impacto**: marcas de chumbos, pregos que se espetam na
  madeira e nos inimigos, marcas de queimadura de explosões.
- **Água melhorada**: transparência, ondulações, salpicos e cáusticas.
- **Teletransportadores transparentes** que mostram o destino com paralaxe.
- Oclusão ambiente, desfoque de movimento por objeto e de velocidade,
  partículas suaves.
- **Shaders de ecrã**: predefinições `.slangp` do RetroArch (CRT e outras).

Options → Gráficos tem também as definições habituais: escala de
renderização, antialiasing, supersampling, filtro de texturas, cor de 8 bits
com pontilhado, nevoeiro, iluminação colorida e interpolação de animações e
movimento.

## Complementos

Os complementos são novas armas e equipamento, geridos pelo motor para
funcionarem com os progs de qualquer mod. São só para um jogador e
cooperativo.

### Iniciar um jogo com complementos

Um jogo só tem complementos se começar com eles:

1. Single Player → marca a caixa **Complementos** e depois New Game.
2. Escolhe que complementos levar e ajusta as suas definições, depois
   Iniciar novo jogo.

Os jogos com complementos gravam em ficheiros `.qsa` em vez de `.sav`, para
que os outros ports não os tentem carregar. Os cartões de gravação mostram
um distintivo "ADD-ONS". Carregar uma gravação normal desliga os
complementos. Durante um jogo com complementos, a página Complementos
(consola: `menu_options addons`) liga e desliga cada complemento; cada um
tem uma pré-visualização em ciclo, instruções, as suas atribuições e as
suas próprias definições.

### Armas

Os complementos vão para as ranhuras de armas normais (escolhe qual para
cada complemento, ou as ranhuras 9 e 0) e aparecem na roda de armas e no
HUD. O disparo e o tiro secundário são os teus botões de ataque habituais.

| Complemento | Disparo | Tiro secundário | Munições |
|---|---|---|---|
| **Arma de portais** | Coloca o portal azul | Coloca o portal laranja | Nenhuma |
| **Railgun** | Projétil perfurante que atinge tudo numa linha | — | 2 células |
| **Gancho** | Prende-se e puxa-te até madeira, metal, pedra ou inimigos | — | Nenhuma |
| **Lança-bombas adesivas** | Mantém para carregar, larga para lançar | Detonar | 1 foguete |
| **Bumerangue** | Mantém para marcar inimigos, larga para lançar; atordoa cada um à vez | Limpar marcas | Nenhuma |
| **Pulverizador de gel** | Gel azul de repulsão (saltitante) | Gel laranja de propulsão (rápido) | Nenhuma |
| **Lavadora de pressão** | Jato de água longo que empurra inimigos e lava o gel | Leque largo de jatos | Nenhuma |

Os portais ficam em superfícies planas do mapa e fazem-te passar a ti, aos
monstros e aos objetos lançados, mantendo o impulso. Com **Ver através dos
portais** ligado vês o outro lado em direto, incluindo portais visíveis
através de portais; a qualidade, o número de vistas em direto, a distância
e a profundidade de recursão são ajustáveis.

### Equipamento

O equipamento liga-se e desliga-se em vez de ser empunhado. Abre a **roda
de equipamento** (`+equipwheel`), ou usa `goggles cycle` para percorrer:

- **Visão noturna**
- **Visão térmica**

## Multiplayer

Além do jogo NetQuake em LAN e pela Internet (protocolos 15, 666 e 999), o
port liga-se a servidores **QuakeWorld**. Multiplayer → Join Game aceita um
endereço ou procura na LAN. As partículas com scripts do FTE são suportadas
para os mods que as usam.

## Linha de comandos e consola

Opções comuns: `-basedir <pasta>`, `-game <pasta>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <l>`, `-height <a>`, `-fullscreen`, `-window`, e
`+comando args` para executar comandos da consola no arranque. A consola
(`~`) aceita os comandos e cvars habituais do Quake.
