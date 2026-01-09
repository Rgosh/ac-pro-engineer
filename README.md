# AC Pro Engineer

**AC Pro Engineer** is a standalone telemetry and race engineer tool designed for performance and utility. Unlike heavy electron-based overlays, this tool runs in a **Terminal User Interface (TUI)** using Rust.

It provides real-time analysis, live engineering advice, and a **one-click Setup Cloud** ecosystem.

![Screenshot](ссылка_на_скриншот.png) ## 🚀 Why Use This?
* **Zero FPS Impact:** Utilizes <0.1% CPU and minimal RAM. Perfect for competitive racing and low-end PCs.
* **Hacker Aesthetics:** Professional TUI design. No bloat, just data.
* **Cloud Setup Database:** Browse and download community setups directly from the app.

## ✨ Key Features
* **Live Telemetry:** Monitor tyre pressures, temps, brake temps, and fuel usage in real-time.
* **Race Engineer:** The app analyzes your driving to give live advice (e.g., "Tyres cold", "Optimum pressure reached", "Push harder").
* **Setup Browser:**
    * Press `B` to open the Online Database.
    * Compare remote setups with your current one instantly.
    * Press `D` to download and install automatically.
* **Analysis:** Lap-by-lap comparison and driving style radar (Smoothness vs Aggression).
* **Strategy:** Fuel calculator based on real consumption.

## 🎮 Controls
Since this is a terminal app, it uses keyboard shortcuts:
* `F1 - F7`: Switch Tabs (Dashboard, Telemetry, Engineer, Setup, Analysis...)
* `B`: Open/Close Setup Browser (Only in Setup Tab)
* `D`: Download Selected Setup
* `PageUp / PageDown`: Scroll details lists
* `Q`: Quit

## 📦 Installation
1. Download the latest `ac_pro_engineer.exe` from the Releases page
2. Run the application.
3. Start Assetto Corsa and drive!

*Note: Since this is a new tool written in Rust that reads game memory, Windows Defender might flag it as a false positive. Please add it to exclusions if necessary.*

## 🛠 Build from Source
```bash
cargo build --release