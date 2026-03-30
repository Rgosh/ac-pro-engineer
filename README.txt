========================================
       AC PRO ENGINEER v0.2.2
========================================

Thank you for downloading AC Pro Engineer!

[ INSTALLATION FOR WINDOWS ]
Open the "Windows" folder and run the executable (ac_pro_engineer.exe). No installation is required. 
Make sure Assetto Corsa or ACC is running so the app can connect to the shared memory and read telemetry.

[ LINUX & STEAM DECK USERS ]
AC Pro Engineer now fully supports Linux and Steam Deck natively!

How it works: The app uses a custom background process (`shm-bridge.exe`) that is injected directly into the game's Proton/Wine sandbox. This allows the native Linux application to read the game's memory with zero lag.

1. Open the "Linux" folder.
2. Ensure both the native binary (`ac_pro_engineer`) and the `shm-bridge.exe` file are kept together in the same folder.
3. You must have `protontricks` installed on your system to bridge the telemetry out of the game's sandbox.
4. Run `./ac_pro_engineer` in your terminal.

*Note for advanced users: If you use a custom Proton/Wine prefix (like Lutris, Heroic) or a Flatpak version of Protontricks, you can set the `AC_PROTON_PATH` environment variable before launching the app.*
*(e.g., AC_PROTON_PATH="flatpak run com.github.Matoking.protontricks" ./ac_pro_engineer)*

[ IMPORTANT: ANTIVIRUS FALSE POSITIVES ]
If Windows Defender or your antivirus software flags this file, please know that it is a FALSE POSITIVE.
Why does this happen? 
1. The application is new and currently lacks an expensive digital publisher certificate.
2. The core functionality requires the app to read the memory of another running process (the game's telemetry), which heuristics often mistake for malicious behavior.

Please add the executable to your antivirus exclusions/whitelist to use the tool. The project is completely open-source, and you can verify the code yourself.

[ LINKS & SUPPORT ]
Source Code & Issue Tracker: https://github.com/Rgosh/ac-pro-engineer
Updates & Reviews: https://www.overtake.gg/downloads/ac-pro-engineer-zero-lag-telemetry-setup-cloud-rust-powered.81695/

Enjoy your racing!