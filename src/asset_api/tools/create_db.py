import sqlite3

# Tạo kết nối tới tệp SQLite
conn = sqlite3.connect("assets.db")

# Tạo con trỏ (cursor) để thao tác với cơ sở dữ liệu
cursor = conn.cursor()

# Tạo bảng "asset"
cursor.execute("""
    CREATE TABLE IF NOT EXISTS asset (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        code TEXT NOT NULL,
        price REAL NOT NULL,
        website TEXT,
        description TEXT
    )
""")

# Lưu các thay đổi và đóng kết nối
conn.commit()
conn.close()
