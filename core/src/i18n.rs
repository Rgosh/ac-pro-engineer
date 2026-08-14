//! The application's words, in one place instead of five hundred.
//!
//! Every user-facing string used to be written twice, at the point it was used:
//!
//! ```ignore
//! component: "Tyres".tr(ru).to_string(),
//! ```
//!
//! That works, and it costs three things that add up. **The code stops being
//! readable in one language** — a rule about tyre pressure is half English and
//! half Russian, and a reader has to skip every other branch. **A translation
//! cannot be reviewed**, because it is scattered across twenty files and there
//! is no list of it to look at. And **a third language means touching every
//! call site again**, which is the point at which nobody adds one.
//!
//! So the code says what it means, in English:
//!
//! ```ignore
//! component: "Tyres".tr(ru).to_string(),
//! ```
//!
//! and the Russian for it lives in [`CATALOGUE`] below, next to every other
//! Russian word in the program. This is the same split the in-game panel has
//! had since it was written — `assets/frontends/csp-panel/acpe/i18n.lua` — and
//! the desktop side was simply the half that never got it.
//!
//! ## What this is not
//!
//! It is not a general localisation framework. There is no plural handling, no
//! gender, no locale-aware number formatting — [`crate::config::Formatter`]
//! already owns units and decimals, and it stays there. This translates
//! *fragments*, exactly as the code did before, so the migration changed no
//! output at all.
//!
//! ## Adding a word
//!
//! Write the English at the call site and add one line to [`CATALOGUE`]. A
//! missing entry is not a compile error — it falls back to the English, which
//! is the right failure: an untranslated word is readable, and a panic in the
//! middle of a race is not. `every_translation_is_reachable` catches the
//! opposite mistake, an entry nobody uses.

use crate::config::Language;

/// English to Russian, and nothing else.
///
/// Sorted by where the words appear rather than alphabetically: the engineer's
/// vocabulary reads as a vocabulary that way, and a translator working through
/// it sees related words together instead of `Aero` next to `All four`.
///
/// **Keys are the English exactly as the code writes it**, including case. Two
/// entries differing only in case are two entries, because "Tyres" is a heading
/// and "tyres" is a word in a sentence, and Russian does not always agree that
/// they are the same word.
pub const CATALOGUE: &[(&str, &str)] = &[
    ("Fronts", "Перед"),
    ("Rears", "Зад"),
    ("Left side", "Левые"),
    ("Right side", "Правые"),
    ("All four", "Все шины"),
    ("Tyres", "Шины"),
    ("Pressure", "Давление"),
    ("over", "перекачаны"),
    ("under", "недокачаны"),
    ("target", "цель"),
    ("Take pressure out", "Спустить"),
    ("Put pressure in", "Накачать"),
    ("Temperature", "Температура"),
    ("over temperature", "перегрев"),
    ("cold", "холодные"),
    ("Less pressure / ease off", "Ниже давление / мягче стиль"),
    (
        "More pressure / work them harder",
        "Выше давление / больше нагрузки",
    ),
    ("Suspension", "Подвеска"),
    ("Camber", "Развал"),
    ("inner edge running hot", "перегрев внутренней части"),
    ("Less negative camber", "Меньше отриц. развала"),
    ("outer edge hotter", "внешняя часть горячее"),
    ("heated too evenly", "прогрев слишком равномерный"),
    ("More negative camber", "Больше отриц. развала"),
    ("Brakes", "Тормоза"),
    ("overheating", "перегрев"),
    ("Open the brake ducts", "Открыть воздуховоды"),
    ("Aero", "Аэро"),
    ("Ride height", "Клиренс"),
    ("Bottoming out", "Пробои по асфальту"),
    (
        "Raise the ride height / stiffer springs",
        "Выше клиренс / жёстче пружины",
    ),
    ("Balance", "Баланс"),
    ("Oversteer", "Избыточная"),
    (
        "Softer rear ARB / more rear wing",
        "Мягче задний стаб / больше заднего антикрыла",
    ),
    ("Understeer", "Недостаточная"),
    (
        "Softer front ARB / more front wing",
        "Мягче передний стаб / больше переднего антикрыла",
    ),
    ("Driving", "Пилотаж"),
    ("Braking", "Торможение"),
    ("Lockups", "Блокировки"),
    (
        "Ease onto the pedal / more ABS",
        "Мягче на педаль / больше ABS",
    ),
    ("Steering", "Руление"),
    ("Over-rotation", "Перекрут руля"),
    (
        "Less steering — the tyres are scrubbing",
        "Меньше угла — шины скребут",
    ),
    ("Pedals", "Педали"),
    ("Coasting", "Накат"),
    (
        "Get back on the throttle sooner",
        "Раньше на газ после торможения",
    ),
    (
        "Waiting for telemetry from Assetto Corsa...",
        "Ожидание данных от Assetto Corsa...",
    ),
    ("Assetto Corsa is not running", "Assetto Corsa не запущена"),
    ("TERMINAL TOO SMALL", "ОКНО СЛИШКОМ МАЛЕНЬКОЕ"),
    ("Resize to continue", "Увеличьте окно"),
    ("Bottoming", "Пробой"),
    ("Chassis bottoming out!", "Удары днищем о трассу!"),
    (
        "Increase ride height or stiffness",
        "Увеличьте клиренс или жесткость",
    ),
    ("Aerodynamics", "Аэродинамика"),
    (
        "Stiffen Rear Springs or add Packers",
        "Увеличьте жесткость задних пружин (Rear Springs) или Packer",
    ),
    (
        "Decrease Front Rebound",
        "Уменьшить отбой (Rebound) спереди",
    ),
    ("Increase Rear Ride Height", "Увеличить клиренс сзади"),
    (
        "Move Brake Bias Rearwards",
        "Сместить тормозной баланс назад",
    ),
    (
        "Increase Front Rebound",
        "Увеличить отбой (Rebound) спереди",
    ),
    (
        "Move Brake Bias Forwards",
        "Сместить тормозной баланс вперед",
    ),
    ("Increase Front Wing", "Увеличить переднее антикрыло"),
    ("Softer Front Springs", "Мягче передние пружины"),
    ("Softer Front ARB", "Мягче передний стабилизатор (ARB)"),
    ("More Front Camber", "Больше развал (Camber) спереди"),
    ("Softer Rear Springs", "Мягче задние пружины"),
    ("Softer Rear ARB", "Мягче задний стабилизатор (ARB)"),
    ("Increase Front Ride Height", "Выше клиренс спереди"),
    ("Increase Front Bump", "Увеличить сжатие (Bump) спереди"),
    ("Stiffer Rear Springs", "Жестче задние пружины"),
    (
        "Increase Diff Power",
        "Увеличить блокировку дифференциала (Power)",
    ),
    ("Decrease Rear Bump", "Уменьшить сжатие (Bump) сзади"),
    (
        "Decrease Diff Power",
        "Уменьшить блокировку дифференциала (Power)",
    ),
    ("Increase TC", "Больше Traction Control"),
    (
        "Increase Downforce (Wings)",
        "Увеличить прижимную силу (Крылья)",
    ),
    ("More Rear Toe-In", "Больше схождения (Toe) сзади"),
    ("Stiffer Suspension Overall", "Жестче подвеску в целом"),
    ("No major differences", "Нет существенных отличий"),
    ("Force Feedback", "Руль (FFB)"),
    ("Lower FFB Gain", "Снизить Gain"),
    ("Bias", "Баланс"),
    ("Move Bias REARWARDS", "Сместить баланс НАЗАД"),
    ("Move Bias FORWARDS", "Сместить баланс ВПЕРЕД"),
    ("Time Loss", "Потеря времени"),
    ("Excessive Coasting", "Много наката (Coasting)"),
    ("Keep throttle or brake", "Держите газ или тормозите"),
    ("High Speed Understeer", "Снос передней оси (High Speed)"),
    (
        "More Front Wing / Softer Front",
        "Больше крыла спереди / Мягче спереди",
    ),
    ("High Speed Oversteer", "Нестабильность сзади (High Speed)"),
    ("More Rear Wing", "Больше крыла сзади"),
    ("Overdriving", "Скраббинг"),
    ("Strategy", "Стратегия"),
    ("Fuel", "Топливо"),
    ("Race Finish", "Финиш"),
    ("Save Fuel / Box", "Экономить / Пит-стоп"),
    (
        "the suspension is running out of travel over kerbs and compressions",
        "подвеске не хватает хода на поребриках и сжатиях",
    ),
    (
        "downforce is squatting the rear, and the rake goes with it",
        "прижимная сила сажает зад, и вместе с ним уходит развал по длине",
    ),
    (
        "the rake difference at the same speed next run out",
        "разница развала по длине на той же скорости в следующем стинте",
    ),
    (
        "the signal is hitting its ceiling, and everything above it never reaches the wheel",
        "усилие упирается в потолок, и всё, что выше него, до руля не доходит",
    ),
    (
        "the clipping share after lowering the gain — near zero through corners",
        "доля клиппинга после снижения Gain — цель около нуля в поворотах",
    ),
    ("pressure", "давление"),
    ("Inflate", "Накачать"),
    ("Deflate", "Спустить"),
    ("Wear", "Износ"),
    ("Box / Careful", "Пит-стоп / Осторожно"),
    ("now", "сейчас"),
    ("COLD", "ХОЛОДНЫЕ"),
    ("Warm tyres", "Греть шины"),
    (
        "not enough energy is going into the tyre to bring it into its window",
        "в шину не вкладывается достаточно энергии, чтобы она вышла в окно",
    ),
    ("window from", "окно от"),
    ("Overheat", "Перегрев"),
    ("OVERHEATING", "ПЕРЕГРЕВ"),
    ("Cool tyres", "Остудить шины"),
    (
        "the tyre is being given more energy than it can shed",
        "шина отдаёт больше энергии, чем успевает сбросить",
    ),
    ("window to", "окно до"),
    ("brakes cooking", "перегрев тормозов"),
    ("Move bias / Cool down", "Сместить баланс / Охладить"),
    (
        "more energy is going into the brakes than they can shed",
        "в тормоза уходит больше энергии, чем они успевают сбросить",
    ),
    ("ceiling", "предел"),
    (
        "too much of the braking is landing on the front axle",
        "слишком много торможения приходится на переднюю ось",
    ),
    (
        "front lockups next run out, after moving the bias back",
        "блокировки спереди в следующем стинте после сдвига баланса назад",
    ),
    (
        "too much of the braking is landing on the rear axle",
        "слишком много торможения приходится на заднюю ось",
    ),
    (
        "rear lockups next run out, after moving the bias forward",
        "блокировки сзади в следующем стинте после сдвига баланса вперёд",
    ),
    (
        "the car is rolling unloaded where it should be braking or driving",
        "машина катится без нагрузки там, где должна тормозить или разгоняться",
    ),
    (
        "the share of the next lap spent on neither pedal",
        "доля наката в следующем круге",
    ),
    (
        "the front axle runs out of grip before the rear at speed",
        "передняя ось теряет сцепление раньше задней на скорости",
    ),
    (
        "the understeer count next run out, after the change",
        "снос передней оси в следующем стинте после изменения",
    ),
    (
        "the rear axle runs out of grip before the front at speed",
        "задняя ось теряет сцепление раньше передней на скорости",
    ),
    (
        "the oversteer count next run out, after the change",
        "нестабильность сзади в следующем стинте после изменения",
    ),
    (
        "more steering angle than the corner will take, so the tyres scrub",
        "руля больше, чем поворот может взять — шины скребут, а не держат",
    ),
    (
        "the over-rotation count through the same corners next lap",
        "перекрут руля в следующем круге на тех же поворотах",
    ),
    ("  ok", "  ок"),
    ("🔴 LIVE FEED [<-]", "🔴 РЕАЛЬНОЕ ВРЕМЯ [<-]"),
    ("📋 POST-STINT", "📋 ДЕБРИФИНГ"),
    ("🎯 PRESSURES [->]", "🎯 ДАВЛЕНИЯ [->]"),
    ("Smoothness", "Плавность (Smoothness)"),
    ("Aggression", "Агрессия (Aggression)"),
    ("Trail Braking", "Трейл-брейкинг (Trail Braking)"),
    ("🛑 Lockups detected: ", "🛑 Блокировки колес: "),
    ("🌀 Wheelspin/Spins: ", "🌀 Пробуксовки/Спины: "),
    (" LAP SUMMARY (UP/DOWN) ", " СВОДКА КРУГА (ВВЕРХ/ВНИЗ) "),
    ("LAP ", "КРУГ "),
    (
        "No data available. Drive a lap.",
        "Нет данных. Проедьте круг.",
    ),
    (
        " ENGINEER ANALYSIS & TELEMETRY ",
        " ИНЖЕНЕРНЫЙ АНАЛИЗ И ТЕЛЕМЕТРИЯ ",
    ),
    ("cause:", "причина:"),
    ("confirm:", "проверить:"),
    (
        " Nothing to report — the lap was clean.",
        " Ничего не нашёл — круг чистый.",
    ),
    (
        "OVER THE STINT — THE CAR OR THE DRIVING",
        "ЗА СТИНТ — МАШИНА ИЛИ ПИЛОТАЖ",
    ),
    (" PRESSURE TARGETS ", " ЦЕЛЕВЫЕ ДАВЛЕНИЯ "),
    ("Waiting for telemetry...", "Ожидание телеметрии..."),
    ("COLD SETUP PRESSURES", "СТАРТОВЫЕ (ХОЛОДНЫЕ) ДАВЛЕНИЯ"),
    ("Air", "Воздух"),
    ("grip", "сцепление"),
    ("Front", "Перед"),
    ("Rear", "Зад"),
    ("hot", "горячее"),
    ("temp", "темп."),
    ("PER-CORNER ADJUSTMENT", "ПОКОРНЕРНАЯ КОРРЕКЦИЯ"),
    ("at the same point", "в той же точке"),
    ("not measured", "нет данных"),
    (
        "never got back to throttle in the corner",
        "не вернулся к газу в повороте",
    ),
    (
        "Needs a reference lap. Drive a second one, or load a saved lap with 'L'.",
        "Нужен эталонный круг. Проедьте второй круг или загрузите сохранённый ('L').",
    ),
    (
        "This is the reference lap — there is nothing to compare it with.",
        "Это и есть эталонный круг — сравнивать не с чем.",
    ),
    (
        "No corners found in the trace — too short a lap, or no telemetry in it.",
        "В трейсе не найдено ни одного поворота. Круг слишком короткий или без телеметрии.",
    ),
    ("  on the lap", "  на круге"),
    ("to T1", "до Т1"),
    (
        "No corner cost more than a tenth. That was a tidy lap.",
        "Ни один поворот не стоил больше десятой. Хороший круг.",
    ),
    (" Where the time went ", " Где ушло время "),
    (
        "Nothing to pull apart — no corner cost more than a tenth.",
        "Нечего разбирать — ни один поворот не стоил больше десятой.",
    ),
    (
        "  The reference lap has no corner here, so there is nothing to compare.",
        "  Эталонный круг не проходил здесь поворот — сравнить не с чем.",
    ),
    ("Entry speed", "Вход"),
    ("Minimum speed", "Минимальная"),
    ("Exit speed", "Выход"),
    ("Throttle", "Газ"),
    (" The worst corner ", " Худший поворот "),
    ("Race Pace History", "История Темпа (Stint Pace)"),
    ("No completed laps yet", "Нет завершенных кругов"),
    ("Lap Time", "Время круга"),
    ("NO DATA", "НЕТ ДАННЫХ"),
    ("Drive more laps...", "Проедьте пару кругов..."),
    ("FUEL IS SAFE", "ТОПЛИВА ХВАТАЕТ"),
    ("No refueling needed", "Дозаправка не требуется"),
    ("REFUEL NEEDED", "НУЖЕН ПИТ-СТОП"),
    ("Not enough to finish", "Не хватит до финиша"),
    ("Tyre Life Predictor", "Прогноз Жизни Шин"),
    ("spent", "конец"),
    ("Time Delta (s)", "Дельта (сек)"),
    ("Time Delta vs Best", "Отставание от Лучшего (Время)"),
    ("Cur Speed", "Тек. Скор"),
    ("Best", "Лучшая"),
    ("Speed (km/h)", "Скорость (км/ч)"),
    ("Pedals (%)", "Педали (%)"),
    ("Steering (deg)", "Руль (град)"),
    ("Slip Ratio", "Проскальзывание"),
    ("Gas (Ref)", "Газ (Ref)"),
    ("Traction Loss", "Потеря Сцепления (Slip vs Time)"),
    ("Traction Stats", "Анализ Трекшена"),
    ("Grip Usage", "Использ. Сцепления"),
    ("Exit Aggression", "Агрессия на выходе"),
    ("Throttle Smooth", "Плавность Газа"),
    ("Stability (TC)", "Стабильность (TC)"),
    ("Throttle in Corner", "Газ в Повороте"),
    ("CRITICAL CLIPPING", "КРИТИЧЕСКИЙ КЛИППИНГ"),
    (
        "Lower Game Gain immediately!",
        "Срочно снизьте Gain в настройках игры!",
    ),
    ("LIGHT CLIPPING", "ЛЕГКИЙ КЛИППИНГ"),
    ("Lower Gain slightly (2-3%)", "Чуть снизьте Gain (на 2-3%)"),
    ("WEAK SIGNAL", "СЛАБЫЙ СИГНАЛ"),
    ("Safe to increase Gain", "Можно повысить Gain (безопасно)"),
    ("OPTIMAL", "ОПТИМАЛЬНО"),
    ("Settings are perfect", "Настройки отличные, не меняйте"),
    ("Status: ", "Состояние: "),
    ("Detail Loss (Clip): ", "Потеря деталей (Clip): "),
    ("Current Force: ", "Текущая сила: "),
    ("ADVICE:", "РЕКОМЕНДАЦИЯ:"),
    (" FFB Gain Tuning ", " Настройка FFB (Gain) "),
    (" Steering ", " Руль "),
    (
        " Signal History (Last 10s) ",
        " История Сигналов (Последние 10 сек) ",
    ),
    (" UPDATE ", " ОБНОВЛЕНИЕ "),
    ("SUCCESSFULLY UPDATED!", "УСПЕШНО ОБНОВЛЕНО!"),
    ("Press ENTER to continue", "Нажмите ENTER чтобы продолжить"),
    (
        "⭐ This is an Open Source project. Your review helps us grow!",
        "⭐ Это Open Source проект. Ваш отзыв помогает нам расти!",
    ),
    (
        "[O] Leave Review  [H] Hide Forever",
        "[O] Оставить отзыв  [H] Скрыть навсегда",
    ),
    ("Downloading", "Скачивание"),
    ("AVAILABLE", "ДОСТУПНО"),
    ("Checking...", "Проверка..."),
    ("Versions & Rollback", "Версии & Откат"),
    ("Net Error", "Ошибка сети"),
    ("START (TERMINAL TUI)", "ЗАПУСК (ТЕРМИНАЛ)"),
    ("DETECTED (READY TO START)", "ОБНАРУЖЕНО (ГОТОВО К СТАРТУ)"),
    ("WAITING FOR SIMULATOR...", "ОЖИДАНИЕ SIMULATOR..."),
    ("Downloading...", "Загрузка..."),
    ("READY!", "ГОТОВО!"),
    ("Press ENTER...", "Нажмите ENTER..."),
    (
        " [←/→] Select Version   [ENTER] Install",
        " [←/→] Выбор версии   [ENTER] Установка",
    ),
    ("Changelog:", "Список изменений:"),
    ("⚠️ WARNING: Legacy Version!", "⚠️ ВНИМАНИЕ: Старая версия!"),
    (
        "No updater inside. You won't be able to switch back.",
        "В ней нет апдейтера. Вы не сможете вернуться обратно.",
    ),
    ("🔥 UPDATE AVAILABLE", "🔥 ДОСТУПНО ОБНОВЛЕНИЕ"),
    ("♻ Downloading...", "♻ Скачивание..."),
    (
        "[↑/↓] Select  [←/→] Change  [ENTER] Open  [Q] Quit",
        "[↑/↓] Навигация  [←/→] Менять  [ENTER] Выбор  [Q] Выход",
    ),
    (" Load Telemetry ", " Загрузить Телеметрию "),
    (
        "No saved files found.\\nCheck 'saved_laps' folder.",
        "Нет сохраненных файлов.\\nПапка 'saved_laps' пуста.",
    ),
    (
        "ENTER: Load | ESC: Close",
        "ENTER: Загрузить | ESC: Закрыть",
    ),
    ("Temps (C)", "Температуры (C)"),
    (
        "Damper Histograms (Bump/Rebound)",
        "Амортизаторы (Сжатие/Отбой)",
    ),
    ("Stability", "Стабильность"),
    ("Scrubbing", "Скраббинг (Scrub)"),
    ("Peak G", "Пик G-Force"),
    ("LAP OVERVIEW", "ОБЗОР КРУГА"),
    ("Session Best", "Лучший в сессии"),
    ("Optimal", "Теор. Оптим."),
    ("Sector Analysis", "Сектора"),
    ("Driving Evaluation", "Оценка Вождения"),
    ("Overall Score", "Общий Рейтинг"),
    ("Car (T/B/W)", "Схема (T/B/W)"),
    ("Micro-Sectors (Delta)", "Микро-Сектора (Дельта)"),
    ("Load Reference Lap", "Загрузите круг сравнения"),
    ("Extended Stats", "Расширенная Статистика"),
    ("Top Speed", "Макс. Скорость"),
    ("Min Speed", "Мин. Скорость"),
    ("Avg Speed", "Средняя Скорость"),
    ("Fuel Used", "Расход Топлива"),
    ("Scrubbing (Errors)", "Скраббинг (Ошибки)"),
    ("Environment", "Среда"),
    ("Inputs", "Ввод"),
    ("Metadata", "Метаданные"),
    ("Unknown", "Неизвестно"),
    ("Car:    ", "Авто:   "),
    ("Track:  ", "Трасса: "),
    ("Date:   ", "Дата:   "),
    ("Time:   ", "Время:  "),
    ("Grip:   ", "Грип:   "),
    ("Engine RPM", "Обороты Двигателя (RPM)"),
    ("Gear Distribution (%)", "Распределение Передач (%)"),
    ("Efficiency", "Эффективность"),
    ("Total Shifts", "Всего переключений"),
    ("Fuel/Lap (Est)", "Ср. Расход (л/круг)"),
    ("Time @ WOT", "Время в пол (WOT)"),
    ("Fuel Level", "Топливо (кг)"),
    ("←/→ Tabs   ↑/↓ Laps", "←/→ Вкладки   ↑/↓ Круги"),
    (
        "No data. Press 'L' to load or drive a lap.",
        "Нет данных. Нажмите 'L' для загрузки или проедьте круг.",
    ),
    (
        "✓ INSTALLED (D to overwrite)",
        "✓ УСТАНОВЛЕНО (D для обновления)",
    ),
    ("Press 'D' to DOWNLOAD", "Нажми 'D' для СКАЧИВАНИЯ"),
    ("Credits: ", "Создатели: "),
    ("⚠ ADVICE: ", "⚠ СОВЕТ: "),
    ("SETUP ANALYSIS", "АНАЛИЗ СЕТАПА"),
    ("✓ EXCELLENT CHOICE", "✓ ОТЛИЧНЫЙ ВЫБОР"),
    ("This setup is a good match.", "Этот сетап подходит."),
    ("ENGINEER VERDICT", "ВЕРДИКТ ИНЖЕНЕРА"),
    ("Parameter", "Параметр"),
    ("Current", "Текущий"),
    ("Reference", "Эталон"),
    ("Diff", "Разница"),
    (
        "Setups are completely identical!",
        "Сетапы полностью идентичны!",
    ),
    (
        "Select a setup to see differences.",
        "Для сравнения выберите сетап в базе.",
    ),
    (
        "ENTER to bind, DEL for the default, ESC to cancel",
        "ENTER — назначить, DEL — стандарт, ESC — отмена",
    ),
    ("press a key…", "нажмите клавишу…"),
    ("Telemetry section", "Телеметрия в оверлее"),
    ("Engineer section", "Советы инженера в оверлее"),
    ("Session section", "Блок сессии в оверлее"),
    ("Lap timing section", "Тайминги в оверлее"),
    ("Fuel section", "Топливо в оверлее"),
    ("Engineer lines", "Строк инженера"),
    (
        "Telemetry update rate (ms). Lower = Smoother.",
        "Интервал обновления телеметрии (мс). Меньше = Плавнее.",
    ),
    (
        "Number of data points on charts. Higher = Longer history.",
        "Количество точек на графиках. Больше = Длиннее история.",
    ),
    (
        "Automatically save settings on exit.",
        "Авто-сохранение настроек при выходе.",
    ),
    (
        "Show 'Leave Review' banner on startup.",
        "Показывать баннер 'Оставить отзыв' при запуске.",
    ),
    (
        "Pressure units (PSI / Bar / kPa).",
        "Единицы давления (PSI / Bar / kPa).",
    ),
    (
        "Temperature units (Celsius / Fahrenheit).",
        "Единицы температуры (Цельсий / Фаренгейт).",
    ),
    (
        "Min Tyre Pressure (Warning: Blue).",
        "Мин. давление шин (Предупреждение: Синий).",
    ),
    (
        "Max Tyre Pressure (Warning: Red).",
        "Макс. давление шин (Предупреждение: Красный).",
    ),
    ("Min Tyre Temp (Cold).", "Мин. температура шин (Холодные)."),
    (
        "Max Tyre Temp (Overheat).",
        "Макс. температура шин (Перегрев).",
    ),
    ("Critical Brake Temp.", "Критическая температура тормозов."),
    (
        "Fuel warning threshold (laps).",
        "Остаток топлива для предупреждения (круги).",
    ),
    (
        "Tyre life below which it is a warning (%).",
        "Остаток жизни шины, ниже которого это предупреждение (%).",
    ),
    (
        "Tyre life below which it is critical (%).",
        "Остаток жизни шины, ниже которого это критично (%).",
    ),
    (
        "Target hot pressure, front.",
        "Целевое горячее давление спереди.",
    ),
    (
        "Target hot pressure, rear.",
        "Целевое горячее давление сзади.",
    ),
    (
        "Measure the delta against your own best lap, not AC's meter.",
        "Считать дельту по своему лучшему кругу, а не по метру AC.",
    ),
    (
        "Show the telemetry block in the in-game overlay.",
        "Показывать блок телеметрии в игровом оверлее.",
    ),
    (
        "Show engineer advice in the in-game overlay.",
        "Показывать советы инженера в игровом оверлее.",
    ),
    (
        "Show position, lap and track conditions in the overlay.",
        "Показывать позицию, круг и условия трассы в оверлее.",
    ),
    (
        "Show delta and lap times in the overlay.",
        "Показывать дельту и времена кругов в оверлее.",
    ),
    (
        "Show fuel and remaining laps in the overlay.",
        "Показывать топливо и остаток кругов в оверлее.",
    ),
    (
        "The startup card. [I] installs it, [U] removes it from the game.",
        "Карточка при запуске. [I] — установить, [U] — удалить из игры.",
    ),
    (" REMOVE THE OVERLAY? ", " УДАЛИТЬ ОВЕРЛЕЙ? "),
    (" INSTALL THE OVERLAY? ", " УСТАНОВИТЬ ОВЕРЛЕЙ? "),
    (
        "  The panel's files leave the game folder.",
        "  Из папки игры уйдут файлы панели.",
    ),
    (
        "  The panel's files go into the game folder.",
        "  Файлы панели лягут в папку игры.",
    ),
    ("files", "файлов"),
    (
        "  The panel's settings live elsewhere and are not touched.",
        "  Настройки панели хранятся отдельно и не пострадают.",
    ),
    (" [ YES, REMOVE ] ", " [ ДА, УДАЛИТЬ ] "),
    (" [ YES, INSTALL ] ", " [ ДА, УСТАНОВИТЬ ] "),
    (" [ CANCEL ] ", " [ ОТМЕНА ] "),
    (" OVERLAY DIAGNOSTICS ", " ПРОВЕРКА ОВЕРЛЕЯ "),
    ("OVERLAY", "ОВЕРЛЕЙ"),
    (
        "  [R] check again   ESC to close",
        "  [R] проверить заново   ESC закрыть",
    ),
    (" OVERLAY REMOVED ", " ОВЕРЛЕЙ УДАЛЁН "),
    (" OVERLAY INSTALLED ", " ОВЕРЛЕЙ УСТАНОВЛЕН "),
    (
        "  Your settings are kept — [I] puts it back as it was.",
        "  Настройки панели сохранены — [I] вернёт всё как было.",
    ),
    (
        "  Remove it any time with [U]. Your settings stay.",
        "  Удалить в любой момент — [U]. Настройки останутся.",
    ),
    ("  Any key to close", "  Любая клавиша — закрыть"),
    ("SYSTEM", "СИСТЕМА"),
    ("DISPLAY", "ДИСПЛЕЙ"),
    ("ENGINEER", "ИНЖЕНЕР"),
    ("KEYS", "КЛАВИШИ"),
    ("ON", "ВКЛ"),
    ("OFF", "ВЫКЛ"),
    ("Launcher Banner", "Баннер в лаунчере"),
    ("SHOW", "ПОКАЗАТЬ"),
    ("HIDE", "СКРЫТЬ"),
    ("Wear: critical below", "Износ: критично ниже"),
    (
        "Target Hot Pressure (Front)",
        "Цель горяч. давления (Перед)",
    ),
    ("Target Hot Pressure (Rear)", "Цель горяч. давления (Зад)"),
    ("Ghost Delta Widget", "Виджет Ghost Delta"),
    (
        "[↑/↓] Select   [ENTER] Edit   [←/→] Change   [A/S/D/F/G] Categories",
        "[↑/↓] Выбор   [ENTER] Изменить   [←/→] Менять   [A/S/D/F/G] Категории",
    ),
    // Words that are one word in English and two in Russian.
    ("grip|short", "сцеп."),
    (
        "WAITING FOR SIMULATOR...|spelled out",
        "ОЖИДАНИЕ СИМУЛЯТОРА...",
    ),
    (
        "Understeer|with the English beside it",
        "Снос передней (Under)",
    ),
    (
        "Oversteer|with the English beside it",
        "Занос задней (Over)",
    ),
    ("Lockups|with the English beside it", "Блокировки (Lockup)"),
    ("Aggression|bare", "Агрессия"),
    ("Grip Usage|shorter", "Использ. Грипа"),
    (
        "How many engineer lines reach the overlay (0-8). The panel may draw fewer — it has a slider of its own.",
        "Сколько строк инженера уходит в оверлей (0-8). Панель может показать меньше — у неё свой ползунок.",
    ),
    // Sentences with a value in them, from the engineer. The whole template
    // is one entry: split into fragments and joined with format!, the units
    // between the words stay English while the words around them do not.
    (
        "Rear dropping too much at high speed (-{0}mm)",
        "Зад сильно проседает на скорости (-{0}мм)",
    ),
    ("Aero: {0}", "Аэродинамика: {0}"),
    ("Front Camber: {0}", "Развал перед: {0}"),
    ("Tyre Press: {0} PSI", "Давление шин: {0} PSI"),
    ("FFB Clipping: {0}% of time", "Клиппинг силы: {0}% времени"),
    ("Tyres ({0})", "Шины ({0})"),
    (" (NOW: {0}%)", " (СЕЙЧАС: {0}%)"),
    ("FRONT Locking detected{0}", "Блокировка ПЕРЕДНИХ колес{0}"),
    ("REAR Locking (Danger!){0}", "Блокировка ЗАДНИХ колес{0}"),
    (
        "Steering over-rotated by {0}°! Tyres sliding.",
        "Перекрут руля на {0}°! Шины скользят.",
    ),
    ("Reduce steering angle by {0}°", "Уменьши угол руля на {0}°"),
    ("FUEL LOW: {0} laps", "ТОПЛИВО: {0} кр."),
    ("Short {0} L", "Не хватит {0} л."),
    (
        "the bottoming count on {0} over the same lap, once it is raised",
        "пробои на {0} в том же круге после подъёма",
    ),
    (
        "-{0} mm between low and high speed",
        "-{0} мм между медленным и быстрым участком",
    ),
    ("{0} frames", "{0} кадров"),
    ("{0} frames of the lap ({1} %)", "{0} кадров круга ({1} %)"),
    (
        "the hot pressure on {0} after two laps at pace: {1} is the target",
        "давление на {0} после двух кругов на темпе: цель {1}",
    ),
    (
        "the I/M/O spread on {0} next run out: {1} is the window",
        "разброс I/M/O на {0} в следующем стинте: цель {1}",
    ),
    (
        "the temperature on {0} after a lap at pace: the window starts at {1}",
        "температура на {0} после круга на темпе: окно от {1}",
    ),
    (
        "the temperature on {0} a lap after the change: the window ends at {1}",
        "температура на {0} через круг после изменения: окно до {1}",
    ),
    (
        "the peak on {0} through the next lap: {1} is the ceiling",
        "пик температуры на {0} в следующем круге: предел {1}",
    ),
    (
        "{0} frames of front lock against {1} at the rear",
        "{0} кадров блокировки спереди против {1} сзади",
    ),
    (
        "{0} frames of rear lock against {1} at the front",
        "{0} кадров блокировки сзади против {1} спереди",
    ),
    (
        "the tyre is not building enough heat to reach the window",
        "шина не набирает температуру и не доходит до окна",
    ),
    (
        "the tyre is building more pressure than the cold setting allows for",
        "шина набирает больше давления, чем заложено в холодном",
    ),
    ("WORN OUT", "ИЗНОС (Крит)"),
    ("high wear", "сильный износ"),
    ("{0} laps on this set", "{0} кругов на этом комплекте"),
    (
        "no complete lap on this set yet",
        "на этом комплекте ещё нет полного круга",
    ),
    (
        "the life on {0} at the end of the next lap: ~{1} laps left at this rate",
        "остаток на {0} в конце круга: по текущему темпу ~{1} кругов",
    ),
    (
        "the life on {0} at the end of the next lap",
        "остаток на {0} в конце круга",
    ),
    ("contact patch inefficient", "пятно контакта не эффективно"),
    ("inner edge overheating", "перегрев внутренней части"),
    (
        "More neg. camber{0}. If maxed -> soften ARB",
        "Больше отриц. развала{0}. Если предел -> смягчите ARB",
    ),
    (
        "Less neg. camber{0}. If maxed -> stiffen ARB",
        "Меньше отриц. развала{0}. Если предел -> зажмите ARB",
    ),
    (
        "the outer shoulder is not being loaded through corners",
        "недостаточно нагрузки на внешнюю часть в поворотах",
    ),
    (
        "the inner shoulder is carrying the corner",
        "внутренняя часть перегружена в поворотах",
    ),
    ("{0}, worst excess {1}°", "{0}, худший перекрут {1}°"),
    // The confidence labels, the debrief, the corner analysis and the parts of
    // the terminal that were left behind by the first pass.
    ("High", "Высокая"),
    ("Medium", "Средняя"),
    ("Low", "Низкая"),
    ("no observations", "нет наблюдений"),
    ("from one {0}", "по одному {0}"),
    ("across {0} {1}s", "по {0} {1}"),
    ("the car", "машина"),
    ("the driving", "пилотаж"),
    ("could be either", "непонятно"),
    ("{0} laps", "{0} кр."),
    (
        "BROWSER: Arrows to navigate | ENTER to select | PgUp/PgDn to scroll",
        "БРАУЗЕР: Стрелки — навигация | ENTER — выбор | PgUp/PgDn — скролл",
    ),
    (
        "Arrows to pick a setup | PgUp/PgDn to scroll the details",
        "Стрелки — выбор сетапа | PgUp/PgDn — скролл деталей",
    ),
    (
        "Recommended: '{0}'. Differences:",
        "Рекомендуется '{0}'. Отличия:",
    ),
    ("G-G Plot (Grip Usage: {0}%)", "G-G (Исп. сцепления: {0}%)"),
    (
        " Lap {0}  {1}   vs {2}   {3} corners, {4} worth looking at ",
        " Круг {0}  {1}   против {2}   {3} поворотов, {4} стоящих внимания ",
    ),
    ("   [losses over {0}s only]", "   [только потери > {0}s]"),
    ("Cnr", "Пов"),
    ("Min", "Мин"),
    ("Speed: in/min/out", "Скор: вход/мин/выход"),
    ("{0} {1} — {2} s lost", "{0} {1} — потеряно {2} с"),
    ("{0} m {1}", "на {0} м {1}"),
    ("later", "позже"),
    ("earlier", "раньше"),
    ("{0} s {1}", "на {0} с {1}"),
    (
        "Corners: losses over {0}s only",
        "Повороты: только потери больше {0}с",
    ),
    ("Corners: showing every corner", "Повороты: показаны все"),
    (
        "  {0} of {1} laps — one lap cannot tell the car from the driving.",
        "  Кругов {0} из {1} — одного круга мало, чтобы отличить машину от пилотажа.",
    ),
    (
        "Startup card  [{0}] installs, [{1}] removes, [{2}] diagnostics",
        "Карточка при запуске  [{0}] ставит, [{1}] удаляет, [{2}] проверка",
    ),
    ("OVERVIEW", "ОБЗОР"),
    ("CORNERS", "ПОВОРОТЫ"),
    ("TELEMETRY", "ТЕЛЕМЕТРИЯ"),
    ("DYNAMICS", "ДИНАМИКА"),
    ("ENGINE", "ДВИГАТЕЛЬ"),
    ("TRACTION", "СЦЕПЛЕНИЕ"),
];

/// Look a string up, or hand back what it was given.
///
/// The fallback is deliberate. A word nobody has translated yet shows in
/// English, which a Russian-speaking driver can still read in context; the
/// alternative — failing, or printing a key — turns a missing translation into
/// a broken screen.
pub fn translate(text: &str, russian: bool) -> &str {
    // `"grip|short"` is one English word in two places that want two different
    // Russian ones — the whole word in a sentence, an abbreviation in a column
    // four characters wide. English shows what is before the bar and never the
    // context; Russian gets an entry of its own for each. Every translation
    // system grows some form of this, because one word in two places is
    // genuinely two words in a language that inflects.
    let english = text.split('|').next().unwrap_or(text);
    if !russian {
        return english;
    }
    CATALOGUE
        .iter()
        .find(|(key, _)| *key == text)
        .map(|(_, translated)| *translated)
        .unwrap_or(english)
}

/// A translated sentence with values dropped into it.
///
/// [`translate`] handles a word. This handles the other half of what the code
/// used to write twice — a sentence with a number in the middle of it:
///
/// ```ignore
/// tr_fmt("Rear dropping too much at high speed (-{0} mm)", ru,
///        &[&format!("{rake_loss:.1}")])
/// ```
///
/// The placeholders are `{0}`, `{1}` and so on, and the **catalogue holds the
/// whole sentence** rather than the pieces of it. That matters more than it
/// looks: splitting a sentence into fragments and joining them with `format!`
/// gets the words translated and leaves everything between them in English, so
/// a Russian driver reads "-12.4 mm" where the old code said "-12.4 мм". Word
/// order goes the same way — a language that puts the number last cannot be
/// served by a fixed English skeleton.
///
/// Values arrive already formatted, because `{:.1}` and `{:>8}` belong to the
/// code and not to a translator. A key with no entry falls back to English,
/// exactly as a single word does.
pub fn tr_fmt(template: &str, russian: bool, args: &[&str]) -> String {
    let mut out = translate(template, russian).to_string();
    for (index, value) in args.iter().enumerate() {
        out = out.replace(&format!("{{{index}}}"), value);
    }
    out
}

/// `"Tyres".tr(ru)`, which is short enough to use everywhere it is needed.
///
/// An extension trait rather than a macro: the call site stays a plain
/// expression, so it works inside `format!` arguments, `match` arms and struct
/// literals without any of them having to know it is there.
pub trait Translate {
    /// The Russian for this, if `russian` and if there is one.
    fn tr(&self, russian: bool) -> &str;

    /// The same, from a [`Language`] rather than a flag — for the call sites
    /// that have one and would otherwise have to make the flag first.
    fn tr_lang(&self, language: Language) -> &str {
        self.tr(language == Language::Russian)
    }
}

impl Translate for str {
    fn tr(&self, russian: bool) -> &str {
        translate(self, russian)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_returned_unchanged() {
        assert_eq!("Tyres".tr(false), "Tyres");
        // Including for words that do have a translation: the flag decides,
        // not the presence of an entry.
        assert_eq!("Brakes".tr(false), "Brakes");
    }

    #[test]
    fn russian_comes_from_the_catalogue() {
        assert_eq!("Tyres".tr(true), "Шины");
        assert_eq!("All four".tr(true), "Все шины");
    }

    /// A word with no entry reads in English rather than breaking the screen.
    #[test]
    fn a_missing_translation_falls_back_rather_than_failing() {
        assert_eq!("Kerb strike".tr(true), "Kerb strike");
    }

    /// A sentence keeps what is *between* its values, which is the whole reason
    /// the catalogue holds templates rather than fragments.
    #[test]
    fn a_template_translates_before_its_values_arrive() {
        // The unit is inside the sentence, so it is translated with it — glue
        // the pieces together in code instead and the Russian reads "mm".
        assert_eq!(
            tr_fmt("{0} laps on this set", false, &["7"]),
            "7 laps on this set"
        );
        // Unknown template: English, values still filled in.
        assert_eq!(
            tr_fmt("{0} of {1} slots used", true, &["3", "8"]),
            "3 of 8 slots used"
        );
    }

    /// The same value can appear twice, and an argument nobody references is
    /// not an error — a template may legitimately drop one in one language.
    #[test]
    fn a_placeholder_may_repeat_and_a_spare_argument_is_harmless() {
        assert_eq!(tr_fmt("{0}-{0}", false, &["x"]), "x-x");
        assert_eq!(tr_fmt("{0} only", false, &["a", "b"]), "a only");
    }

    /// Two entries for one key means the second is dead and the first may not
    /// be the one somebody edited.
    #[test]
    fn no_english_word_is_translated_twice() {
        let mut seen: Vec<&str> = CATALOGUE.iter().map(|(english, _)| *english).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(
            before,
            seen.len(),
            "the catalogue has a duplicate English key; \
             the later entry is unreachable"
        );
    }

    /// An entry that translates a word to itself is either a mistake or a
    /// reminder somebody left. Both are worth failing over: the first is wrong
    /// and the second belongs in a comment.
    #[test]
    fn nothing_is_translated_to_itself() {
        for (english, russian) in CATALOGUE {
            assert_ne!(
                english, russian,
                "{english} is its own translation, which is not a translation"
            );
        }
    }
}
