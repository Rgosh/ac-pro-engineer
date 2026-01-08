import paramiko
import json
import os
import re
import time

# --- НАСТРОЙКИ ---
HOST = "93.115.21.32"
USERNAME = "root"
PASSWORD = "jZcuC5KRBr5GBTak#"  # Твой пароль от сервера
PORT = 22

# ПОРТ ДЛЯ ОБНОВЛЕНИЙ (Чтобы не ломать магазин на 80 порту)
UPDATE_PORT = 5555
REMOTE_DIR = "/var/www/avnpro_updates" 

# Пути
LOCAL_EXE = "target/release/ac_pro_engineer.exe"
CARGO_TOML = "Cargo.toml"

def get_version():
    """Читает версию прямо из Cargo.toml"""
    if not os.path.exists(CARGO_TOML): return None
    with open(CARGO_TOML, "r", encoding="utf-8") as f:
        content = f.read()
    match = re.search(r'version\s*=\s*"([\d\.]+)"', content)
    return match.group(1) if match else None

def configure_nginx(ssh):
    """Настраивает Nginx на порт 5555"""
    print(f"🔧 Настройка Nginx на порт {UPDATE_PORT}...")
    
    config = f"""server {{
    listen {UPDATE_PORT};
    root {REMOTE_DIR};
    index index.html;
    server_name _;
    
    location / {{
        try_files $uri $uri/ =404;
        autoindex on;
        add_header Access-Control-Allow-Origin *;
        add_header Cache-Control "no-cache";
    }}
}}"""
    
    # Записываем конфиг в отдельный файл
    ssh.exec_command(f"echo '{config}' > /etc/nginx/sites-available/avnpro_updates")
    # Включаем его
    ssh.exec_command("ln -s -f /etc/nginx/sites-available/avnpro_updates /etc/nginx/sites-enabled/")
    # Перезагружаем Nginx
    ssh.exec_command("systemctl reload nginx")
    print("✅ Nginx настроен (Магазин не затронут).")

def main():
    print(f"=== АВТО-ДЕПЛОЙ НА {HOST}:{UPDATE_PORT} ===")
    
    version = get_version()
    if not version:
        print("❌ Не удалось найти версию в Cargo.toml!")
        return
    print(f"📦 Обнаружена версия: {version}")

    if not os.path.exists(LOCAL_EXE):
        print(f"❌ Файл {LOCAL_EXE} не найден! Сделай: cargo build --release")
        return

    remote_exe_name = f"ac_pro_engineer_v{version}.exe"
    download_url = f"http://{HOST}:{UPDATE_PORT}/{remote_exe_name}"
    
    # Создаем JSON
    data = {
        "version": version,
        "url": download_url,
        "notes": input("Что нового (Enter если пусто): ").strip() or "Update"
    }
    with open("version.json", "w", encoding="utf-8") as f:
        json.dump(data, f, indent=4)

    try:
        print(f"📡 Подключение...")
        transport = paramiko.Transport((HOST, PORT))
        transport.connect(username=USERNAME, password=PASSWORD)
        sftp = paramiko.SFTPClient.from_transport(transport)
        
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        ssh.connect(HOST, username=USERNAME, password=PASSWORD)

        # 1. Настройка сервера
        configure_nginx(ssh)

        # 2. Загрузка файлов
        print(f"\n--- Загрузка файлов ---")
        ssh.exec_command(f"mkdir -p {REMOTE_DIR}")
        # Чистим старый json чтобы не кэшировался
        ssh.exec_command(f"rm -f {REMOTE_DIR}/version.json")
        
        print(f"📤 {remote_exe_name}")
        sftp.put(LOCAL_EXE, f"{REMOTE_DIR}/{remote_exe_name}")
        
        print(f"📤 version.json")
        sftp.put("version.json", f"{REMOTE_DIR}/version.json")
        
        # Права доступа
        ssh.exec_command(f"chmod -R 755 {REMOTE_DIR}")
        
        print("\n🎉 ГОТОВО!")
        print(f"Ссылка для проверки: {download_url}")
        print(f"JSON API: http://{HOST}:{UPDATE_PORT}/version.json")
        
        sftp.close()
        transport.close()
        ssh.close()

    except Exception as e:
        print(f"❌ Ошибка: {e}")

if __name__ == "__main__":
    main()