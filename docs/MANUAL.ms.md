# Manual pengguna

Manual ini menerangkan cara bermain Quake dengan port ini, dengan tumpuan
pada apa yang ditambahnya berbanding Quake asal dan port sumber lain.
Semua yang baharu adalah pilihan: dengan tetapan lalai, permainan kelihatan
dan dimainkan seperti Quake.

## Bermula

1. Dapatkan salinan Quake anda sendiri. Halakan port ke direktori yang
   mengandungi `id1/` (dan sebarang mod di sebelahnya), dengan
   `QUAKE_BASEDIR` atau `-basedir <dir>`.
2. Jalankan `cargo run --release`, atau mulakan mod terus dengan
   `-game <dir> +map <map>`.
3. Konfigurasi, simpanan dan tangkapan skrin disimpan dalam
   `~/.local/share/quake-source-port/<game>/`, satu folder bagi setiap mod.

Menu utama mengandungi Single Player, Multiplayer, Options, **Help** (manual
ini), Quit dan **Pek Peta**, dengan tema Quake dimainkan di latar belakang
(Options → Audio → Muzik Menu Utama). Setiap menu berfungsi dengan tetikus,
papan kekunci, alat kawalan dan sentuhan; halakan atau pilih sesuatu pilihan
untuk melihat petua yang menerangkannya.

Dalam Help, atas/bawah memilih bahagian; Page Up/Page Down, kiri/kanan,
butang bahu atau roda tetikus di atas teks menatalnya.

## Bahasa

Options → Permainan → **Bahasa** memilih bahasa menu, mesej pada skrin dan
manual ini: Inggeris, Perancis, Jerman, Itali, Sepanyol, Rusia, Poland,
Ukraine, Czech, Slovak, Croatia, Bulgaria, Sweden, Finland, Denmark,
Norway, Belanda, Greek, Hungary, Romania, Portugis (Portugal atau Brazil),
Jepun, Korea, Cina (Ringkas atau Tradisional), Turki, Indonesia, Melayu,
Filipino, Vietnam, Serbia, Lithuania, Latvia, Estonia, Malta, Albania,
Armenia, Georgia, Iceland, Ireland, Wales, Basque, Catalan, Galicia, Latin,
Esperanto, Belarus, Macedonia, Slovenia, Swahili, Arab, Ibrani, Parsi, Urdu,
Hindi, Marathi, Benggali, Punjabi, Gujerat, Tamil, Telugu, Thai, Khmer, Amharik atau
Kantonis.
**Sistem** mengikut bahasa komputer anda. Teks permainan keluaran semula
2021 sendiri hanya diterjemahkan ke bahasa Perancis, Jerman, Itali, Sepanyol
dan Rusia; bahasa lain memaparkannya dalam bahasa Inggeris. Dalam bahasa
selain Inggeris, teks kecil dalam permainan menggunakan fon menu, kerana
fon Quake klasik hanya mempunyai huruf Latin tanpa diakritik. Mesej konsol
kekal dalam bahasa Inggeris kecuali **Terjemah Konsol** dihidupkan.

## Pek Peta (pelayar mod)

Menu utama → Pek Peta menyenaraikan setiap mod dan pek peta yang dipasang
bersama keterangannya. Pilih satu untuk bertukar kepadanya, kemudian layari
petanya dan mulakan mana-mana peta secara terus. Pelayar ini merekodkan
bunuhan, rahsia dan masa terbaik anda bagi setiap peta. Setiap mod
mempunyai konfigurasinya sendiri, jadi ikatan kekunci dan tetapan yang
dilaraskan untuk satu mod tidak terbawa ke mod lain.

**Tempat letak pek peta.** Port mencarinya dalam direktori asas Quake
(direktori yang mengandungi `id1/`, ditetapkan dengan `QUAKE_BASEDIR` atau
`-basedir`). Setiap subfolder di situ yang mengandungi data permainan (fail
`.pak` atau `.pk3`, `progs.dat`, atau folder `maps/`) disenaraikan, jadi
pasang pek peta dengan membukanya ke dalam folder sendiri di sebelah
`id1/`, contohnya `<basedir>/ad/` atau `<basedir>/my_maps/maps/*.bsp`.
Secara pilihan, tambah fail `modinfo.txt` dengan baris `title`, `base` dan
`description` untuk mengawal cara ia disenaraikan. Pek ditemui semasa
permainan bermula, jadi mulakan semula selepas menambah pek. Simpanan,
tangkapan skrin dan konfigurasi bagi setiap pek disimpan dalam
`~/.local/share/quake-source-port/<folder>/`, bukan dalam folder pek.

## Skrin pisah

Sehingga enam pemain pada satu mesin, dalam koperatif atau deathmatch.

- Multiplayer → New Game → **Pemain Tempatan** menetapkan bilangan pemain.
- Setiap pemain mendapat paparan, HUD, bunyi dan peranti input sendiri
  (papan kekunci dan tetikus untuk pemain 1, satu alat kawalan bagi setiap
  pemain lain).
- Susun atur skrin boleh bertindan, bersebelahan, 2×2 atau 3×2
  (`r_splitscreen_layout`).
- Setiap pemain memilih nama dan warna baju/seluar di bawah Multiplayer →
  Setup.

## Alat kawalan

Port ini direka untuk dimainkan dengan pad permainan.

- **Roda Senjata** (`+weaponwheel`): tahan untuk memperlahankan masa dan
  memilih senjata dengan mana-mana stik atau tetikus; lepaskan untuk
  bertukar.
- **Bantuan Bidikan** (Options → Input → Alat Kawalan): perlahan berhampiran
  musuh dan putaran yang mengikut mereka. Ia hanya terpakai pada stik, tidak
  sekali-kali pada tetikus.
- **Bidikan Giro** (Options → Input → Alat Kawalan → Giro): togol, serta
  kepekaan berasingan untuk paksi rewang, pic dan gulung (gulung mati secara
  lalai dan memusingkan pandangan seperti rewang, nilai negatif
  menyongsangkannya), penapis hingar, dan bila giro digunakan (sentiasa, mati
  atau hanya semasa `+gyroaction` ditahan). **Gaya Bidikan** menukar stik
  kanan antara bidikan 2D (ia memusingkan pandangan secara berterusan) dan
  **flick stick** (tolak stik untuk menghalakan pandangan ke arah itu, kemudian
  pusingkan stik untuk memutarnya); kedua-duanya sesuai digabungkan dengan
  giro untuk bidikan halus.
- **Getaran**, kepekaan stik dan penyongsangan.
- **Simbol Butang** sepadan dengan pad anda (Xbox, PlayStation, Nintendo) dan
  dikesan secara automatik.
- Ikat semula semuanya dalam Options → Kawalan, termasuk dari alat kawalan.
- Kawalan skrin sentuh dengan saiz, kelegapan dan kelajuan pandangan yang
  boleh dilaraskan.

## Menyimpan

- **Simpanan Auto** berputar berasingan daripada simpanan manual dan simpan
  pantas, jadi ia tidak pernah menulis ganti simpanan anda sendiri. Options →
  Permainan → Tetapan Simpan Auto memilih berapa banyak yang dikekalkan dan
  bila menyimpan: semasa masuk peringkat, mengikut pemasa, apabila menemui
  rahsia atau semua rahsia, dan apabila membunuh setiap musuh.
- **Kad simpanan** dalam menu Muat/Simpan menunjukkan peringkat, kesihatan,
  perisai, senjata, kunci dan bunuhan/rahsia bagi setiap simpanan.
- **Bila Mati** (Options → Permainan → Bila Mati), pemain tunggal boleh
  bertanya apa yang perlu dilakukan dalam menu yang dijeda, memuat simpanan
  terbaru, membiarkan anda memilih simpanan, atau memulakan semula peringkat.

## Permainan dan kamera

Options → Permainan:

- Medan pandangan 60–130, ayunan pandangan, sentakan senjata, sentiasa
  berlari.
- **Kamera**: orang pertama, orang ketiga, atau atas bahu (kanan atau
  kiri).
- Kedudukan **Model Senjata**: tengah, kanan, kiri atau tersembunyi.

## Ubah Suai: pengubah suai permainan dan tambahan

Sesuatu permainan sama ada *asal* atau *diubah suai*, dipilih semasa ia
bermula. New Game (dan memilih peringkat dalam Pilih Peringkat atau Pek Peta)
memaparkan tetingkap timbul: **Main** memulakan permainan, **Ubah Suai
Pengalaman** membuka Pengubah Suai Permainan dan Tambahan. Selagi ada yang
diubah, Main ditanda **UBAH** dan **Matikan Semua Pengubahsuaian** (Del,
pad permainan X, atau klik) mengembalikan peraturan standard tanpa tambahan.
Permainan asal menyimpan fail `.sav` biasa yang boleh dimuat oleh ironwail
dan vkQuake; permainan diubah suai menyimpan fail `.qsa` yang hanya dimuat
oleh port ini (begitu juga permainan skrin pisah dan koperatif, yang tidak
dapat disimpan oleh port lain). Semasa permainan diubah suai, Menu utama →
Ubah Suai menukar pengubah suai dan tambahannya pada bila-bila masa;
peraturan permainan asal kekal tetap. Pengubah suai berfungsi dengan
sebarang mod dan dimatikan dalam deathmatch.

- Lebih sukar: lebih banyak kerosakan diterima, musuh lebih kuat, kurang
  kutipan peluru dan kesihatan, raksasa 2–4x, mula kosong setiap peringkat,
  tiada simpan manual.
- Lebih mudah: kurang kerosakan diterima, musuh lebih lemah, lebih banyak
  peluru dan kesihatan, peluru tanpa had, pemulihan kesihatan, tiada
  kerosakan jatuh, tidak lemas, tiada kerosakan sendiri, lompatan berganda
  atau tiga kali.

## Demo

`record <name>` dan `stop` merakam demo ke dalam folder pengguna anda;
`playdemo <name>` memainkannya. Apabila rakaman berhenti, demo ditulis semula
di latar belakang supaya ironwail dan vkQuake juga boleh memainkannya. Jika
sesuatu mod menggunakan kesan zarah yang tidak dapat dipaparkan oleh port
lain, versi penuh disimpan sebagai `<name>.qsd` (`playdemo <name>.qsd`).
`demo_convert <name>` menukar demo yang lebih lama.

## Kebolehcapaian

Options → Kebolehcapaian:

- Gerakan dikurangkan dan keamatan kilatan skrin.
- Penapis buta warna.
- Rerambut silang boleh dikonfigurasi: bentuk, saiz, garis luar, dan
  perubahan warna apabila pada sasaran atau semasa membunuh; penanda kena
  pilihan.
- **Sari Kata Bunyi** dan tetapan mesej (Mesej & Sari Kata), skala teks.
- Serang dan lari secara togol (bukannya tahan).
- Peleret kelajuan permainan untuk memperlahankan seluruh permainan.
- Bidikan auto dengan penunjuk kunci sasaran, serta tetapan kon dan jarak.

## Peningkatan

Options → Peningkatan. Semuanya di bahagian klien dan dimatikan secara
lalai, jadi ia berfungsi dengan sebarang mod dan dalam talian.

- **Serpihan**: cebisan daging yang bergolek dan serpihan daripada tong yang
  pecah, juga dengan raksasa mod; ia boleh berlanggar dengan pintu dan lif.
- **Darah dipertingkat**: percikan pada permukaan, luka yang mengikut badan,
  titisan di udara yang memercik di tempat ia jatuh.
- **Kesan hentaman**: kesan peluru senapang patah, paku yang tertancap pada
  kayu dan musuh, kesan hangus letupan.
- **Air Dipertingkat**: lut cahaya, riak, percikan dan kaustik.
- **Teleporter Lut Sinar** yang menunjukkan destinasinya dengan paralaks.
- Oklusi ambien, kabur gerakan bagi setiap objek dan kelajuan, zarah lembut.
- **Shader Skrin**: pratetap RetroArch `.slangp` (CRT dan lain-lain).

Options → Grafik juga mempunyai skala render, anti-alias, supersampling,
penapisan tekstur, warna 8-bit dengan dithering, kabus, pencahayaan berwarna
dan interpolasi animasi/pergerakan yang biasa.

## Tambahan

Tambahan ialah senjata dan kelengkapan baharu, dijalankan oleh enjin supaya
ia berfungsi dengan progs mana-mana mod. Ia untuk pemain tunggal dan
koperatif sahaja.

### Memulakan permainan dengan tambahan

Sesuatu permainan mempunyai tambahan hanya jika ia dimulakan dengannya:

1. Single Player → tandakan kotak **Tambahan**, kemudian New Game.
2. Pilih tambahan yang hendak dibawa dan laraskan tetapannya, kemudian Mula
   Permainan.

Permainan dengan tambahan disimpan sebagai fail `.qsa` dan bukannya `.sav`,
supaya port lain tidak cuba memuatkannya. Kad simpanan memaparkan lencana
"ADD-ONS". Memuat simpanan biasa mematikan tambahan. Semasa bermain
permainan dengan tambahan, halaman Tambahan (konsol: `menu_options addons`)
menghidupkan dan mematikan setiap tambahan; setiap satu mempunyai pratonton
bergelung, arahan, ikatan kekunci dan tetapannya sendiri.

### Senjata

Tambahan dimasukkan ke dalam slot senjata biasa (pilih bagi setiap tambahan,
atau slot 9 dan 0) dan muncul pada roda senjata dan HUD. Tembak dan tembakan
alt ialah butang serang biasa anda.

| Tambahan | Tembak | Tembakan Alt | Peluru |
|---|---|---|---|
| **Senapang Portal** | Letak portal biru | Letak portal jingga | Tiada |
| **Railgun** | Peluru penembus yang mengena semua dalam satu garisan | — | 2 sel |
| **Cangkuk** | Melekat dan tarik diri anda ke kayu, logam, batu atau musuh | — | Tiada |
| **Pelancar Bom Lekit** | Tahan untuk mengecas, lepaskan untuk melancar | Letupkan | 1 roket |
| **Bumerang** | Tahan untuk menanda musuh, lepaskan untuk membaling; ia memengsankan setiap satu bergilir-gilir | Kosongkan tanda | Tiada |
| **Penyembur Gel** | Gel tolakan biru (melantun) | Gel pecutan jingga (laju) | Tiada |
| **Pencuci Tekanan** | Pancutan air panjang yang menolak musuh dan mencuci gel | Kipas pancutan lebar | Tiada |

Portal diletakkan pada permukaan dunia yang rata dan membawa anda, raksasa
dan objek yang dibaling melaluinya bersama momentumnya. Dengan **Lihat
Menembusi Portal** dihidupkan, anda melihat sebelah sana secara langsung,
termasuk portal yang kelihatan melalui portal; kualiti, bilangan paparan
langsung, jarak dan kedalaman rekursi boleh dilaraskan.

### Kelengkapan

Kelengkapan dihidupkan dan dimatikan, bukannya dipegang. Buka **roda
kelengkapan** (`+equipwheel`), atau guna `goggles cycle` untuk bertukar
antara:

- **Penglihatan malam**
- **Penglihatan terma**

## Berbilang pemain

Selain permainan NetQuake melalui LAN dan internet (protokol 15, 666 dan
999), port ini boleh bersambung ke pelayan **QuakeWorld**. Multiplayer →
Join Game menerima alamat atau mencari dalam LAN. Zarah skrip FTE disokong
untuk mod yang menggunakannya.

## Baris arahan dan konsol

Pilihan biasa: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, dan
`+command args` untuk menjalankan arahan konsol semasa permulaan. Konsol
(`~`) menerima arahan dan cvar Quake yang biasa.
