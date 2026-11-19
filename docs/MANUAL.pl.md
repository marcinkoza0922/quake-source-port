# Instrukcja obsługi

Ta instrukcja opisuje grę w Quake'a w tym porcie, przede wszystkim to, co
port dodaje w porównaniu z oryginalnym Quake'iem i innymi portami. Wszystkie
nowości są opcjonalne: przy ustawieniach domyślnych gra wygląda i działa
jak Quake.

## Pierwsze kroki

1. Zdobądź własny egzemplarz Quake'a. Wskaż portowi katalog zawierający
   `id1/` (oraz mody obok niego) za pomocą `QUAKE_BASEDIR` lub
   `-basedir <katalog>`.
2. Uruchom `cargo run --release` albo od razu włącz moda poleceniem
   `-game <katalog> +map <mapa>`.
3. Konfiguracja, zapisy i zrzuty ekranu znajdują się w
   `~/.local/share/quake-source-port/<gra>/`, osobny folder dla każdego moda.

Menu główne zawiera Single Player, Multiplayer, Options, **Help** (tę
instrukcję), Quit oraz **Paczki map**, a w tle gra motyw Quake'a
(Options → Dźwięk → Muzyka w menu głównym). Każde menu obsługuje mysz,
klawiaturę, kontroler i ekran dotykowy; najedź na opcję lub ją zaznacz, by
zobaczyć podpowiedź z jej opisem.

W Help strzałki w górę/w dół wybierają rozdział; Page Up/Page Down,
w lewo/w prawo, przyciski boczne lub kółko myszy nad tekstem przewijają go.

## Język

Options → Rozgrywka → **Język** wybiera język menu, komunikatów na ekranie i
tej instrukcji: angielski, francuski, niemiecki, włoski, hiszpański, rosyjski,
polski, ukraiński, czeski, słowacki, chorwacki, bułgarski, szwedzki, fiński,
duński, norweski, niderlandzki, grecki, węgierski, rumuński, portugalski
(Portugalia lub Brazylia), japoński, koreański, chiński (uproszczony lub
tradycyjny), turecki, indonezyjski, malajski, filipiński, wietnamski, serbski,
litewski, łotewski, estoński, maltański, albański, ormiański, gruziński,
islandzki, irlandzki, walijski, baskijski, kataloński, galicyjski, łaciński,
esperanto, białoruski, macedoński, słoweński, suahili, arabski, hebrajski,
perski, urdu, hindi, marathi, bengalski, pendżabski, gudżarati, tamilski,
telugu, tajski, khmerski, amharski lub kantoński.
**System** używa języka twojego komputera. Własne teksty gry z reedycji
z 2021 roku są przetłumaczone tylko na francuski, niemiecki, włoski,
hiszpański i rosyjski; w pozostałych językach są wyświetlane po angielsku.
W językach innych niż angielski drobny tekst w grze używa czcionki menu,
ponieważ klasyczna czcionka Quake'a ma tylko łacińskie litery bez znaków
diakrytycznych. Komunikaty konsoli pozostają po angielsku, chyba że
włączona jest opcja **Tłumacz konsolę**.

## Paczki map (przeglądarka modów)

Menu główne → Paczki map wyświetla wszystkie zainstalowane mody i paczki
map wraz z opisami. Wybierz jedną pozycję, by się na nią przełączyć, a
następnie przeglądaj jej mapy i uruchamiaj dowolną z nich bezpośrednio.
Przeglądarka zapamiętuje twoje najlepsze wyniki zabójstw, sekretów i czasu
na każdej mapie. Każdy mod ma własną konfigurację, więc przypisania
klawiszy i ustawienia dopasowane do jednego moda nie przenoszą się do
innego.

**Gdzie umieszczać paczki map.** Port szuka ich w katalogu bazowym Quake'a
(tym, który zawiera `id1/`, ustawianym przez `QUAKE_BASEDIR` lub
`-basedir`). Wyświetlany jest każdy podfolder, który zawiera dane gry (plik
`.pak` lub `.pk3`, `progs.dat` albo folder `maps/`), więc paczkę map
instaluje się, rozpakowując ją do osobnego folderu obok `id1/`, na
przykład `<basedir>/ad/` lub `<basedir>/my_maps/maps/*.bsp`. Opcjonalnie
dodaj plik `modinfo.txt` z wierszami `title`, `base` i `description`, by
określić, jak paczka ma być wyświetlana. Paczki są wykrywane przy starcie
gry, więc po dodaniu nowej uruchom grę ponownie. Zapisy, zrzuty ekranu i
konfiguracja każdej paczki znajdują się w
`~/.local/share/quake-source-port/<folder>/`, a nie w folderze paczki.

## Podzielony ekran

Do sześciu graczy na jednym komputerze, w trybie kooperacji lub
deathmatchu.

- Multiplayer → New Game → **Gracze lokalni** określa liczbę graczy.
- Każdy gracz ma własny widok, HUD, dźwięk i urządzenie sterujące
  (klawiatura i mysz dla gracza 1, po jednym kontrolerze dla pozostałych).
- Układ ekranu może być pionowy, obok siebie, 2×2 lub 3×2
  (`r_splitscreen_layout`).
- Każdy gracz wybiera nazwę oraz kolor koszuli i spodni w Multiplayer →
  Setup.

## Kontrolery

Port został zaprojektowany z myślą o grze na padzie.

- **Koło broni** (`+weaponwheel`): przytrzymaj, by spowolnić czas i wybrać
  broń dowolną gałką lub myszą; puść, by ją zmienić.
- **Wspomaganie celowania** (Options → Wejście → Kontroler): spowolnienie
  w pobliżu wrogów i obrót podążający za nimi. Działa wyłącznie na gałkę,
  nigdy na mysz.
- **Celowanie żyroskopem** (Options → Wejście → Kontroler → Żyroskop):
  przełącznik, osobna czułość dla osi poziomej, pionowej i przechyłu
  (przechył jest domyślnie wyłączony i obraca widok tak jak oś pozioma,
  wartości ujemne go odwracają), filtr szumu oraz wybór, kiedy żyroskop
  działa (zawsze, wyłączony lub tylko przy trzymaniu `+gyroaction`).
  **Styl celowania** przełącza prawą gałkę między celowaniem 2D (płynnie
  obraca widok) a **flick stickiem** (wychyl gałkę, by natychmiast obrócić
  widok w tym kierunku, a potem obracaj gałkę, by go dalej obracać); oba
  dobrze łączą się z żyroskopem przy precyzyjnym celowaniu.
- **Wibracje**, czułość i odwracanie gałek.
- **Symbole przycisków** pasują do twojego pada (Xbox, PlayStation,
  Nintendo) i są wykrywane automatycznie.
- Wszystko można przypisać na nowo w Options → Sterowanie, także z
  poziomu kontrolera.
- Sterowanie dotykowe z regulacją rozmiaru, krycia i szybkości patrzenia.

## Zapisywanie

- **Autozapisy** rotują niezależnie od ręcznych i szybkich zapisów, więc
  nigdy nie nadpisują twoich. Options → Rozgrywka → Ustawienia autozapisu
  określa, ile ich zachować i kiedy zapisywać: na początku poziomu,
  co określony czas, po znalezieniu sekretu lub wszystkich sekretów oraz po
  zabiciu wszystkich wrogów.
- **Karty zapisów** w menu wczytywania i zapisu pokazują poziom, zdrowie,
  pancerz, bronie, klucze oraz zabójstwa i sekrety każdego zapisu.
- **Po śmierci** (Options → Rozgrywka → Po śmierci) gra jednoosobowa może
  zapytać, co zrobić, w menu pauzy, wczytać najnowszy zapis, pozwolić
  wybrać zapis lub zacząć poziom od nowa.

## Rozgrywka i kamera

Options → Rozgrywka:

- Pole widzenia 60–130, kołysanie widoku, odrzut broni, zawsze biegnij.
- **Kamera**: pierwsza osoba, trzecia osoba lub widok zza ramienia
  (prawego lub lewego).
- Pozycja **modeli broni**: środek, prawo, lewo lub ukryte.

## Dostosowanie: modyfikatory rozgrywki i dodatki

Gra jest albo *standardowa*, albo *dostosowana*, co wybiera się przy jej
rozpoczęciu. New Game (oraz wybór poziomu w Wybór poziomu lub Paczki map)
wyświetla okno: **Graj** rozpoczyna grę, **Dostosuj rozgrywkę** otwiera
Modyfikatory rozgrywki i Dodatki. Gdy cokolwiek jest zmienione, przycisk
Graj ma oznaczenie **WŁASNA**, a **Wyłącz wszystkie modyfikacje** (Del,
przycisk X na padzie lub kliknięcie) przywraca standardowe zasady bez
dodatków. Standardowe gry zapisują zwykłe pliki `.sav`, które wczytają
ironwail i vkQuake; dostosowane gry zapisują pliki `.qsa`, które wczytuje
tylko ten port (tak samo gry na podzielonym ekranie i w kooperacji, których
inne porty nie potrafią zapisać). Podczas dostosowanej gry Menu główne →
Dostosuj w każdej chwili zmienia jej modyfikatory i dodatki; zasady gry
standardowej pozostają ustalone. Modyfikatory działają z każdym modem i
wyłączają się w deathmatchu.

- Trudniej: więcej otrzymywanych obrażeń, twardsi wrogowie, mniej amunicji
  i apteczek, 2–4× potwory, start z pistoletem na każdym poziomie, bez
  ręcznych zapisów.
- Łatwiej: mniej otrzymywanych obrażeń, słabsi wrogowie, więcej amunicji i
  zdrowia, nieskończona amunicja, regeneracja zdrowia, bez obrażeń od
  upadku, bez tonięcia, bez obrażeń własnych, podwójne lub potrójne skoki.

## Dema

`record <nazwa>` i `stop` nagrywają demo do twojego folderu użytkownika;
`playdemo <nazwa>` je odtwarza. Po zakończeniu nagrywania demo jest w tle
zapisywane ponownie, by ironwail i vkQuake też mogły je odtworzyć. Jeśli
mod używa efektów cząsteczkowych, których inne porty nie potrafią pokazać,
pełna wersja jest zachowywana jako `<nazwa>.qsd` (`playdemo <nazwa>.qsd`).
`demo_convert <nazwa>` konwertuje starsze demo.

Menu główne → Dema pokazuje zapisane dema, a gdy włączone jest Opcje → Dema
→ Automatyczne nagrywanie dem, także automatyczne demo każdego rozegranego
poziomu; najstarsze są zastępowane, gdy miejsca się zapełnią. K zachowuje
demo, E eksportuje je do wideo, Del je usuwa. Raporty błędów zawierają
automatyczne demo bieżącego poziomu.

Podczas dema Spacja wstrzymuje, przytrzymanie Lewo lub Prawo przewija do
tyłu lub do przodu, Góra i Dół zmieniają prędkość, PgUp i PgDn skaczą o 10
sekund, a Home i End przechodzą na początek i do następnej mapy. I i O
zaznaczają fragment; E eksportuje go albo całe demo do folderu `videos`.
Sterowanie zostaje na ekranie; H lub Ukryj je chowa, a wskazanie dolnej
krawędzi pokazuje pasek, który je przywraca. Klikaj przyciski, kliknij oś
czasu, aby tam przeskoczyć, albo przeciągnij po niej, aby zaznaczyć
fragment; Eksportuj pyta o liczbę klatek, rozmiar, format i plik oraz o to,
czy wyeksportować zaznaczony fragment, czy całe demo.

`capture` zaczyna i kończy nagrywanie wideo podczas gry. Liczba klatek,
format i jakość są w Opcje → Dema → Nagrywanie wideo. Wideo wymaga ffmpeg;
bez niego klatki są zapisywane jako obrazy PNG z plikiem WAV.

## Dostępność

Options → Dostępność:

- Ograniczenie ruchu i intensywność błysków ekranu.
- Filtry dla daltonistów.
- Konfigurowalny celownik: kształt, rozmiar, obrys oraz zmiana koloru na
  celu lub przy zabiciu; opcjonalne znaczniki trafień.
- **Napisy dźwiękowe** i ustawienia komunikatów (Komunikaty i napisy),
  skalowanie tekstu.
- Przełączanie (zamiast przytrzymywania) ataku i biegu.
- Suwak tempa gry, który spowalnia całą grę.
- Automatyczne celowanie ze wskaźnikiem namierzenia oraz ustawieniami
  stożka i zasięgu.

## Ulepszenia

Options → Ulepszenia. Wszystko działa po stronie klienta i jest domyślnie
wyłączone, więc współpracuje z każdym modem i w grze online.

- **Odłamki**: toczące się szczątki i szrapnele z rozbitych beczek, także
  z potworami z modów; mogą odbijać się od drzwi i wind.
- **Lepsza krew**: rozbryzgi na powierzchniach, rany podążające za ciałami,
  krople w powietrzu, które rozpryskują się tam, gdzie spadną.
- **Efekty trafień**: ślady śrutu, gwoździe wbijające się w drewno i
  wrogów, ślady wybuchów.
- **Lepsza woda**: przezroczystość, fale, rozbryzgi i kaustyka.
- **Przezroczyste teleportery**, które pokazują swój cel z paralaksą.
- Okluzja otoczenia, rozmycie ruchu obiektów i prędkości, miękkie
  cząsteczki.
- **Shadery ekranu**: profile RetroArch `.slangp` (CRT i inne).

Options → Grafika zawiera też typowe ustawienia: skalę renderowania,
wygładzanie krawędzi, supersampling, filtrowanie tekstur, 8-bitowy kolor
z ditheringiem, mgłę, kolorowe oświetlenie oraz interpolację animacji i
ruchu.

## Dodatki

Dodatki to nowe bronie i wyposażenie obsługiwane przez silnik, więc działają
z progs każdego moda. Są dostępne tylko w grze jednoosobowej i w
kooperacji.

### Rozpoczynanie gry z dodatkami

Gra ma dodatki tylko wtedy, gdy zostanie z nimi rozpoczęta:

1. Single Player → zaznacz pole **Dodatki**, a potem New Game.
2. Wybierz dodatki, które chcesz zabrać, dostosuj ich ustawienia, a potem
   rozpocznij nową grę.

Gry z dodatkami zapisują się jako pliki `.qsa` zamiast `.sav`, więc inne
porty nie próbują ich wczytywać. Karty zapisów pokazują oznaczenie
„ADD-ONS”. Wczytanie zwykłego zapisu wyłącza dodatki. Podczas gry z
dodatkami strona Dodatki (konsola: `menu_options addons`) włącza i wyłącza
poszczególne dodatki; każdy ma zapętlony podgląd, instrukcję, swoje
przypisania i własne ustawienia.

### Bronie

Dodatki trafiają do zwykłych miejsc na broń (wybieranych osobno dla każdego
dodatku, lub miejsca 9 i 0) i pojawiają się w kole broni oraz na
HUD-zie. Strzał i ogień alternatywny to twoje zwykłe przyciski ataku.

| Dodatek | Strzał | Ogień alternatywny | Amunicja |
|---|---|---|---|
| **Działo portalowe** | Umieszcza niebieski portal | Umieszcza pomarańczowy portal | Brak |
| **Railgun** | Przebijający pocisk, który trafia wszystko na linii | — | 2 ogniwa |
| **Hak** | Zaczepia się o drewno, metal, kamień lub wrogów i przyciąga cię | — | Brak |
| **Wyrzutnia bomb przylepnych** | Przytrzymaj, by naładować, puść, by wystrzelić | Detonacja | 1 rakieta |
| **Bumerang** | Przytrzymaj, by oznaczyć wrogów, puść, by rzucić; ogłusza każdego po kolei | Czyści oznaczenia | Brak |
| **Miotacz żelu** | Niebieski żel odpychający (sprężysty) | Pomarańczowy żel przyspieszający (szybki) | Brak |
| **Myjka ciśnieniowa** | Długi strumień wody, który odpycha wrogów i zmywa żel | Szeroki wachlarz strumieni | Brak |

Portale umieszcza się na płaskich powierzchniach świata; przenoszą ciebie,
potwory i rzucone przedmioty, zachowując ich pęd. Gdy opcja **Widok przez
portale** jest włączona, widzisz drugą stronę na żywo, także portale
widoczne przez portale; jakość, liczbę podglądów na żywo, odległość i
głębokość rekurencji można regulować.

### Wyposażenie

Wyposażenie włącza się i wyłącza, zamiast trzymać je w ręku. Otwórz
**koło ekwipunku** (`+equipwheel`) lub użyj `goggles cycle`, by przełączać
kolejno:

- **Noktowizja**
- **Termowizja**

## Gra wieloosobowa

Oprócz gry NetQuake w sieci LAN i przez internet (protokoły 15, 666 i 999)
port łączy się z serwerami **QuakeWorld**. Multiplayer → Join Game
przyjmuje adres lub przeszukuje sieć LAN. Skryptowe cząsteczki FTE są
obsługiwane w modach, które ich używają.

## Wiersz poleceń i konsola

Typowe opcje: `-basedir <katalog>`, `-game <katalog>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <szer>`, `-height <wys>`, `-fullscreen`,
`-window` oraz `+polecenie argumenty`, by uruchomić polecenia konsoli przy
starcie. Konsola (`~`) przyjmuje zwykłe polecenia i zmienne Quake'a.
