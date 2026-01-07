use crate::config::Language;

pub fn tr(key: &str, lang: &Language) -> String {
    match lang {
        Language::English => match key {
            // General
            "settings_title" => "SETTINGS",
            "cat_general" => "General",
            "cat_units" => "Units",
            "cat_alerts" => "Alerts",
            "cat_theme" => "Theme Colors",
            
            // General Items
            "lang" => "Language",
            "update_rate" => "Update Rate (ms)",
            "history_size" => "History Size",
            "auto_save" => "Auto Save",
            
            // Units Items
            "unit_pressure" => "Pressure Unit",
            "unit_temp" => "Temperature Unit",
            "unit_speed" => "Speed Unit",
            
            // Alerts Items
            "alert_p_min" => "Min Tyre Pressure",
            "alert_p_max" => "Max Tyre Pressure",
            "alert_t_min" => "Min Tyre Temp",
            "alert_t_max" => "Max Tyre Temp",
            "alert_b_max" => "Max Brake Temp",
            "alert_fuel" => "Fuel Warning (Laps)",
            "alert_wear" => "Wear Warning (%)",
            
            // Footer
            "footer_connected" => "CONNECTED",
            "footer_disconnected" => "DISCONNECTED - Waiting for AC...",
            "footer_keys" => "F1-F7: Tabs | Q: Quit | Settings: Enter/Arrows | Setup: Up/Down",
            
            // Tabs
            "tab_dash" => "DASHBOARD",
            "tab_tele" => "TELEMETRY",
            "tab_eng" => "ENGINEER",
            "tab_setup" => "SETUP",
            "tab_anal" => "ANALYSIS",
            "tab_strat" => "STRATEGY",
            "tab_set" => "SETTINGS",
            
            // Dashboard
            "dash_tyre_status" => "TYRE STATUS",
            "dash_perf" => "PERFORMANCE",
            "dash_session" => "SESSION INFO",
            "dash_quick" => "QUICK INFO",
            "avg_press" => "Avg Pressure",
            "avg_temp" => "Avg Temp",
            "avg_wear" => "Avg Wear",
            "lbl_speed" => "SPEED",
            "lbl_rpm" => "RPM",
            "lbl_gear" => "GEAR",
            "lbl_delta" => "DELTA",
            "lbl_throttle" => "Throttle",
            "lbl_brake" => "Brake",
            "lbl_car" => "Car",
            "lbl_track" => "Track",
            "lbl_session" => "Session",
            "lbl_laps" => "Laps",
            "lbl_pos" => "Position",
            "lbl_fuel" => "Fuel",
            "lbl_tc" => "TC",
            "lbl_abs" => "ABS",
            "lbl_map" => "Engine Map",
            
            // Engineer
            "eng_recs" => "RECOMMENDATIONS",
            "eng_analysis" => "DRIVING ANALYSIS",
            "eng_good" => "All systems operating within optimal parameters.",
            "eng_push" => "Keep pushing! 🏎️💨",
            "eng_action" => "Action",
            "eng_smooth" => "Smoothness",
            "eng_aggr" => "Aggression",
            "eng_trail" => "Trail Braking",
            "eng_lock" => "Lockups",
            "eng_spin" => "Wheel Spin",
            
            // Views Placeholders
            "view_setup" => "Setup view - Compare and adjust car setup",
            "view_anal" => "Analysis view - Lap analysis and comparisons",
            "view_strat" => "Strategy view - Race strategy and pit stops",
            "view_tele" => "Telemetry view - Detailed graphs and data",

            // SETUP KEYS
            "set_list" => "AVAILABLE SETUPS",
            "set_compare" => "COMPARISON",
            "set_param" => "Parameter",
            "set_val" => "Value",
            "set_diff" => "Diff",
            "set_no_file" => "No setups found for this car/track",
            
            "grp_gen" => "GENERAL",
            "grp_tyres" => "TYRES",
            "grp_aero" => "AERO",
            "grp_align" => "ALIGNMENT",
            "grp_susp" => "SUSPENSION",
            "grp_damp" => "DAMPERS",
            "grp_driv" => "DRIVETRAIN",
            "grp_brake" => "BRAKES",
            
            "p_fuel" => "Fuel",
            "p_bias" => "Brake Bias",
            "p_limiter" => "Engine Limiter",
            "p_press" => "Pressure",
            "p_wing" => "Wing",
            "p_camber" => "Camber",
            "p_toe" => "Toe",
            "p_spring" => "Spring Rate",
            "p_rod" => "Height (Rod)",
            "p_arb" => "ARB",
            "p_bump" => "Bump",
            "p_reb" => "Rebound",
            "p_f_bump" => "Fast Bump",
            "p_f_reb" => "Fast Rebound",
            "p_diff_p" => "Diff Power",
            "p_diff_c" => "Diff Coast",
            "p_gear" => "Gear",
            "p_final" => "Final Ratio",
            
            _ => key,
        }.to_string(),
        
        Language::Russian => match key {
            // Общее
            "settings_title" => "НАСТРОЙКИ",
            "cat_general" => "Общие",
            "cat_units" => "Единицы изм.",
            "cat_alerts" => "Оповещения",
            "cat_theme" => "Тема и цвета",
            
            // Пункты Общие
            "lang" => "Язык",
            "update_rate" => "Частота обн. (мс)",
            "history_size" => "История (кадров)",
            "auto_save" => "Автосохранение",
            
            // Пункты Единицы
            "unit_pressure" => "Давление",
            "unit_temp" => "Температура",
            "unit_speed" => "Скорость",
            
            // Пункты Оповещения
            "alert_p_min" => "Мин. Давление шин",
            "alert_p_max" => "Макс. Давление шин",
            "alert_t_min" => "Мин. Темп. шин",
            "alert_t_max" => "Макс. Темп. шин",
            "alert_b_max" => "Макс. Темп. тормозов",
            "alert_fuel" => "Топливо (кругов)",
            "alert_wear" => "Износ шин (%)",
            
             // Футер
            "footer_connected" => "ПОДКЛЮЧЕНО",
            "footer_disconnected" => "ОТКЛЮЧЕНО - Ожидание AC...",
            "footer_keys" => "F1-F7: Меню | Q: Выход | Настройки: Enter/Стрелки | Сетап: Вверх/Вниз",
            
            // Вкладки
            "tab_dash" => "ДАШБОРД",
            "tab_tele" => "ТЕЛЕМЕТРИЯ",
            "tab_eng" => "ИНЖЕНЕР",
            "tab_setup" => "СЕТАП",
            "tab_anal" => "АНАЛИЗ",
            "tab_strat" => "СТРАТЕГИЯ",
            "tab_set" => "НАСТРОЙКИ",
            
            // Dashboard
            "dash_tyre_status" => "СОСТОЯНИЕ ШИН",
            "dash_perf" => "ПРОИЗВОДИТЕЛЬНОСТЬ",
            "dash_session" => "ИНФО СЕССИИ",
            "dash_quick" => "БЫСТРОЕ ИНФО",
            "avg_press" => "Ср. Давление",
            "avg_temp" => "Ср. Темп.",
            "avg_wear" => "Ср. Износ",
            "lbl_speed" => "СКОРОСТЬ",
            "lbl_rpm" => "ОБ/МИН",
            "lbl_gear" => "ПЕРЕДАЧА",
            "lbl_delta" => "ДЕЛЬТА",
            "lbl_throttle" => "Газ",
            "lbl_brake" => "Тормоз",
            "lbl_car" => "Авто",
            "lbl_track" => "Трасса",
            "lbl_session" => "Сессия",
            "lbl_laps" => "Круги",
            "lbl_pos" => "Позиция",
            "lbl_fuel" => "Топливо",
            "lbl_tc" => "TC",
            "lbl_abs" => "ABS",
            "lbl_map" => "Карта",
            
             // Engineer
            "eng_recs" => "РЕКОМЕНДАЦИИ",
            "eng_analysis" => "АНАЛИЗ ВОЖДЕНИЯ",
            "eng_good" => "Все системы работают в оптимальном режиме.",
            "eng_push" => "Так держать! 🏎️💨",
            "eng_action" => "Действие",
            "eng_smooth" => "Плавность",
            "eng_aggr" => "Агрессия",
            "eng_trail" => "Трейл-брейкинг",
            "eng_lock" => "Блок. колес",
            "eng_spin" => "Пробуксовка",
            
            // Views Placeholders
            "view_setup" => "Меню Сетапа - Сравнение и настройка автомобиля",
            "view_anal" => "Меню Анализа - Анализ кругов и сравнение",
            "view_strat" => "Меню Стратегии - Стратегия гонки и пит-стопы",
            "view_tele" => "Меню Телеметрии - Детальные графики и данные",

            // SETUP KEYS
            "set_list" => "ДОСТУПНЫЕ СЕТАПЫ",
            "set_compare" => "СРАВНЕНИЕ",
            "set_param" => "Параметр",
            "set_val" => "Знач.",
            "set_diff" => "Разн.",
            "set_no_file" => "Сетапы не найдены для этого авто/трассы",
            
            "grp_gen" => "ОБЩИЕ",
            "grp_tyres" => "ШИНЫ",
            "grp_aero" => "АЭРО",
            "grp_align" => "ГЕОМЕТРИЯ",
            "grp_susp" => "ПОДВЕСКА",
            "grp_damp" => "АМОРТИЗАТОРЫ",
            "grp_driv" => "ТРАНСМИССИЯ",
            "grp_brake" => "ТОРМОЗА",
            
            "p_fuel" => "Топливо",
            "p_bias" => "Баланс Тормозов",
            "p_limiter" => "Лимитер Двс",
            "p_press" => "Давление",
            "p_wing" => "Крыло",
            "p_camber" => "Развал",
            "p_toe" => "Схождение",
            "p_spring" => "Пружины",
            "p_rod" => "Высота (Шток)",
            "p_arb" => "Стаб.",
            "p_bump" => "Сжатие",
            "p_reb" => "Отбой",
            "p_f_bump" => "Быстр. Сжатие",
            "p_f_reb" => "Быстр. Отбой",
            "p_diff_p" => "Дифф. Тяга",
            "p_diff_c" => "Дифф. Накат",
            "p_gear" => "Передача",
            "p_final" => "Главная пара",
            
            _ => key,
        }.to_string(),
    }
}