import paramiko
import json
import os
import re
import configparser
import subprocess
import shutil


VPS_HOST = "93.115.21.32"
VPS_USER = "root"
VPS_PASS = "jZcuC5KRBr5GBTak#"  
VPS_PORT = 22
VPS_HTTP_PORT = 5555
VPS_REMOTE_DIR = "/var/www/avnpro_updates" 


LOCAL_SETUPS_DIR = "server_setups"     
GITHUB_EXPORT_DIR = "github_export"    
LOCAL_EXE = "target/release/ac_pro_engineer.exe"
CARGO_TOML = "Cargo.toml"



def get_version():
    if not os.path.exists(CARGO_TOML): return None
    with open(CARGO_TOML, "r", encoding="utf-8") as f:
        content = f.read()
    match = re.search(r'version\s*=\s*"([\d\.]+)"', content)
    return match.group(1) if match else None

def build_project():
    print("🔨 [1/4] Компиляция проекта (cargo build --release)...")
    try:
        subprocess.run(["cargo", "build", "--release"], check=True)
        print("✅ Сборка успешна!")
        return True
    except Exception as e:
        print(f"❌ ОШИБКА СБОРКИ: {e}")
        return False

def parse_ac_ini(file_path, car_model, filename, source_track, author):
    cfg = configparser.ConfigParser(strict=False, interpolation=None)
    try:
        cfg.read(file_path)
    except:
        return None

    def get_val(section, key, default=0):
        try: return int(cfg.get(section, key)) # Исправил: здесь должен быть key, а не "VALUE"
        except: return default

    return {
        "name": os.path.splitext(filename)[0],
        "source": source_track, 
        "author": author,
        "credits": "Inspired by community benchmarks", # АВТОМАТИЧЕСКАЯ СТРОКА
        "car_id": car_model, 
        "is_remote": True,
        
        "fuel": get_val("FUEL", "VALUE"),
        "brake_bias": get_val("FRONT_BIAS", "VALUE"),
        "engine_limiter": get_val("ENGINE_LIMITER", "VALUE"),
        "pressure_lf": get_val("PRESSURE_LF", "VALUE"),
        "pressure_rf": get_val("PRESSURE_RF", "VALUE"),
        "pressure_lr": get_val("PRESSURE_LR", "VALUE"),
        "pressure_rr": get_val("PRESSURE_RR", "VALUE"),
        "wing_1": get_val("WING_1", "VALUE"),
        "wing_2": get_val("WING_2", "VALUE"),
        "camber_lf": get_val("CAMBER_LF", "VALUE"),
        "camber_rf": get_val("CAMBER_RF", "VALUE"),
        "camber_lr": get_val("CAMBER_LR", "VALUE"),
        "camber_rr": get_val("CAMBER_RR", "VALUE"),
        "toe_lf": get_val("TOE_OUT_LF", "VALUE"),
        "toe_rf": get_val("TOE_OUT_RF", "VALUE"),
        "toe_lr": get_val("TOE_OUT_LR", "VALUE"),
        "toe_rr": get_val("TOE_OUT_RR", "VALUE"),
        "spring_lf": get_val("SPRING_RATE_LF", "VALUE"),
        "spring_rf": get_val("SPRING_RATE_RF", "VALUE"),
        "spring_lr": get_val("SPRING_RATE_LR", "VALUE"),
        "spring_rr": get_val("SPRING_RATE_RR", "VALUE"),
        "rod_length_lf": get_val("ROD_LENGTH_LF", "VALUE"),
        "rod_length_rf": get_val("ROD_LENGTH_RF", "VALUE"),
        "rod_length_lr": get_val("ROD_LENGTH_LR", "VALUE"),
        "rod_length_rr": get_val("ROD_LENGTH_RR", "VALUE"),
        "arb_front": get_val("ARB_FRONT", "VALUE"),
        "arb_rear": get_val("ARB_REAR", "VALUE"),
        "damp_bump_lf": get_val("DAMP_BUMP_LF", "VALUE"),
        "damp_bump_rf": get_val("DAMP_BUMP_RF", "VALUE"),
        "damp_bump_lr": get_val("DAMP_BUMP_LR", "VALUE"),
        "damp_bump_rr": get_val("DAMP_BUMP_RR", "VALUE"),
        "damp_rebound_lf": get_val("DAMP_REBOUND_LF", "VALUE"),
        "damp_rebound_rf": get_val("DAMP_REBOUND_RF", "VALUE"),
        "damp_rebound_lr": get_val("DAMP_REBOUND_LR", "VALUE"),
        "damp_rebound_rr": get_val("DAMP_REBOUND_RR", "VALUE"),
        "diff_power": get_val("DIFF_POWER", "VALUE"),
        "diff_coast": get_val("DIFF_COAST", "VALUE"),
        "final_ratio": get_val("FINAL_RATIO", "VALUE"),
        "gears": []
    }

def generate_github_files():
    print(f"📦 [2/4] Генерация базы сетапов в '{GITHUB_EXPORT_DIR}'...")
    
    if not os.path.exists(LOCAL_SETUPS_DIR):
        print(f"⚠️ Папка {LOCAL_SETUPS_DIR} не найдена. Пропуск.")
        return False

    cars_db = {} 

    for root, dirs, files in os.walk(LOCAL_SETUPS_DIR):
        for file in files:
            if not file.endswith(".ini"): continue
            
            full_path = os.path.join(root, file)
            rel_path = os.path.relpath(full_path, LOCAL_SETUPS_DIR)
            parts = rel_path.split(os.sep)
            
            # Логика определения автора из структуры папок
            if len(parts) == 4:
                author, car_model, track_name = parts[0], parts[1], parts[2]
            elif len(parts) == 3:
                author, car_model, track_name = "AFN PRO", parts[0], parts[1] # Сделал по умолчанию AFN PRO
            elif len(parts) == 2:
                author, car_model, track_name = "AFN PRO", parts[0], "Generic"
            else:
                continue

            print(f"   ➕ {author} | {car_model} -> {file}")
            
            setup = parse_ac_ini(full_path, car_model, file, track_name, author)
            if setup:
                # Если автор AFN PRO, добавляем кредиты о сообществе
                if author == "AFN PRO":
                    setup["credits"] = "Refined by AFN PRO | Inspired by community benchmarks"
                
                if car_model not in cars_db: cars_db[car_model] = []
                cars_db[car_model].append(setup)

    if not cars_db:
        print("⚠️ Сетапы не найдены!")
        return False

    if os.path.exists(GITHUB_EXPORT_DIR):
        shutil.rmtree(GITHUB_EXPORT_DIR)
    os.makedirs(GITHUB_EXPORT_DIR)
    
    # Manifest
    manifest = []
    for car, setups in cars_db.items():
        authors = list(set([s["author"] for s in setups]))
        manifest.append({"id": car, "count": len(setups), "authors": authors})
    
    with open(f"{GITHUB_EXPORT_DIR}/manifest.json", "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)
        
    # Car JSONs
    for car, setups in cars_db.items():
        with open(f"{GITHUB_EXPORT_DIR}/{car}.json", "w", encoding="utf-8") as f:
            json.dump(setups, f, indent=2, ensure_ascii=False)

    print(f"✅ База данных создана: {len(cars_db)} авто.")
    return True


def deploy_to_vps(version):
    print(f"🚀 [3/4] Загрузка обновления на сервер {VPS_HOST}...")
    
    remote_exe_name = f"ac_pro_engineer_v{version}.exe"
    
   
    update_data = {
        "version": version,
        "url": f"http://{VPS_HOST}:{VPS_HTTP_PORT}/{remote_exe_name}",
        "notes": input("📝 Описание обновления (Enter = Update): ").strip() or "Update"
    }
    with open("version.json", "w", encoding="utf-8") as f:
        json.dump(update_data, f, indent=4)

    try:
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        ssh.connect(VPS_HOST, port=VPS_PORT, username=VPS_USER, password=VPS_PASS, banner_timeout=60)
        
        sftp = ssh.open_sftp()

       
        ssh.exec_command(f"mkdir -p {VPS_REMOTE_DIR}")
        
        print(f"📤 Загрузка {remote_exe_name}...")
        sftp.put(LOCAL_EXE, f"{VPS_REMOTE_DIR}/{remote_exe_name}")
        
        print(f"📤 Загрузка version.json...")
        sftp.put("version.json", f"{VPS_REMOTE_DIR}/version.json")
        
        sftp.close()
        ssh.close()
        print("✅ Загрузка на VPS завершена!")
        return True

    except Exception as e:
        print(f"❌ ОШИБКА ПОДКЛЮЧЕНИЯ К VPS: {e}")
        return False



def main():
  
    if not build_project(): return

   
    version = get_version()
    if not version:
        print("❌ Не удалось найти версию в Cargo.toml")
        return
    print(f"ℹ️ Версия: {version}")

  
    has_setups = generate_github_files()

  
    if deploy_to_vps(version):
        print("\n🎉 ВСЕ ГОТОВО!")
        
        if has_setups:
            print("\n⚠️ ВАЖНО: ТЕПЕРЬ ЗАЛЕЙ СЕТАПЫ НА GITHUB!")
            print(f"1. Открой папку: {os.path.abspath(GITHUB_EXPORT_DIR)}")
            print("2. Перетащи все файлы оттуда в свой репозиторий на сайте github.com")
            print("3. Нажми 'Commit changes'.")
        else:
            print("\n(Сетапы не обновлялись, заливать на GitHub нечего)")

if __name__ == "__main__":
    main()