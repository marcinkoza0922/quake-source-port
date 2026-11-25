# Manual do usuário

Este manual explica como jogar Quake com este port, com ênfase no que ele
adiciona em relação ao Quake original e a outros ports. Tudo o que é novo é
opcional: com as configurações padrão, o jogo tem a aparência e a
jogabilidade do Quake.

## Primeiros passos

1. Tenha sua própria cópia do Quake. Indique ao port a pasta que contém
   `id1/` (e os mods ao lado dela), com `QUAKE_BASEDIR` ou
   `-basedir <pasta>`.
2. Execute `cargo run --release`, ou inicie um mod diretamente com
   `-game <pasta> +map <mapa>`.
3. Configurações, jogos salvos e capturas de tela ficam em
   `~/.local/share/quake-source-port/<jogo>/`, uma pasta por mod.

O menu principal tem Single Player, Multiplayer, Options, **Help** (este
manual), Quit e **Pacotes de mapas**, com o tema do Quake tocando ao fundo
(Options → Áudio → Música do menu principal). Todos os menus funcionam com
mouse, teclado, controle e tela de toque; passe o cursor sobre uma opção ou
selecione-a para ver uma dica que a explica.

Na Ajuda, cima/baixo escolhe uma seção; Page Up/Page Down, esquerda/direita,
os botões de ombro ou a roda do mouse sobre o texto o rolam.

## Idioma

Options → Jogabilidade → **Idioma** escolhe o idioma dos menus, das
mensagens na tela e deste manual: inglês, francês, alemão, italiano,
espanhol, russo, polonês, ucraniano, tcheco, eslovaco, croata, búlgaro,
sueco, finlandês, dinamarquês, norueguês, holandês, grego, húngaro, romeno,
português (Portugal ou Brasil), japonês, coreano, chinês (simplificado ou
tradicional), turco, indonésio, malaio, filipino, vietnamita, sérvio,
lituano, letão, estoniano, maltês, albanês, armênio, georgiano, islandês,
irlandês, galês, basco, catalão, galego, latim, esperanto, bielorrusso,
macedônio, esloveno, suaíli, árabe, hebraico, persa, urdu, híndi, marati,
bengali, punjabi, guzerate, tâmil, télugo, tailandês, khmer, amárico ou
cantonês.
**Sistema** segue o idioma do seu computador. Os textos do próprio jogo no
relançamento de 2021 só estão traduzidos para francês, alemão, italiano,
espanhol e russo; os outros idiomas os mostram em inglês. Em idiomas
diferentes do inglês, os textos pequenos do jogo usam a fonte dos menus,
porque a fonte clássica do Quake só tem letras latinas sem acento. As
mensagens do console continuam em inglês, a menos que **Traduzir o
console** esteja ativado.

## Pacotes de mapas

Menu principal → Pacotes de mapas lista cada mod e pacote de mapas
instalado com sua descrição. Escolha um para mudar para ele, depois explore
seus mapas e inicie qualquer um diretamente. O navegador registra seus
melhores abates, segredos e tempo em cada mapa. Cada mod mantém suas
próprias configurações, então os controles e ajustes de um mod não passam
para outro.

**Onde ficam os pacotes de mapas.** O port os procura na pasta base do
Quake (a que contém `id1/`, definida com `QUAKE_BASEDIR` ou `-basedir`).
Toda subpasta ali que contenha dados de jogo (um arquivo `.pak` ou `.pk3`,
um `progs.dat` ou uma pasta `maps/`) aparece na lista, então instale um
pacote de mapas descompactando-o em sua própria pasta ao lado de `id1/`,
por exemplo `<basedir>/ad/` ou `<basedir>/my_maps/maps/*.bsp`. Se quiser,
adicione um `modinfo.txt` com linhas `title`, `base` e `description` para
controlar como ele aparece. Os pacotes são detectados quando o jogo inicia,
então reinicie depois de adicionar um. Jogos salvos, capturas de tela e
configurações de cada pacote ficam em
`~/.local/share/quake-source-port/<pasta>/`, não na pasta do pacote.

## Tela dividida

Até seis jogadores na mesma máquina, em cooperativo ou deathmatch.

- Multiplayer → New Game → **Jogadores locais** define quantas pessoas
  jogam.
- Cada jogador tem sua própria visão, HUD, som e dispositivo de entrada
  (teclado e mouse para o jogador 1, um controle para cada um dos outros).
- O layout da tela pode ser empilhado, lado a lado, 2×2 ou 3×2
  (`r_splitscreen_layout`).
- Cada jogador escolhe um nome e as cores da camisa e da calça em
  Multiplayer → Setup.

## Controles

O port foi feito para ser jogado com controle.

- **Roda de armas** (`+weaponwheel`): segure para desacelerar o tempo e
  escolher uma arma com qualquer analógico ou com o mouse; solte para
  trocar.
- **Assistência de mira** (Options → Entrada → Controle): desaceleração
  perto dos inimigos e rotação que os acompanha. Ela só se aplica ao
  analógico, nunca ao mouse.
- **Mira por giroscópio** (Options → Entrada → Controle → Giroscópio): uma
  chave liga/desliga, sensibilidades separadas para os eixos de guinada,
  arfagem e rolagem (a rolagem vem desativada e gira a visão como a
  guinada; valores negativos a invertem), um filtro de ruído e quando o
  giroscópio se aplica (sempre, desativado ou só enquanto `+gyroaction`
  está pressionado). **Estilo de mira** alterna o analógico direito entre a
  mira 2D (gira a visão continuamente) e o **flick stick** (empurre o
  analógico para apontar a visão na hora para aquela direção, depois gire o
  analógico para rotacioná-la); os dois combinam bem com o giroscópio para
  uma mira precisa.
- **Vibração**, sensibilidade e inversão dos analógicos.
- Os **ícones dos botões** correspondem ao seu controle (Xbox, PlayStation,
  Nintendo) e são detectados automaticamente.
- Remapeie tudo em Options → Controles, inclusive pelo controle.
- **Select / View ×2**: Captura de tela; **Start ×2**: Salvamento rápido;
  **Start** (Segurar): Carregamento rápido.
- Controles de toque com tamanho, opacidade e velocidade da visão
  ajustáveis.

## Salvar

- Os **salvamentos automáticos** se revezam separados dos jogos salvos
  manualmente e dos salvamentos rápidos, então nunca sobrescrevem os seus.
  Options → Jogabilidade → Configurações de salvamento automático escolhe
  quantos manter e quando salvar: ao entrar na fase, por intervalo, ao
  achar um segredo ou todos os segredos e ao matar todos os inimigos.
- Os **cartões de jogos salvos** nos menus Carregar/Salvar mostram a fase,
  a vida, a armadura, as armas, as chaves e os abates/segredos de cada jogo
  salvo.
- **Ao morrer** (Options → Jogabilidade → Ao morrer), no modo um jogador o
  jogo pode perguntar o que fazer em um menu pausado, carregar o jogo salvo
  mais recente, deixar você escolher um jogo salvo ou reiniciar a fase.

## Jogabilidade e câmera

Options → Jogabilidade:

- Campo de visão de 60 a 130, balanço da visão, coice da arma, sempre
  correr.
- **Câmera**: primeira pessoa, terceira pessoa ou sobre o ombro (direito ou
  esquerdo).
- Posição dos **modelos da arma**: centro, direita, esquerda ou oculto.

## Personalizar: modificadores de jogabilidade e complementos

Um jogo é *padrão* ou *personalizado*, escolhido quando ele começa. New
Game (e escolher uma fase em Seleção de fase ou em Pacotes de mapas) mostra
uma janela: **Jogar** inicia o jogo, **Personalizar a experiência** abre os
modificadores de jogabilidade e os complementos. Enquanto houver algo
alterado, Jogar recebe a etiqueta **PERSONALIZADO** e **Desativar todas as
modificações** (Del, X no controle ou clique) restaura as regras padrão sem
complementos. Jogos padrão salvam arquivos `.sav` comuns, que o ironwail e o
vkQuake conseguem carregar; jogos personalizados salvam arquivos `.qsa` que
só este port carrega (assim como os jogos em tela dividida e cooperativo,
que outros ports não conseguem salvar). Durante um jogo personalizado, Menu
principal → Personalizar muda seus modificadores e complementos a qualquer
momento; as regras de um jogo padrão continuam fixas. Os modificadores
funcionam com qualquer mod e são desativados no deathmatch.

- Mais difícil: mais dano recebido, inimigos mais resistentes, menos
  munição e vida coletadas, monstros x2–x4, início do zero em cada fase, sem
  salvamento manual.
- Mais fácil: menos dano recebido, inimigos mais fracos, mais munição e
  vida, munição infinita, regeneração de vida, sem dano de queda, sem
  afogamento, sem dano a si mesmo, pulos duplos ou triplos.

## Demos

`record <nome>` e `stop` gravam uma demo na sua pasta de usuário;
`playdemo <nome>` a reproduz. Quando a gravação termina, a demo é reescrita
em segundo plano para que o ironwail e o vkQuake também possam
reproduzi-la. Se um mod usa efeitos de partículas que outros ports não
conseguem mostrar, a versão completa é mantida como `<nome>.qsd`
(`playdemo <nome>.qsd`). `demo_convert <nome>` converte uma demo antiga.

Menu principal → Demos mostra suas demos salvas e, com Opções → Demos →
Gravar demos automaticamente ligado, uma demo automática de cada fase que
você jogar; as mais antigas são substituídas quando os espaços estão cheios.
Digite para filtrar as listas e pressione Tab para mudar a ordem; os botões
ao lado da lista reproduzem, exportam, renomeiam, guardam ou apagam a demo
escolhida, e renomear uma demo automática a guarda. Os relatórios de bug
incluem a demo automática da fase atual.

Durante uma demo, Espaço pausa, segurar Esquerda ou Direita retrocede ou
avança, Cima e Baixo mudam a velocidade, PgUp e PgDn pulam 10 segundos, e
Home e End vão para o início e para o próximo mapa. I e O marcam um trecho;
E o exporta, ou a demo inteira, para a pasta `videos`. Os controles ficam na
tela; H ou Ocultar os escondem, e apontar para a borda inferior mostra uma
faixa que os traz de volta. Clique nos botões, clique na linha do tempo para
pular para lá ou arraste sobre ela para destacar um trecho; Exportar pede a
taxa de quadros, o tamanho, o formato e o arquivo, e se deve exportar o
trecho destacado ou a demo inteira.

No controle, A pausa, o direcional volta, avança e muda a velocidade, LB e
RB pulam 10 segundos, e os botões dos analógicos vão para o início e para o
próximo mapa. LT e RT marcam um trecho, B o limpa, X exporta, Y guarda uma
demo automática e View oculta os controles.

`capture` inicia e para a gravação de um vídeo enquanto você joga. Taxa de
quadros, formato e qualidade ficam em Opções → Demos → Gravação de vídeo. Os
vídeos precisam de ffmpeg; sem ele, os quadros são salvos como imagens PNG
com um arquivo WAV.

## Acessibilidade

Options → Acessibilidade:

- Movimento reduzido e intensidade dos clarões na tela.
- Filtros para daltonismo.
- Mira configurável: formato, tamanho, contorno e mudança de cor sobre o
  alvo ou ao matar; marcadores de acerto opcionais.
- **Legendas de sons**; a posição, o tamanho e a duração das mensagens e
  legendas são ajustados no editor de HUD.
- Atacar e correr no modo alternar (em vez de segurar).
- Um controle deslizante de velocidade do jogo para desacelerar o jogo
  inteiro.
- Mira automática com indicador de trava e configurações de cone e alcance.

## Melhorias

Options → Melhorias. Tudo roda no cliente e vem desativado, então funciona
com qualquer mod e online.

- **Destroços**: vísceras que rolam e estilhaços de barris destruídos,
  também com monstros de mods; eles podem bater em portas e elevadores.
- **Sangue aprimorado**: respingos nas superfícies, ferimentos que
  acompanham os corpos, gotas no ar que se espalham onde caem.
- **Efeitos de impacto**: marcas de chumbo, pregos que se fixam na madeira e
  nos inimigos, marcas de queimadura de explosões.
- **Água aprimorada**: translucidez, ondulações, respingos e cáusticas;
  **Refração debaixo d'água**, **Gotas na tela**.
- **Teletransportes transparentes** que mostram o destino com paralaxe.
- Oclusão de ambiente, desfoque de movimento por objeto e de velocidade,
  partículas suaves.
- **Shaders de tela**: predefinições `.slangp` do RetroArch (CRT e outras).

Options → Gráficos também tem as opções de sempre: escala de renderização,
antisserrilhamento, superamostragem, filtro de texturas, cor de 8 bits com
pontilhado, neblina, iluminação colorida e interpolação de animações e
movimento.

## Complementos

Complementos são novas armas e equipamentos controlados pelo motor, então
funcionam com o progs de qualquer mod. Eles estão disponíveis só no modo um
jogador e no cooperativo.

### Iniciar um jogo com complementos

Um jogo só tem complementos se começar com eles:

1. Single Player → marque a caixa **Complementos** e depois New Game.
2. Escolha quais complementos levar e ajuste as configurações deles, depois
   inicie o novo jogo.

Jogos com complementos são salvos como arquivos `.qsa` em vez de `.sav`,
para que outros ports não tentem carregá-los. Os cartões de jogos salvos
mostram um selo "ADD-ONS". Carregar um jogo salvo comum desativa os
complementos. Durante um jogo com complementos, a página Complementos
(console: `menu_options addons`) liga e desliga cada complemento; cada um
tem uma prévia em loop, instruções, suas atribuições e suas próprias
configurações.

### Armas

Os complementos ocupam os espaços de arma comuns (escolha qual para cada
complemento, ou os espaços 9 e 0) e aparecem na roda de armas e no HUD. O
disparo e o disparo alternativo são seus botões de ataque de sempre.

| Complemento | Disparo | Disparo alternativo | Munição |
|---|---|---|---|
| **Arma de portais** | Coloca o portal azul | Coloca o portal laranja | Nenhuma |
| **Railgun** | Projétil perfurante que atinge tudo em linha | — | 2 células |
| **Gancho** | Prende-se a madeira, metal, pedra ou inimigos e puxa você | — | Nenhuma |
| **Lança-bombas adesivas** | Segure para carregar, solte para lançar | Detonar | 1 foguete |
| **Bumerangue** | Segure para marcar inimigos, solte para arremessar; atordoa cada um por vez | Limpar marcas | Nenhuma |
| **Pulverizador de gel** | Gel azul de repulsão (quicante) | Gel laranja de propulsão (rápido) | Nenhuma |
| **Lava-jato** | Jato de água longo que empurra inimigos e lava o gel | Leque largo de jatos | Nenhuma |
| **Cabo macho-macho** | O tiro secundário em um painel eletrônico iluminado ao alcance conecta uma ponta; a outra dispara raios com munição ilimitada | O tiro secundário ou trocar de arma o arranca | Nenhuma |
| **Pistolas duplas** | Esquerda | Direita | Nenhuma (Recarregar: R) |
| **Fuzil de assalto** | Um fuzil automático que recarrega com seus pregos | Disparo alternativo mira pela mira de ponto vermelho para tiros mais precisos e uma visão mais próxima | Pregos (Recarregar: R) |

Os portais são colocados em superfícies planas do mundo e levam você,
monstros e objetos arremessados através deles, mantendo o impulso. Com
**Ver através dos portais** ativado, você vê o outro lado ao vivo,
inclusive portais visíveis através de portais; a qualidade, o número de
visões ao vivo, a distância e a profundidade da recursão são ajustáveis.

### Equipamento

O equipamento é ligado e desligado em vez de empunhado. Abra a **roda de
equipamentos** (`+equipwheel`) ou use `goggles cycle` para alternar entre:

- **Visão noturna**
- **Visão térmica**

## Multiplayer

Além do NetQuake por LAN e internet (protocolos 15, 666 e 999), o port se
conecta a servidores **QuakeWorld**. Multiplayer → Join Game aceita um
endereço ou busca na LAN. As partículas com script do FTE são suportadas
nos mods que as usam.

## Linha de comando e console

Opções comuns: `-basedir <pasta>`, `-game <pasta>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <l>`, `-height <a>`, `-fullscreen`, `-window` e
`+comando args` para executar comandos do console na inicialização. O
console (`~`) aceita os comandos e cvars habituais do Quake.
