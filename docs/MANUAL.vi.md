# Hướng dẫn sử dụng

Tài liệu này hướng dẫn chơi Quake với bản port này, tập trung vào những gì
nó bổ sung so với Quake gốc và các bản port khác. Mọi tính năng mới đều là
tùy chọn: với cài đặt mặc định, trò chơi trông và chơi giống hệt Quake.

## Bắt đầu

1. Hãy có bản Quake của riêng bạn. Trỏ bản port tới thư mục chứa `id1/`
   (và các mod nằm cạnh nó), bằng `QUAKE_BASEDIR` hoặc
   `-basedir <thư mục>`.
2. Chạy `cargo run --release`, hoặc khởi động thẳng một mod bằng
   `-game <thư mục> +map <bản đồ>`.
3. Cấu hình, bản lưu và ảnh chụp màn hình nằm trong
   `~/.local/share/quake-source-port/<game>/`, mỗi mod một thư mục.

Menu chính có Single Player, Multiplayer, Options, **Help** (tài liệu
này), Quit và **Gói bản đồ**, với nhạc chủ đề Quake phát phía sau
(Options → Âm thanh → Nhạc menu chính). Mọi menu đều dùng được bằng chuột,
bàn phím, tay cầm và cảm ứng; di chuột lên hoặc chọn một tùy chọn để xem
chú thích giải thích nó.

Trong Help, lên/xuống chọn một mục; Page Up/Page Down, trái/phải, các nút
vai hoặc con lăn chuột trên phần chữ sẽ cuộn nội dung.

## Ngôn ngữ

Options → Lối chơi → **Ngôn ngữ** chọn ngôn ngữ của menu, thông báo trên
màn hình và tài liệu này: tiếng Anh, Pháp, Đức, Ý, Tây Ban Nha, Nga, Ba
Lan, Ukraina, Séc, Slovakia, Croatia, Bulgaria, Thụy Điển, Phần Lan, Đan
Mạch, Na Uy, Hà Lan, Hy Lạp, Hungary, Romania, Bồ Đào Nha (Bồ Đào Nha hoặc
Brazil), Nhật, Hàn, Trung (Giản thể hoặc Phồn thể), Thổ Nhĩ Kỳ, Indonesia,
Mã Lai, Filipino, Việt, Serbia, Litva, Latvia, Estonia, Malta, Albania,
Armenia, Gruzia, Iceland, Ireland, Wales, Basque, Catalan, Galicia, Latinh,
Esperanto, Belarus, Macedonia, Slovenia, Swahili, Ả Rập, Do Thái, Ba Tư,
Urdu, Hindi, Marathi, Bengal, Punjab, Gujarat, Tamil, Telugu, Thái, Khmer,
Amhara hoặc Quảng Đông.
**Hệ thống** theo ngôn ngữ máy tính của bạn. Văn bản trò chơi riêng của
bản phát hành lại 2021 chỉ được dịch sang tiếng Pháp, Đức, Ý, Tây Ban Nha
và Nga; các ngôn ngữ khác hiển thị nó bằng tiếng Anh. Với các ngôn ngữ khác
tiếng Anh, chữ nhỏ trong trò chơi dùng phông chữ của menu, vì phông chữ
Quake cổ điển chỉ có chữ Latinh không dấu. Thông báo console vẫn bằng
tiếng Anh trừ khi bật **Dịch console**.

## Gói bản đồ (trình duyệt mod)

Menu chính → Gói bản đồ liệt kê mọi mod và gói bản đồ đã cài cùng phần mô
tả. Chọn một mục để chuyển sang nó, rồi duyệt các bản đồ và bắt đầu bất kỳ
bản đồ nào ngay lập tức. Trình duyệt ghi lại số hạ gục, bí mật và thời gian
tốt nhất của bạn trên từng bản đồ. Mỗi mod có cấu hình riêng, nên phím gán
và cài đặt chỉnh cho mod này không lẫn sang mod khác.

**Đặt gói bản đồ ở đâu.** Bản port tìm chúng trong thư mục gốc của Quake
(thư mục chứa `id1/`, đặt bằng `QUAKE_BASEDIR` hoặc `-basedir`). Mọi thư
mục con trong đó có dữ liệu trò chơi (tệp `.pak` hoặc `.pk3`, một
`progs.dat`, hoặc thư mục `maps/`) đều được liệt kê, vì vậy hãy cài gói bản
đồ bằng cách giải nén nó vào thư mục riêng cạnh `id1/`, ví dụ
`<basedir>/ad/` hoặc `<basedir>/my_maps/maps/*.bsp`. Có thể thêm tệp
`modinfo.txt` với các dòng `title`, `base` và `description` để điều chỉnh
cách nó được liệt kê. Các gói được tìm khi trò chơi khởi động, nên hãy khởi
động lại sau khi thêm gói mới. Bản lưu, ảnh chụp màn hình và cấu hình của
mỗi gói nằm trong `~/.local/share/quake-source-port/<thư mục>/`, không nằm
trong thư mục của gói.

## Chia màn hình

Tối đa sáu người chơi trên một máy, ở chế độ co-op hoặc deathmatch.

- Multiplayer → New Game → **Người chơi cục bộ** đặt số người chơi.
- Mỗi người chơi có khung nhìn, HUD, âm thanh và thiết bị nhập riêng (bàn
  phím và chuột cho người chơi 1, mỗi người còn lại một tay cầm).
- Bố cục màn hình có thể xếp chồng, cạnh nhau, 2×2 hoặc 3×2
  (`r_splitscreen_layout`).
- Mỗi người chơi chọn tên và màu áo/quần trong Multiplayer → Setup.

## Tay cầm

Bản port được thiết kế để chơi bằng tay cầm.

- **Vòng vũ khí** (`+weaponwheel`): giữ để làm chậm thời gian và chọn vũ
  khí bằng một trong hai cần hoặc bằng chuột; thả ra để đổi.
- **Hỗ trợ ngắm** (Options → Thiết bị nhập → Tay cầm): giảm tốc khi gần kẻ
  địch và xoay theo chúng. Nó chỉ áp dụng cho cần điều khiển, không bao giờ
  cho chuột.
- **Ngắm bằng con quay** (Options → Thiết bị nhập → Tay cầm → Con quay):
  một công tắc bật/tắt, cùng độ nhạy riêng cho các trục ngang, dọc và
  nghiêng (trục nghiêng mặc định tắt và xoay góc nhìn như trục ngang, giá
  trị âm để đảo chiều), bộ lọc nhiễu, và khi nào con quay có tác dụng (luôn
  luôn, tắt hoặc chỉ khi giữ `+gyroaction`). **Kiểu ngắm** chuyển cần phải
  giữa ngắm 2D (xoay góc nhìn liên tục) và **flick stick** (đẩy cần để
  hướng ngay góc nhìn về phía đó, rồi xoay cần để xoay tiếp); cả hai kết
  hợp tốt với con quay để ngắm chính xác.
- **Rung**, độ nhạy và đảo chiều cần.
- **Biểu tượng nút** khớp với tay cầm của bạn (Xbox, PlayStation, Nintendo)
  và được nhận diện tự động.
- Gán lại mọi thứ trong Options → Điều khiển, kể cả ngay từ tay cầm.
- Điều khiển cảm ứng với kích cỡ, độ mờ đục và tốc độ nhìn có thể chỉnh.

## Lưu trò chơi

- **Bản tự lưu** xoay vòng riêng với bản lưu thủ công và bản lưu nhanh, nên
  không bao giờ ghi đè bản lưu của bạn. Options → Lối chơi → Cài đặt tự động
  lưu chọn số bản giữ lại và lúc nào lưu: khi vào màn, theo hẹn giờ, khi tìm
  thấy một hoặc tất cả bí mật, và khi hạ hết kẻ địch.
- **Thẻ bản lưu** trong menu Tải/Lưu hiển thị màn chơi, máu, giáp, vũ khí,
  chìa khóa và số hạ gục/bí mật của từng bản lưu.
- **Khi chết** (Options → Lối chơi → Khi chết), chế độ chơi đơn có thể hỏi
  bạn muốn làm gì trong một menu tạm dừng, tải bản lưu mới nhất, cho bạn
  chọn bản lưu, hoặc chơi lại màn.

## Lối chơi và camera

Options → Lối chơi:

- Trường nhìn 60–130, nhún góc nhìn, độ giật súng, luôn chạy.
- **Camera**: ngôi thứ nhất, ngôi thứ ba, hoặc qua vai (phải hoặc trái).
- Vị trí **mô hình vũ khí**: giữa, phải, trái hoặc ẩn.

## Tùy chỉnh: điều chỉnh lối chơi và tiện ích

Một ván chơi hoặc là *nguyên bản* hoặc là *tùy chỉnh*, được chọn khi bắt
đầu. New Game (và việc chọn màn trong Chọn màn hoặc Gói bản đồ) hiện một
hộp thoại: **Chơi** bắt đầu trò chơi, **Tùy chỉnh trải nghiệm** mở Điều
chỉnh lối chơi và Tiện ích. Khi có bất cứ thay đổi nào, Chơi được gắn nhãn
**TÙY CHỈNH** và **Tắt mọi thay đổi** (Del, nút X trên tay cầm, hoặc nhấp
chuột) đưa về luật chuẩn không có tiện ích. Ván nguyên bản lưu tệp `.sav`
thông thường mà ironwail và vkQuake tải được; ván tùy chỉnh lưu tệp `.qsa`
mà chỉ bản port này tải được (ván chia màn hình và co-op cũng vậy, vì các
bản port khác không lưu được chúng). Trong một ván tùy chỉnh, Menu chính →
Tùy chỉnh đổi điều chỉnh và tiện ích của ván bất cứ lúc nào; luật của ván
nguyên bản luôn cố định. Điều chỉnh hoạt động với mọi mod và tự tắt trong
deathmatch.

- Khó hơn: nhận nhiều sát thương hơn, kẻ địch trâu hơn, nhặt được ít đạn
  và máu hơn, quái vật x2–x4, bắt đầu tay trắng mỗi màn, không lưu thủ
  công.
- Dễ hơn: nhận ít sát thương hơn, kẻ địch yếu hơn, nhiều đạn và máu hơn, vô
  hạn đạn, hồi máu, không sát thương rơi, không chết đuối, không tự gây sát
  thương, nhảy đôi hoặc nhảy ba.

## Demo

`record <tên>` và `stop` ghi một demo vào thư mục người dùng của bạn;
`playdemo <tên>` phát lại nó. Khi dừng ghi, demo được viết lại trong nền để
ironwail và vkQuake cũng phát được. Nếu một mod dùng hiệu ứng hạt mà các
bản port khác không hiển thị được, phiên bản đầy đủ được giữ lại dưới tên
`<tên>.qsd` (`playdemo <tên>.qsd`). `demo_convert <tên>` chuyển đổi một
demo cũ hơn.

Menu chính → Demo liệt kê các demo đã lưu và, khi bật Tùy chọn → Demo → Tự
động ghi demo, một demo tự động của mỗi màn bạn chơi; các bản cũ nhất bị
thay khi các ô đã đầy. K giữ lại một bản, E xuất nó thành video và Del xóa
nó. Báo cáo lỗi có kèm demo tự động của màn hiện tại.

Khi đang phát demo, Space tạm dừng, giữ Trái hoặc Phải để tua lại hoặc tua
nhanh, Lên và Xuống đổi tốc độ, PgUp và PgDn nhảy 10 giây, còn Home và End
về đầu và sang bản đồ kế tiếp. I và O đánh dấu một đoạn; E xuất đoạn đó,
hoặc cả demo, vào thư mục `videos`. Thanh điều khiển luôn hiện trên màn
hình; H hoặc Ẩn sẽ ẩn nó, và trỏ vào mép dưới sẽ hiện một dải để gọi nó lại.
Bấm các nút, bấm vào dòng thời gian để nhảy tới đó, hoặc kéo trên nó để đánh
dấu một đoạn; Xuất sẽ hỏi tốc độ khung hình, kích thước, định dạng và tệp,
và xuất đoạn đã đánh dấu hay toàn bộ demo.

`capture` bắt đầu và dừng ghi video khi bạn chơi. Tốc độ khung hình, định
dạng và chất lượng nằm trong Tùy chọn → Demo → Ghi video. Video cần ffmpeg;
không có nó thì khung hình được lưu thành ảnh PNG kèm một tệp WAV.

## Trợ năng

Options → Trợ năng:

- Giảm chuyển động và cường độ chớp màn hình.
- Bộ lọc cho người mù màu.
- Tâm ngắm tùy chỉnh: hình dạng, kích cỡ, viền, và đổi màu khi nhắm trúng
  hoặc khi hạ gục; dấu trúng tùy chọn.
- **Phụ đề âm thanh** và cài đặt thông báo (Thông báo & Phụ đề), phóng to
  chữ.
- Tấn công và chạy kiểu bật/tắt (thay vì giữ).
- Thanh trượt tốc độ trò chơi để làm chậm toàn bộ trò chơi.
- Tự động ngắm với chỉ báo khóa mục tiêu, cài đặt nón và tầm.

## Cải tiến

Options → Cải tiến. Tất cả chạy phía máy khách và mặc định tắt, nên chúng
hoạt động với mọi mod và khi chơi trực tuyến.

- **Mảnh vỡ**: mảnh thịt lăn lộn và mảnh văng từ thùng vỡ, kể cả với quái
  vật của mod; chúng có thể va vào cửa và thang máy.
- **Vết máu nâng cao**: vết máu bắn trên bề mặt, vết thương bám theo cơ
  thể, giọt máu bay trong không khí và tóe ra nơi chúng rơi xuống.
- **Hiệu ứng va chạm**: vết đạn súng săn, đinh cắm vào gỗ và kẻ địch, vết
  cháy do nổ.
- **Nước nâng cao**: trong suốt, gợn sóng, bắn tóe và tụ quang.
- **Máy dịch chuyển nhìn xuyên** hiện điểm đến với hiệu ứng thị sai.
- Che bóng môi trường, nhòe chuyển động theo vật thể và theo tốc độ, hạt
  mềm.
- **Shader màn hình**: thiết lập sẵn `.slangp` của RetroArch (CRT và nhiều
  loại khác).

Options → Đồ họa còn có các cài đặt quen thuộc: tỷ lệ kết xuất, khử răng
cưa, siêu lấy mẫu, lọc texture, màu 8-bit với hòa sắc, sương mù, ánh sáng
màu và nội suy hoạt ảnh/chuyển động.

## Tiện ích

Tiện ích là vũ khí và trang bị mới do engine điều khiển, nên chúng hoạt
động với progs của mọi mod. Chúng chỉ có trong chế độ chơi đơn và co-op.

### Bắt đầu ván chơi có tiện ích

Một ván chơi chỉ có tiện ích nếu nó bắt đầu cùng chúng:

1. Single Player → đánh dấu ô **Tiện ích**, rồi New Game.
2. Chọn tiện ích muốn mang theo và chỉnh cài đặt của chúng, rồi bắt đầu
   ván mới.

Ván có tiện ích lưu thành tệp `.qsa` thay vì `.sav`, để các bản port khác
không thử tải chúng. Thẻ bản lưu hiện huy hiệu "ADD-ONS". Tải một bản lưu
thường sẽ tắt tiện ích. Khi đang chơi ván có tiện ích, trang Tiện ích
(console: `menu_options addons`) bật và tắt từng tiện ích; mỗi tiện ích có
bản xem trước lặp lại, hướng dẫn, phím gán và cài đặt riêng.

### Vũ khí

Tiện ích nằm trong các ô vũ khí thông thường (chọn ô cho từng tiện ích,
hoặc ô 9 và 0) và xuất hiện trên vòng vũ khí và HUD. Bắn và bắn phụ là các
nút tấn công quen thuộc của bạn.

| Tiện ích | Bắn | Bắn phụ | Đạn |
|---|---|---|---|
| **Súng cổng** | Đặt cổng màu xanh | Đặt cổng màu cam | Không |
| **Railgun** | Viên đạn xuyên phá trúng mọi thứ trên một đường thẳng | — | 2 pin |
| **Móc kéo** | Bám vào gỗ, kim loại, đá hoặc kẻ địch và kéo bạn tới đó | — | Không |
| **Súng phóng bom dính** | Giữ để nạp, thả để phóng | Kích nổ | 1 tên lửa |
| **Boomerang** | Giữ để đánh dấu kẻ địch, thả để ném; nó lần lượt làm choáng từng kẻ | Xóa dấu | Không |
| **Súng phun gel** | Gel đẩy bật màu xanh (nảy) | Gel tăng tốc màu cam (nhanh) | Không |
| **Máy xịt rửa** | Tia nước dài đẩy kẻ địch và rửa sạch gel | Hình quạt tia nước rộng | Không |

Cổng được đặt trên bề mặt phẳng của thế giới và đưa bạn, quái vật và vật bị
ném đi xuyên qua mà vẫn giữ nguyên đà. Khi bật **Nhìn xuyên cổng**, bạn
thấy trực tiếp phía bên kia, kể cả các cổng nhìn thấy qua cổng; chất lượng,
số khung nhìn trực tiếp, khoảng cách và độ sâu đệ quy đều có thể chỉnh.

### Trang bị

Trang bị được bật và tắt chứ không cầm trên tay. Mở **vòng trang bị**
(`+equipwheel`), hoặc dùng `goggles cycle` để chuyển lần lượt:

- **Nhìn đêm**
- **Tầm nhiệt**

## Chơi mạng

Ngoài chơi NetQuake qua LAN và Internet (giao thức 15, 666 và 999), bản
port còn kết nối tới máy chủ **QuakeWorld**. Multiplayer → Join Game nhận
một địa chỉ hoặc tìm trong mạng LAN. Hạt theo script của FTE được hỗ trợ
cho các mod sử dụng chúng.

## Dòng lệnh và console

Các tùy chọn thường dùng: `-basedir <thư mục>`, `-game <thư mục>`,
`-hipnotic` / `-rogue` / `-quoth`, `-width <r>`, `-height <c>`,
`-fullscreen`, `-window`, và `+lệnh tham_số` để chạy lệnh console khi khởi
động. Console (`~`) nhận các lệnh và cvar quen thuộc của Quake.
