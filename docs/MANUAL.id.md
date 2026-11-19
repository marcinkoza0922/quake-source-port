# Panduan pengguna

Panduan ini menjelaskan cara memainkan Quake dengan port ini, terutama apa
saja yang ditambahkannya dibanding Quake vanilla dan source port lain. Semua
fitur baru bersifat opsional: dengan pengaturan bawaan, game ini terlihat
dan dimainkan seperti Quake.

## Memulai

1. Siapkan salinan Quake milik Anda sendiri. Arahkan port ke folder yang
   berisi `id1/` (beserta mod di sebelahnya), dengan `QUAKE_BASEDIR` atau
   `-basedir <folder>`.
2. Jalankan `cargo run --release`, atau langsung mulai sebuah mod dengan
   `-game <folder> +map <peta>`.
3. Konfigurasi, simpanan, dan tangkapan layar disimpan di
   `~/.local/share/quake-source-port/<game>/`, satu folder per mod.

Menu utama berisi Single Player, Multiplayer, Options, **Help** (panduan
ini), Quit, dan **Paket Peta**, dengan tema Quake diputar di latar belakang
(Options → Audio → Musik Menu Utama). Setiap menu bisa dipakai dengan mouse,
keyboard, kontroler, dan layar sentuh; arahkan kursor ke atau pilih sebuah
opsi untuk melihat tooltip yang menjelaskannya.

Di Help, atas/bawah memilih bagian; Page Up/Page Down, kiri/kanan, tombol
bahu, atau roda mouse di atas teks akan menggulirnya.

## Bahasa

Options → Gameplay → **Bahasa** memilih bahasa menu, pesan di layar, dan
panduan ini: Inggris, Prancis, Jerman, Italia, Spanyol, Rusia, Polandia,
Ukraina, Ceko, Slowakia, Kroasia, Bulgaria, Swedia, Finlandia, Denmark,
Norwegia, Belanda, Yunani, Hungaria, Rumania, Portugis (Portugal atau
Brasil), Jepang, Korea, Tionghoa (Sederhana atau Tradisional), Turki,
Indonesia, Melayu, Filipino, Vietnam, Serbia, Lituania, Latvia, Estonia,
Malta, Albania, Armenia, Georgia, Islandia, Irlandia, Wales, Basque,
Katalan, Galisia, Latin, Esperanto, Belarus, Makedonia, Slovenia, Swahili,
Arab, Ibrani, Persia, Urdu, Hindi, Marathi, Bengali, Punjabi, Gujarat,
Tamil, Telugu, Thai, Khmer, Amhara, atau Kanton.
**Sistem** mengikuti bahasa komputer Anda. Teks game bawaan rilis ulang 2021
hanya diterjemahkan ke bahasa Prancis, Jerman, Italia, Spanyol, dan Rusia;
bahasa lain menampilkannya dalam bahasa Inggris. Dalam bahasa selain
Inggris, teks kecil di dalam game memakai fon menu, karena fon Quake klasik
hanya memiliki huruf Latin tanpa aksen. Pesan konsol tetap dalam bahasa
Inggris kecuali **Terjemahkan Konsol** aktif.

## Paket Peta (penjelajah mod)

Menu utama → Paket Peta menampilkan setiap mod dan paket peta yang
terpasang beserta deskripsinya. Pilih salah satu untuk beralih ke sana, lalu
telusuri petanya dan mulai peta mana pun secara langsung. Penjelajah ini
mencatat jumlah bunuh, rahasia, dan waktu terbaik Anda per peta. Setiap mod
punya konfigurasi sendiri, jadi penetapan tombol dan pengaturan yang
disesuaikan untuk satu mod tidak terbawa ke mod lain.

**Letak paket peta.** Port mencarinya di folder dasar Quake (yang berisi
`id1/`, diatur dengan `QUAKE_BASEDIR` atau `-basedir`). Setiap subfolder di
sana yang berisi data game (berkas `.pak` atau `.pk3`, `progs.dat`, atau
folder `maps/`) akan ditampilkan, jadi pasang paket peta dengan
mengekstraknya ke foldernya sendiri di sebelah `id1/`, misalnya
`<basedir>/ad/` atau `<basedir>/my_maps/maps/*.bsp`. Secara opsional,
tambahkan `modinfo.txt` dengan baris `title`, `base`, dan `description`
untuk mengatur cara paket itu ditampilkan. Paket dicari saat game dimulai,
jadi mulai ulang game setelah menambahkannya. Simpanan, tangkapan layar, dan
konfigurasi tiap paket disimpan di
`~/.local/share/quake-source-port/<folder>/`, bukan di folder paket.

## Layar terbagi

Hingga enam pemain di satu komputer, dalam co-op atau deathmatch.

- Multiplayer → New Game → **Pemain Lokal** menentukan jumlah pemain.
- Setiap pemain mendapat pandangan, HUD, suara, dan perangkat input sendiri
  (keyboard dan mouse untuk pemain 1, satu kontroler untuk masing-masing
  pemain lainnya).
- Tata letak layar bisa bertumpuk, berdampingan, 2×2, atau 3×2
  (`r_splitscreen_layout`).
- Setiap pemain memilih nama serta warna baju/celana di Multiplayer →
  Setup.

## Kontroler

Port ini dirancang untuk dimainkan dengan gamepad.

- **Roda Senjata** (`+weaponwheel`): tahan untuk memperlambat waktu dan
  memilih senjata dengan stik mana pun atau mouse; lepas untuk beralih.
- **Bantuan Bidik** (Options → Input → Kontroler): perlambatan di dekat
  musuh dan rotasi yang mengikuti mereka. Fitur ini hanya berlaku untuk
  stik, tidak pernah untuk mouse.
- **Bidik Giroskop** (Options → Input → Kontroler → Giroskop): sakelar,
  ditambah sensitivitas terpisah untuk sumbu yaw, pitch, dan roll (roll
  nonaktif secara bawaan dan memutar pandangan seperti yaw, nilai negatif
  membalikkannya), filter derau, dan kapan giroskop berlaku (selalu,
  nonaktif, atau hanya saat `+gyroaction` ditahan). **Gaya Bidik** mengubah
  stik kanan antara bidikan 2D (memutar pandangan terus-menerus) dan
  **flick stick** (dorong stik untuk langsung mengarahkan pandangan ke arah
  itu, lalu putar stik untuk merotasinya); keduanya berpadu baik dengan
  giroskop untuk bidikan halus.
- **Getaran**, sensitivitas stik, dan pembalikan.
- **Simbol Tombol** menyesuaikan kontroler Anda (Xbox, PlayStation,
  Nintendo) dan terdeteksi otomatis.
- Atur ulang semua tombol di Options → Kontrol, termasuk dari kontroler.
- Kontrol layar sentuh dengan ukuran, opasitas, dan kecepatan pandangan yang
  bisa diatur.

## Menyimpan

- **Simpanan otomatis** dirotasi terpisah dari simpanan manual dan simpan
  cepat, sehingga tidak pernah menimpa simpanan Anda sendiri. Options →
  Gameplay → Pengaturan Simpan Otomatis menentukan berapa banyak yang
  disimpan dan kapan menyimpan: saat masuk level, berdasarkan interval, saat
  menemukan satu atau semua rahasia, dan saat semua musuh tewas.
- **Kartu simpanan** di menu Muat/Simpan menampilkan level, kesehatan,
  zirah, senjata, kunci, serta bunuh/rahasia dari setiap simpanan.
- **Saat Mati** (Options → Gameplay → Saat Mati), mode pemain tunggal bisa
  menanyakan tindakan dalam menu jeda, memuat simpanan terbaru, membiarkan
  Anda memilih simpanan, atau mengulangi level.

## Gameplay dan kamera

Options → Gameplay:

- Bidang pandang 60–130, goyangan pandangan, hentakan senjata, selalu lari.
- **Kamera**: orang pertama, orang ketiga, atau di atas bahu (kanan atau
  kiri).
- Posisi **Model Senjata**: tengah, kanan, kiri, atau disembunyikan.

## Sesuaikan: pengubah gameplay dan add-on

Sebuah game bisa *vanilla* atau *disesuaikan*, dipilih saat game dimulai.
New Game (dan memilih level di Pilih Level atau Paket Peta) menampilkan
jendela pop-up: **Main** memulai game, **Sesuaikan Pengalaman** membuka
Pengubah Gameplay dan Add-on. Selama ada yang diubah, Main diberi tanda
**KUSTOM** dan **Nonaktifkan Semua Modifikasi** (Del, tombol X gamepad, atau
klik) mengembalikan aturan standar tanpa add-on. Game vanilla menyimpan
berkas `.sav` biasa yang bisa dimuat ironwail dan vkQuake; game yang
disesuaikan menyimpan berkas `.qsa` yang hanya bisa dimuat port ini (begitu
pula game layar terbagi dan co-op, yang tidak bisa disimpan oleh port lain).
Selama game yang disesuaikan berjalan, Menu utama → Sesuaikan bisa mengubah
pengubah dan add-on-nya kapan saja; aturan game vanilla tetap. Pengubah
berfungsi dengan mod apa pun dan nonaktif dalam deathmatch.

- Lebih sulit: kerusakan yang diterima lebih besar, musuh lebih tangguh,
  amunisi dan kesehatan yang diambil lebih sedikit, monster 2–4x, mulai dari
  nol di setiap level, tanpa simpan manual.
- Lebih mudah: kerusakan yang diterima lebih kecil, musuh lebih lemah,
  amunisi dan kesehatan lebih banyak, amunisi tak terbatas, regenerasi
  kesehatan, tanpa kerusakan jatuh, tanpa tenggelam, tanpa melukai diri,
  lompatan ganda atau tripel.

## Demo

`record <nama>` dan `stop` merekam demo ke folder pengguna Anda; `playdemo
<nama>` memutarnya. Saat perekaman berhenti, demo ditulis ulang di latar
belakang agar ironwail dan vkQuake juga bisa memutarnya. Jika mod memakai
efek partikel yang tidak bisa ditampilkan port lain, versi lengkapnya
disimpan sebagai `<nama>.qsd` (`playdemo <nama>.qsd`). `demo_convert <nama>`
mengonversi demo lama.

Menu utama → Demo menampilkan demo tersimpan Anda dan, jika Opsi → Demo →
Rekam demo otomatis menyala, demo otomatis dari setiap level yang Anda
mainkan; yang tertua diganti saat slot penuh. K menyimpan satu, E
mengekspornya ke video, dan Del menghapusnya. Laporan bug menyertakan demo
otomatis level saat ini.

Saat demo diputar, Spasi menjeda, menahan Kiri atau Kanan memutar mundur
atau maju cepat, Atas dan Bawah mengubah kecepatan, PgUp dan PgDn melompat
10 detik, serta Home dan End menuju awal dan peta berikutnya. I dan O
menandai klip; E mengekspornya, atau seluruh demo, ke folder `videos`.
Kontrol tetap di layar; H atau Sembunyikan menyembunyikannya, dan menunjuk
tepi bawah memunculkan strip yang mengembalikannya. Klik tombolnya, klik
linimasa untuk melompat ke sana, atau seret di atasnya untuk menandai klip;
Ekspor menanyakan laju bingkai, ukuran, format, dan file, serta apakah
mengekspor klip yang ditandai atau seluruh demo.

`capture` memulai dan menghentikan perekaman video saat bermain. Laju
bingkai, format, dan kualitas ada di Opsi → Demo → Perekaman video. Video
memerlukan ffmpeg; tanpanya bingkai disimpan sebagai gambar PNG dengan file
WAV.

## Aksesibilitas

Options → Aksesibilitas:

- Gerakan dikurangi dan intensitas kilatan layar.
- Filter buta warna.
- Crosshair yang bisa diatur: bentuk, ukuran, garis tepi, dan perubahan
  warna saat membidik target atau saat membunuh; penanda kena opsional.
- **Teks Suara** dan pengaturan pesan (Pesan & Teks), skala teks.
- Serang dan lari dengan mode tetap (bukan ditahan).
- Penggeser kecepatan game untuk memperlambat seluruh game.
- Bidik otomatis dengan indikator kunci target, pengaturan kerucut dan
  jangkauan.

## Peningkatan

Options → Peningkatan. Semuanya berjalan di sisi klien dan nonaktif secara
bawaan, sehingga berfungsi dengan mod apa pun dan saat daring.

- **Puing**: potongan daging yang berguling dan serpihan dari tong yang
  hancur, juga pada monster mod; puing bisa membentur pintu dan lift.
- **Darah yang ditingkatkan**: cipratan di permukaan, luka yang mengikuti
  tubuh, tetesan di udara yang memercik di tempat jatuhnya.
- **Efek hantaman**: bekas peluru shotgun, paku yang menancap di kayu dan
  musuh, bekas hangus ledakan.
- **Air Ditingkatkan**: tembus pandang, riak, cipratan, dan kaustik.
- **Teleporter Tembus Pandang** yang menampilkan tujuannya dengan paralaks.
- Ambient occlusion, motion blur per objek dan saat bergerak cepat,
  partikel lembut.
- **Shader Layar**: preset RetroArch `.slangp` (CRT dan lainnya).

Options → Grafis juga memiliki pengaturan umum seperti skala render,
anti-aliasing, supersampling, filter tekstur, warna 8-bit dengan dithering,
kabut, cahaya berwarna, serta interpolasi animasi/gerakan.

## Add-on

Add-on adalah senjata dan perlengkapan baru yang dijalankan oleh engine,
sehingga berfungsi dengan progs mod apa pun. Add-on hanya tersedia untuk
pemain tunggal dan co-op.

### Memulai game dengan add-on

Sebuah game hanya memiliki add-on jika dimulai dengan add-on:

1. Single Player → centang kotak **Add-on**, lalu New Game.
2. Pilih add-on yang ingin dibawa dan sesuaikan pengaturannya, lalu mulai
   game baru.

Game dengan add-on disimpan sebagai berkas `.qsa`, bukan `.sav`, agar port
lain tidak mencoba memuatnya. Kartu simpanan menampilkan lencana "ADD-ONS".
Memuat simpanan biasa akan menonaktifkan add-on. Selama memainkan game
dengan add-on, halaman Add-on (konsol: `menu_options addons`) mengaktifkan
dan menonaktifkan add-on satu per satu; masing-masing memiliki pratinjau
berulang, petunjuk, penetapan tombol, dan pengaturannya sendiri.

### Senjata

Add-on menempati slot senjata biasa (pilih per add-on, atau slot 9 dan 0)
dan muncul di roda senjata serta HUD. Tembak dan tembakan alt memakai tombol
serang Anda yang biasa.

| Add-on | Tembak | Tembakan Alt | Amunisi |
|---|---|---|---|
| **Portal Gun** | Memasang portal biru | Memasang portal oranye | Tidak ada |
| **Railgun** | Peluru penembus yang mengenai semua yang segaris | — | 2 sel |
| **Hookshot** | Mengait dan menarik Anda ke kayu, logam, batu, atau musuh | — | Tidak ada |
| **Peluncur Bom Lengket** | Tahan untuk mengisi, lepas untuk meluncurkan | Ledakkan | 1 roket |
| **Bumerang** | Tahan untuk menandai musuh, lepas untuk melempar; membuat mereka pingsan bergiliran | Hapus tanda | Tidak ada |
| **Penyemprot Gel** | Gel repulsi biru (memantul) | Gel propulsi oranye (cepat) | Tidak ada |
| **Powerwasher** | Semburan air panjang yang mendorong musuh dan membersihkan gel | Semburan lebar menyebar | Tidak ada |

Portal dipasang di permukaan datar dunia dan membawa Anda, monster, serta
benda yang dilempar melewatinya dengan momentumnya. Dengan **Lihat Tembus
Portal** aktif, Anda melihat sisi lain secara langsung, termasuk portal yang
terlihat melalui portal; kualitas, jumlah tampilan langsung, jarak, dan
kedalaman rekursi bisa diatur.

### Perlengkapan

Perlengkapan diaktifkan dan dinonaktifkan, bukan dipegang. Buka **Roda
Perlengkapan** (`+equipwheel`), atau gunakan `goggles cycle` untuk
bergantian:

- **Penglihatan malam**
- **Penglihatan termal**

## Multipemain

Selain permainan NetQuake lewat LAN dan internet (protokol 15, 666, dan
999), port ini dapat tersambung ke server **QuakeWorld**. Multiplayer → Join
Game menerima alamat atau mencari di LAN. Partikel skrip FTE didukung untuk
mod yang memakainya.

## Baris perintah dan konsol

Opsi umum: `-basedir <folder>`, `-game <folder>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <l>`, `-height <t>`, `-fullscreen`, `-window`, dan
`+perintah argumen` untuk menjalankan perintah konsol saat game dimulai.
Konsol (`~`) menerima perintah dan cvar Quake yang biasa.
