# Kullanım kılavuzu

Bu kılavuz, Quake'i bu portla oynamayı anlatır; özellikle de portun
orijinal Quake'e ve diğer kaynak portlarına kattıklarını. Yeni olan her şey
isteğe bağlıdır: varsayılan ayarlarla oyun Quake gibi görünür ve oynanır.

## Başlarken

1. Kendi Quake kopyanızı edinin. Portu, `QUAKE_BASEDIR` veya
   `-basedir <klasör>` ile `id1/` klasörünü (ve yanındaki modları)
   içeren klasöre yönlendirin.
2. `cargo run --release` komutunu çalıştırın veya bir modu doğrudan
   `-game <klasör> +map <harita>` ile başlatın.
3. Yapılandırma, kayıtlar ve ekran görüntüleri her mod için ayrı bir
   klasörde, `~/.local/share/quake-source-port/<oyun>/` altında tutulur.

Ana menüde Single Player, Multiplayer, Options, **Help** (bu kılavuz),
Quit ve **Harita Paketleri** bulunur; arka planda Quake teması çalar
(Options → Ses → Ana Menü Müziği). Her menü fare, klavye, kumanda ve
dokunmatik ekranla çalışır; bir seçeneğin üzerine gelin veya onu seçin,
açıklayan bir ipucu görünür.

Yardım ekranında yukarı/aşağı bir bölüm seçer; Page Up/Page Down,
sol/sağ, omuz düğmeleri veya metnin üzerindeki fare tekerleği metni
kaydırır.

## Dil

Options → Oynanış → **Dil**, menülerin, ekran mesajlarının ve bu
kılavuzun dilini seçer: İngilizce, Fransızca, Almanca, İtalyanca,
İspanyolca, Rusça, Lehçe, Ukraynaca, Çekçe, Slovakça, Hırvatça, Bulgarca,
İsveççe, Fince, Danca, Norveççe, Felemenkçe, Yunanca, Macarca, Rumence,
Portekizce (Portekiz veya Brezilya), Japonca, Korece, Çince (Basitleştirilmiş
veya Geleneksel), Türkçe, Endonezce, Malayca, Filipince, Vietnamca,
Sırpça, Litvanca, Letonca, Estonca, Maltaca, Arnavutça, Ermenice, Gürcüce,
İzlandaca, İrlandaca, Galce, Baskça, Katalanca, Galiçyaca, Latince,
Esperanto, Belarusça, Makedonca, Slovence, Svahilice, Arapça, İbranice,
Farsça, Urduca, Hintçe, Marathi, Bengalce, Pencapça, Güceratça, Tamilce,
Teluguca, Tayca, Kmerce, Amharca veya Kantonca.
**Sistem** bilgisayarınızın dilini izler. 2021 yeniden sürümünün kendi oyun
metinleri yalnızca Fransızca, Almanca, İtalyanca, İspanyolca ve Rusçaya
çevrilmiştir; diğer dillerde bu metinler İngilizce görünür. İngilizce
dışındaki dillerde oyun içindeki küçük yazılar menü yazı tipini kullanır,
çünkü klasik Quake yazı tipinde yalnızca aksansız Latin harfleri vardır.
Konsol mesajları, **Konsolu Çevir** açık olmadıkça İngilizce kalır.

## Harita Paketleri (mod tarayıcısı)

Ana menü → Harita Paketleri, kurulu her modu ve harita paketini
açıklamasıyla listeler. Birini seçerek ona geçin, ardından haritalarına göz
atın ve istediğinizi doğrudan başlatın. Tarayıcı her harita için en iyi
öldürme, gizli alan ve süre değerlerinizi kaydeder. Her modun kendi
yapılandırması vardır; böylece bir mod için ayarlanan tuş atamaları ve
ayarlar diğerine sızmaz.

**Harita paketleri nereye konur.** Port onları Quake ana klasöründe arar
(`id1/` klasörünü barındıran, `QUAKE_BASEDIR` veya `-basedir` ile
belirlenen klasör). Oradaki, oyun verisi içeren (bir `.pak` veya `.pk3`
dosyası, bir `progs.dat` ya da bir `maps/` klasörü) her alt klasör
listelenir; bu yüzden bir harita paketini kurmak için onu `id1/` yanında
kendi klasörüne açın, örneğin `<basedir>/ad/` veya
`<basedir>/my_maps/maps/*.bsp`. Nasıl listeleneceğini belirlemek için
isteğe bağlı olarak `title`, `base` ve `description` satırları içeren bir
`modinfo.txt` ekleyin. Paketler oyun başlarken bulunur, bu yüzden bir paket
ekledikten sonra oyunu yeniden başlatın. Her paketin kayıtları, ekran
görüntüleri ve yapılandırması paket klasöründe değil,
`~/.local/share/quake-source-port/<klasör>/` altında tutulur.

## Bölünmüş ekran

Tek bilgisayarda, ortak oyunda veya deathmatch'te altı oyuncuya kadar.

- Multiplayer → New Game → **Yerel Oyuncular** kaç kişinin oynayacağını
  belirler.
- Her oyuncunun kendi görüntüsü, HUD'u, sesi ve giriş cihazı olur (1.
  oyuncu için klavye ve fare, diğerleri için birer kumanda).
- Ekran düzeni üst üste, yan yana, 2×2 veya 3×2 olabilir
  (`r_splitscreen_layout`).
- Her oyuncu Multiplayer → Setup altında bir ad ile gömlek/pantolon
  renklerini seçer.

## Kumandalar

Port, kumandayla oynanmak üzere tasarlanmıştır.

- **Silah Çarkı** (`+weaponwheel`): zamanı yavaşlatmak ve iki çubuktan
  biriyle veya fareyle bir silah seçmek için basılı tutun; değiştirmek için
  bırakın.
- **Nişan Yardımı** (Options → Giriş → Kumanda): düşmanların yakınında
  yavaşlama ve onları izleyen dönüş. Yalnızca çubuğa uygulanır, fareye asla.
- **Jiroskopla Nişan** (Options → Giriş → Kumanda → Jiroskop): bir aç/kapat
  düğmesi; ayrıca yatay, dikey ve yuvarlanma eksenleri için ayrı hassasiyet
  (yuvarlanma varsayılan olarak kapalıdır ve görüşü yatay eksen gibi
  döndürür, negatif değerler yönü tersine çevirir), bir gürültü filtresi ve
  jiroskopun ne zaman geçerli olduğu (her zaman, `+gyroaction` basılıyken
  kapalı veya yalnızca basılıyken). **Nişan Stili** sağ çubuğu 2D nişan
  (görüşü sürekli döndürür) ile **fiske çubuğu** (çubuğu ittiğiniz yöne
  görüş anında döner, ardından çubuğu döndürmek görüşü döndürür) arasında
  değiştirir; ikisi de hassas nişan için jiroskopla iyi birleşir.
- **Titreşim**, çubuk hassasiyeti ve ters çevirme.
- **Düğme Simgeleri** kumandanıza (Xbox, PlayStation, Nintendo) uyar ve
  otomatik algılanır.
- Her şeyi Options → Kontroller altında, kumandadan da olmak üzere
  yeniden atayın.
- Boyutu, opaklığı ve bakış hızı ayarlanabilen dokunmatik kontroller.

## Kayıt

- **Otomatik kayıtlar** elle yapılan kayıtlardan ve hızlı kayıtlardan ayrı
  döner, böylece sizinkilerin üzerine asla yazmaz. Options → Oynanış →
  Otomatik Kayıt Ayarları kaç tane saklanacağını ve ne zaman kaydedileceğini
  seçer: bölüm girişinde, zamanlayıcıyla, bir gizli alan ya da tüm gizli
  alanlar bulununca ve tüm düşmanlar öldürülünce.
- Yükleme/Kayıt menülerindeki **kayıt kartları** her kaydın bölümünü,
  sağlığını, zırhını, silahlarını, anahtarlarını ve öldürme/gizli alan
  sayılarını gösterir.
- **Ölünce** (Options → Oynanış → Ölünce) tek oyunculu oyun, duraklatılmış
  bir menüde ne yapılacağını sorabilir, en son kaydı yükleyebilir, bir
  kayıt seçmenize izin verebilir veya bölümü yeniden başlatabilir.

## Oynanış ve kamera

Options → Oynanış:

- 60–130 arası görüş alanı, görüş sallanması, silah tepmesi, hep koşma.
- **Kamera**: birinci şahıs, üçüncü şahıs veya omuz üstü (sağ ya da sol).
- **Silah modeli** konumu: orta, sağ, sol veya gizli.

## Özelleştir: oynanış değiştiricileri ve eklentiler

Bir oyun, başlarken seçilen biçimde ya *orijinal* ya da *özelleştirilmiş*
olur. New Game (ve Bölüm Seçimi'nde ya da Harita Paketleri'nde bir bölüm
seçmek) bir pencere açar: **Oyna** oyunu başlatır, **Deneyimi
Özelleştir** Oynanış Değiştiricileri ve Eklentiler'i açar. Herhangi bir
şey değiştirildiğinde Oyna **ÖZEL** olarak işaretlenir ve **Tüm
Değişiklikleri Kapat** (Del, kumandada X veya tıklama) eklentisiz standart
kuralları geri getirir. Orijinal oyunlar ironwail ve vkQuake'in
yükleyebildiği sıradan `.sav` dosyalarına kaydeder; özelleştirilmiş oyunlar
yalnızca bu portun yükleyebildiği `.qsa` dosyalarına kaydeder (diğer
portların kaydedemediği bölünmüş ekran ve ortak oyunlar da öyle).
Özelleştirilmiş bir oyun sırasında Ana menü → Özelleştir, değiştiricilerini
ve eklentilerini istediğiniz zaman değiştirir; orijinal bir oyunun
kuralları sabit kalır. Değiştiriciler her modla çalışır ve deathmatch'te
kapanır.

- Daha zor: daha fazla alınan hasar, daha güçlü düşmanlar, daha az cephane
  ve sağlık toplama, 2–4x canavar, her bölümde sıfırdan başlangıç, elle
  kayıt yok.
- Daha kolay: daha az alınan hasar, daha zayıf düşmanlar, daha fazla
  cephane ve sağlık, sınırsız cephane, sağlık yenilenmesi, düşme hasarı
  yok, boğulma yok, kendine hasar yok, çift veya üçlü zıplama.

## Demolar

`record <ad>` ve `stop` kullanıcı klasörünüze bir demo kaydeder;
`playdemo <ad>` onu oynatır. Kayıt durduğunda demo arka planda yeniden
yazılır, böylece ironwail ve vkQuake de onu oynatabilir. Bir mod diğer
portların gösteremediği parçacık efektleri kullanıyorsa tam sürüm
`<ad>.qsd` olarak saklanır (`playdemo <ad>.qsd`). `demo_convert <ad>`
eski bir demoyu dönüştürür.

Ana menü → Demolar kayıtlı demolarınızı ve Seçenekler → Demolar → Demoları
otomatik kaydet açıksa oynadığınız her bölümün otomatik demosunu listeler;
yuvalar dolunca en eskiler değiştirilir. K bir demoyu saklar, E onu videoya
aktarır, Del siler. Hata raporları geçerli bölümün otomatik demosunu içerir.

Demo oynarken Boşluk duraklatır, Sol veya Sağ'ı basılı tutmak geri ya da
ileri sarar, Yukarı ve Aşağı hızı değiştirir, PgUp ve PgDn 10 saniye atlar,
Home ve End başa ve sonraki haritaya gider. I ve O bir klibi işaretler; E
onu ya da tüm demoyu `videos` klasörüne aktarır. Kontroller ekranda kalır; H
veya Gizle onları gizler, alt kenarı işaret etmek ise onları geri getiren
bir şerit gösterir. Düğmelere tıklayın, oraya atlamak için zaman çizelgesine
tıklayın ya da bir klibi işaretlemek için üzerinde sürükleyin; Dışa aktar,
kare hızını, boyutu, biçimi ve dosyayı, ayrıca işaretli klibin mi tüm
demonun mu aktarılacağını sorar.

`capture` oynarken video kaydını başlatır ve durdurur. Kare hızı, biçim ve
kalite Seçenekler → Demolar → Video kaydı altındadır. Videolar için ffmpeg
gerekir; yoksa kareler bir WAV dosyasıyla birlikte PNG resimleri olarak
kaydedilir.

## Erişilebilirlik

Options → Erişilebilirlik:

- Azaltılmış hareket ve ekran parlaması yoğunluğu.
- Renk körlüğü filtreleri.
- Ayarlanabilir nişangah: şekil, boyut, dış çizgi ve hedefteyken ya da
  öldürünce renk değişimi; isteğe bağlı isabet işaretleri.
- **Ses Altyazıları** ve mesaj ayarları (Mesajlar ve Altyazılar), yazı
  ölçekleme.
- Saldırı ve koşma için aç/kapat (basılı tutmak yerine).
- Tüm oyunu yavaşlatmak için bir oyun hızı kaydırıcısı.
- Kilit göstergesi, koni ve menzil ayarlarıyla otomatik nişan.

## Geliştirmeler

Options → Geliştirmeler. Hepsi istemci tarafında çalışır ve varsayılan
olarak kapalıdır, bu yüzden her modla ve çevrim içi çalışır.

- **Enkaz**: mod canavarlarında da yuvarlanan et parçaları ve parçalanan
  varillerden şarapnel; kapılara ve asansörlere çarpabilir.
- **Gelişmiş kan**: yüzeylere sıçrayan kan, bedenleri izleyen yaralar,
  düştükleri yere yapışan havadaki damlalar.
- **İsabet efektleri**: saçma izleri, ahşaba ve düşmanlara saplanan
  çiviler, patlama yanık izleri.
- **Gelişmiş Su**: saydamlık, dalgalar, sıçramalar ve kostikler.
- Hedeflerini paralaksla gösteren **Saydam Işınlayıcılar**.
- Ortam perdeleme, nesne başına ve hıza bağlı hareket bulanıklığı, yumuşak
  parçacıklar.
- **Ekran gölgelendiricileri**: RetroArch `.slangp` hazır ayarları (CRT ve
  diğerleri).

Options → Grafik ayrıca bilinen görüntü ölçeği, kenar yumuşatma, süper
örnekleme, doku filtreleme, titreklikle 8-bit renk, sis, renkli aydınlatma
ve animasyon/hareket ara değerleme ayarlarını sunar.

## Eklentiler

Eklentiler, motor tarafından çalıştırılan yeni silahlar ve ekipmanlardır;
bu yüzden her modun progs'u ile çalışırlar. Yalnızca tek oyunculu ve ortak
oyunda kullanılabilirler.

### Eklentili oyun başlatma

Bir oyunda eklentiler yalnızca oyun onlarla başlarsa bulunur:

1. Single Player → **Eklentiler** kutusunu işaretleyin, ardından New Game.
2. Hangi eklentileri alacağınızı seçin ve ayarlarını yapın, ardından yeni
   oyunu başlatın.

Eklentili oyunlar `.sav` yerine `.qsa` dosyalarına kaydedilir, böylece
diğer portlar onları yüklemeye çalışmaz. Kayıt kartlarında "ADD-ONS"
rozeti görünür. Normal bir kaydı yüklemek eklentileri kapatır. Eklentili
bir oyun oynarken Eklentiler sayfası (konsol: `menu_options addons`) tek
tek eklentileri açıp kapatır; her birinin döngüsel bir önizlemesi,
talimatları, tuş atamaları ve kendi ayarları vardır.

### Silahlar

Eklentiler normal silah yuvalarına yerleşir (her eklenti için seçilir ya da
9 ve 0 yuvaları) ve silah çarkında ve HUD'da görünür. Ateş ve alternatif
ateş her zamanki saldırı düğmelerinizdir.

| Eklenti | Ateş | Alternatif Ateş | Cephane |
|---|---|---|---|
| **Portal Silahı** | Mavi portalı yerleştirir | Turuncu portalı yerleştirir | Yok |
| **Railgun** | Hattaki her şeyi vuran delici mermi | — | 2 hücre |
| **Kanca** | Ahşaba, metale, taşa veya düşmanlara takılır ve sizi çeker | — | Yok |
| **Yapışkan Bomba Atar** | Doldurmak için basılı tutun, fırlatmak için bırakın | Patlat | 1 roket |
| **Bumerang** | Düşmanları işaretlemek için basılı tutun, fırlatmak için bırakın; sırayla her birini sersemletir | İşaretleri temizle | Yok |
| **Jel Püskürtücü** | Mavi itme jeli (zıplatır) | Turuncu hız jeli (hızlandırır) | Yok |
| **Basınçlı Yıkayıcı** | Düşmanları iten ve jeli yıkayan uzun su fışkırması | Geniş fışkırma yelpazesi | Yok |

Portallar dünyanın düz yüzeylerine yerleşir ve sizi, canavarları ve atılan
nesneleri hızlarıyla birlikte geçirir. **Portallardan Görme** açıkken
karşı tarafı, portallardan görünen portallar dahil, canlı görürsünüz;
kalite, canlı görüntü sayısı, mesafe ve özyineleme derinliği
ayarlanabilir.

### Ekipman

Ekipman elde tutulmaz, açılıp kapatılır. Aralarında geçmek için **Ekipman
Çarkı**'nı (`+equipwheel`) açın veya `goggles cycle` kullanın:

- **Gece görüşü**
- **Termal görüş**

## Çok oyunculu

LAN ve internet üzerinden NetQuake oyununun (protokoller 15, 666 ve 999)
yanı sıra port **QuakeWorld** sunucularına da bağlanır. Multiplayer → Join
Game bir adres alır veya LAN'da arama yapar. FTE betikli parçacıkları,
onları kullanan modlar için desteklenir.

## Komut satırı ve konsol

Yaygın seçenekler: `-basedir <klasör>`, `-game <klasör>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <g>`, `-height <y>`, `-fullscreen`,
`-window` ve başlangıçta konsol komutları çalıştırmak için
`+command args`. Konsol (`~`) bilinen Quake komutlarını ve cvar'larını
kabul eder.
