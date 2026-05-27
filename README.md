# lingine

**lingine** là một Cờ Tướng Engine hiệu năng cao được phát triển hoàn toàn bằng ngôn ngữ Rust.

Engine giao tiếp thông qua chuẩn **UCI** (tùy biến cho Cờ Tướng), cho phép dễ dàng tích hợp và thi đấu trên các GUI phổ biến.

---

## Nhóm phát triển

Dự án được thực hiện bởi nhóm sinh viên lớp Nhập môn Trí tuệ Nhân tạo:

1. **Trần Tuấn Anh** - MSSV: 202416124
2. **Lê Thành Trung** - MSSV: 202400076
3. **Bùi Tiến Dũng** - MSSV: 202416167

---

## Hướng dẫn cài đặt công cụ và môi trường thử nghiệm (ELO Testing)

Để chạy kiểm thử chất lượng (ELO Testing) cho **lingine** so với đối thủ tiêu chuẩn **Fairy-Stockfish** một cách chính xác và tái lập được kết quả (reproducible), bạn có thể tự động cài đặt và cấu hình toàn bộ công cụ bằng script tự động hóa được viết sẵn:

```bash
# Thực thi script tự động thiết lập
./scripts/setup_tools.sh
```

### Script trên sẽ thực hiện các bước sau:
1. **Tạo thư mục `tools/`** để quản lý các công cụ bổ trợ.
2. **Cài đặt Sylvan-CLI**: Phiên bản fork đặc biệt của `cutechess-cli` tối ưu hóa riêng cho Cờ Tướng.
3. **Cài đặt Fairy-Stockfish**: Bản build động cơ cờ đối thủ chuẩn với sức mạnh giới hạn (mặc định cấu hình 1200 ELO).
4. **Tải và cấu hình CSDL Khai cuộc**: Tải thư viện hơn 40,000 ván đấu Masters của Wukong-Xiangqi để làm tư liệu khai cuộc ngẫu nhiên cho giải đấu thử nghiệm.

---

## Quy trình chạy Thử nghiệm ELO (Gauntlet Testing)

Để đảm bảo đo lường sức mạnh (ELO) của **lingine** một cách khách quan nhất, chúng ta tổ chức giải đấu theo dạng **Gauntlet (Đấu Thử Thách)**:
* **Đối tượng:** **Lingine** (Engine thử nghiệm) sẽ là đấu thủ chính.
* **Đối thủ tham chiếu:** **5 thực thể Fairy-Stockfish** được giới hạn sức mạnh ở các mốc ELO cố định: **1000, 1200, 1400, 1600, 1800**.
* **Cấu hình khai cuộc:** Mỗi cặp đối đầu chơi **20 ván** (tổng cộng 100 ván đấu) xuất phát từ các thế cờ ngẫu nhiên trong thư viện Masters với độ sâu khai cuộc là **12 plies (6 nước đi đầu tiên)**.
* **Nguyên tắc công bằng:** Bật cờ `-repeat` để đảm bảo cả hai bên chơi cả quân Đỏ và quân Đen trên cùng một thế cờ khai cuộc nhằm triệt tiêu lợi thế đi trước.

### Kịch bản kiểm thử tự động (`scripts/run_gauntlet.py`)

Hệ thống sử dụng một script Python hợp nhất duy nhất để tự động thực hiện từ đầu đến cuối quy trình:
1. **Kiểm tra công cụ:** Đảm bảo tất cả các công cụ bổ trợ và cơ sở dữ liệu khai cuộc đã được tải đầy đủ. Nếu thiếu, script sẽ nhắc nhở bạn chạy `./scripts/setup_tools.sh`.
2. **Tự động Biên dịch:** Chạy `cargo build --release` để luôn đảm bảo bản kiểm thử là phiên bản mã nguồn mới nhất.
3. **Thực thi giải đấu:** Gọi `sylvan-cli` chạy song song 4 ván đấu cùng lúc đối đầu với 5 mốc ELO của Fairy-Stockfish.
4. **Phân tích & Tính toán ELO:** Đọc tệp tin kết quả `gauntlet.pgn` và tự động hiển thị bảng phân tích ELO cực kỳ trực quan.

Bạn có thể chạy kiểm thử mặc định bằng lệnh:
```bash
./scripts/run_gauntlet.py
```

### Chỉnh sửa cấu hình linh hoạt thông qua tham số (Arguments)
Script hỗ trợ các tham số dòng lệnh sau để bạn tự do thay đổi cấu hình mà không cần sửa code:

| Tham số ngắn | Tham số đầy đủ | Ý nghĩa | Mặc định |
| :--- | :--- | :--- | :--- |
| `-c` | `--cores`, `--concurrency` | Số ván đấu chạy song song đồng thời (số nhân CPU sử dụng) | Tự động tối ưu (`cores - 2`) |
| `-g` | `--games` | Số ván đấu đấu với mỗi đối thủ | `20` |
| `-t` | `--tc` | Thiết lập kiểm soát thời gian (Time Control) | `10/10+0.1` |
| `-d` | `--depth` | Độ sâu khai cuộc bắt buộc (plies) | `12` |
| `-e` | `--elos` | Danh sách ELO đối thủ (cách nhau bằng dấu phẩy) | `1000,1200,1400,1600,1800` |
| `-o` | `--pgnout` | Đường dẫn lưu tệp PGN kết quả | `gauntlet.pgn` |
| `-s` | `--skip-build` | Bỏ qua bước tự động chạy `cargo build --release` | Tự động build |

**Ví dụ một số lệnh cấu hình nâng cao:**
```bash
# 1. Chỉ chạy giải đấu nhanh gồm 4 ván mỗi mốc ELO để thử nghiệm nhanh
./scripts/run_gauntlet.py -g 4

# 2. Chỉ đấu với mốc ELO cao là 1600 và 1800, sử dụng đúng 20 cores CPU
./scripts/run_gauntlet.py -e 1600,1800 --cores 20

# 3. Đấu với mốc ELO 1200 và 1400, bỏ qua bước build lại code Rust
./scripts/run_gauntlet.py -e 1200,1400 -s
```

---

## Cách Tính & Ước Lượng Điểm ELO của Lingine

Script `run_gauntlet.py` sẽ tự động thực hiện phép tính toán ELO của **Lingine** dựa trên mô hình **FIDE Logistic Elo** tiêu chuẩn:

$$\text{Tỉ lệ điểm} = \frac{\text{Số ván Thắng} + 0.5 \times \text{Số ván Hòa}}{\text{Tổng số ván đấu}}$$

### Bảng đối chiếu chênh lệch ELO tham khảo:

| Tỉ lệ điểm của Lingine | Chênh lệch ELO ($\Delta\text{ELO}$) so với đối thủ |
| :--- | :--- |
| **50%** | Bằng ELO đối thủ ($\pm 0$) |
| **64%** | Mạnh hơn đối thủ $\approx +100$ ELO |
| **76%** | Mạnh hơn đối thủ $\approx +200$ ELO |
| **85%** | Mạnh hơn đối thủ $\approx +300$ ELO |
| **36%** | Yếu hơn đối thủ $\approx -100$ ELO |
| **24%** | Yếu hơn đối thủ $\approx -200$ ELO |
| **15%** | Yếu hơn đối thủ $\approx -300$ ELO |

> **Ví dụ:** Nếu **Lingine** đạt tỉ lệ điểm là **64%** trước đối thủ **FS-1400** (thắng 10, hòa 5, thua 5), công thức sẽ tự động cộng thêm $+100$ ELO để ước lượng sức mạnh của **Lingine** đạt **1500 ELO**. Kết quả trung bình cộng của tất cả 5 mốc đối thủ sẽ là điểm ELO tổng kết cuối cùng của bot.

