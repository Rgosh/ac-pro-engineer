-- The panel's own words.
--
-- The advice already arrives translated — the desktop application writes it in
-- whichever language it runs in — and it read badly next to English captions,
-- so the captions follow the same language.
--
-- The language is not a setting of the panel's: two places to set one thing is
-- how they end up disagreeing, and the application is the one that already
-- writes the advice. `acpe.frame` calls `speak` from the frame's flags.

local M = {}

local RUSSIAN = {
  ['KM/H'] = 'КМ/Ч',
  ['MPH'] = 'МИЛЬ/Ч',
  ['LIMITER'] = 'ЛИМИТЕР',
  ['TYRES & BRAKES'] = 'ШИНЫ И ТОРМОЗА',
  ['BRAKES'] = 'ТОРМОЗА',
  ['DELTA'] = 'ДЕЛЬТА',
  ['BEST'] = 'ЛУЧШИЙ',
  ['LAST'] = 'ПОСЛЕДНИЙ',
  ['FUEL'] = 'ТОПЛИВО',
  ['LAPS LEFT'] = 'КРУГОВ',
  ['PER LAP'] = 'НА КРУГ',
  ['SESSION'] = 'СЕССИЯ',
  ['POS'] = 'МЕСТО',
  ['LAP'] = 'КРУГ',
  ['CURRENT'] = 'ТЕКУЩИЙ',
  ['ENGINEER'] = 'ИНЖЕНЕР',
  ['nothing to report'] = 'без замечаний',
  ['AC Pro Engineer is not running'] = 'AC Pro Engineer не запущен',
  ['Start the desktop application to see telemetry.'] =
    'Запусти приложение, чтобы увидеть телеметрию.',
  ['Last frame %.0f s ago.'] = 'Последний кадр %.0f с назад.',
  ['The shared mapping is not there yet. Start the desktop application — it creates the mapping, and this panel picks it up within a couple of seconds.'] =
    'Общий маппинг ещё не создан. Запустите программу — она его создаёт, и панель подхватит его за пару секунд.',
  ["On Linux shm-bridge.exe must be running in the game's Proton prefix as well — the panel cannot see the mapping without it."] =
    'На Linux в префиксе Proton должен работать ещё и shm-bridge.exe — без него панель не видит маппинг.',
  ['the application writes frame v%d, this panel reads v%d'] =
    'программа пишет кадр v%d, эта панель читает v%d',
  ['panel v%s — reinstall it from the desktop application'] =
    'панель v%s — переустановите её из программы',
  ['Nothing has been published yet.'] = 'Данные ещё не публиковались.',
  ['Shared memory unavailable'] = 'Общая память недоступна',
  ['Version mismatch'] = 'Версии не совпадают',
  ['Waiting for AC Pro Engineer'] = 'Жду AC Pro Engineer',
  ['Waiting for the car'] = 'Жду машину',
  ['AC Pro Engineer is running. Telemetry starts when you go on track.'] =
    'AC Pro Engineer работает. Телеметрия появится, когда выедешь на трассу.',
  ['app'] = 'прил.',
  ['car'] = 'машина',
  ['on track'] = 'на трассе',
  ['in the garage'] = 'в боксах',
  ['CAR'] = 'МАШИНА',
  ['TIMING'] = 'ВРЕМЯ КРУГА',
  ['CORNERS'] = 'КОЛЁСА',
  ['FLAGS'] = 'ФЛАГИ',
  ['LINK'] = 'СВЯЗЬ',
  ['FRAME'] = 'КАДР',
  ['PANEL'] = 'ПАНЕЛЬ',
  ['panel'] = 'панель',
  ['frame'] = 'кадр',
  ['Panel %s is installed — restart Assetto Corsa to load it'] =
    'Установлена панель %s — перезапусти Assetto Corsa, чтобы она загрузилась',
  ['Tell me when a newer panel is installed'] =
    'Сообщать об установке новой панели',

  -- The settings window, so the tabs and the switches read in one language.
  ['Panel'] = 'Панель', ['Advice'] = 'Советы', ['Look'] = 'Вид',
  ['Units'] = 'Единицы', ['Console'] = 'Консоль', ['Dev'] = 'Разраб',
  ['Blocks'] = 'Блоки', ['Corners'] = 'Колёса', ['Limits'] = 'Пороги',
  ['Fields'] = 'Поля', ['State'] = 'Состояние', ['Screen'] = 'Экран',
  ['Size'] = 'Размер', ['Colour'] = 'Цвет', ['Switches'] = 'Ключи',
  ['Data'] = 'Данные', ['Link'] = 'Связь',

  ['Speed and gear'] = 'Скорость и передача',
  ['RPM bar'] = 'Полоса оборотов',
  ['Tyres and brakes'] = 'Шины и тормоза',
  ['Lap timing'] = 'Время круга',
  ['Fuel'] = 'Топливо',
  ['Session'] = 'Сессия',
  ['Engineer advice'] = 'Советы инженера',
  ['Section captions'] = 'Подписи секций',
  ['LIMITER badge'] = 'Значок лимитера',
  ['One-line mode'] = 'Одной строкой',
  ['Shift light'] = 'Индикатор переключения',
  ['Tyre temperature'] = 'Температура шин',
  ['Brake temperature'] = 'Температура тормозов',
  ['Wear'] = 'Износ',
  ['Distance from target'] = 'Отклонение от цели',
  ['Delta'] = 'Дельта',
  ['Best lap'] = 'Лучший круг',
  ['Last lap'] = 'Последний круг',
  ['In the tank'] = 'В баке',
  ['Laps left'] = 'Кругов осталось',
  ['Per lap'] = 'На круг',
  ['Position'] = 'Позиция',
  ['Lap number'] = 'Номер круга',
  ['Current lap'] = 'Текущий круг',
  ['Track conditions'] = 'Условия трассы',
  ['Wrap long lines'] = 'Переносить длинные строки',
  ['Highlight advice'] = 'Подсвечивать советы',
  ['Space between lines'] = 'Отступ между строками',
  ['Rule between lines'] = 'Линия между строками',
  ['Show how many are hidden'] = 'Показывать скрытые',
  ['Upper case'] = 'Верхний регистр',
  ['Number the lines'] = 'Нумеровать строки',
  ['Spell the severity'] = 'Важность словом',
  ['Grow with the window'] = 'Расти вместе с окном',
  ['VR mode'] = 'Режим VR',
  ['Celsius'] = 'Цельсий',
  ['PSI'] = 'PSI',
  ['Miles per hour'] = 'Мили в час',
  ['Gallons'] = 'Галлоны',
  ['Short lap times'] = 'Короткие времена',
  ['Unit suffixes'] = 'Суффиксы единиц',
  ['Save now'] = 'Сохранить',
  ['Reset to defaults'] = 'Сбросить',
  ['Default palette'] = 'Палитра по умолчанию',
  ['settings are saved as you change them'] = 'настройки сохраняются сразу',
  ['could not write a settings file'] = 'не удалось записать файл настроек',
  ["CSP's own storage is working too"] = 'хранилище CSP тоже работает',
  ['storage unavailable: settings last for this session'] =
    'хранилище недоступно: настройки только на эту сессию',
  ['%d saved'] = 'сохранено: %d',
  ['%d saved, %d would not stick'] = 'сохранено: %d, не записалось: %d',
  ['everything'] = 'всё',
  ['warnings and worse'] = 'предупреждения и хуже',
  ['critical only'] = 'только критичное',
  ['compact'] = 'плотно', ['normal'] = 'обычно', ['large'] = 'крупно',
  ['SECTIONS'] = 'СЕКЦИИ', ['UNITS'] = 'ЕДИНИЦЫ', ['FORMAT'] = 'ФОРМАТ',
  ['SHOW'] = 'ПОКАЗЫВАТЬ', ['LINES'] = 'СТРОКИ', ['MARKER'] = 'МАРКЕР',
  ['the application is sending %d of %d'] = 'приложение шлёт %d из %d',
  ['ACCENT'] = 'АКЦЕНТ', ['PALETTE'] = 'ПАЛИТРА', ['SCREEN'] = 'ЭКРАН',
  ['PRESSURE'] = 'ДАВЛЕНИЕ', ['COLUMNS'] = 'КОЛОНКИ', ['QUICK'] = 'БЫСТРО',
  ['COMMAND'] = 'КОМАНДА', ['OUTPUT'] = 'ВЫВОД',
  ['TYRE TEMPERATURE'] = 'ТЕМПЕРАТУРА ШИН',
  ['BRAKE TEMPERATURE'] = 'ТЕМПЕРАТУРА ТОРМОЗОВ',
  ['PER CORNER'] = 'ПО КОЛЁСАМ',
}

local speakRussian = false

--- Follow the application. Called once per settled frame, from `acpe.frame`.
function M.speak(russian)
  speakRussian = russian and true or false
end

function M.russian()
  return speakRussian
end

--- Say it in the application's language, or say it unchanged.
---
--- A missing entry returns the English, which is the right answer twice over:
--- it is readable, and it is visibly untranslated to anyone looking for gaps.
function M.tr(text)
  if not speakRussian then return text end
  return RUSSIAN[text] or text
end

M.RUSSIAN = RUSSIAN

return M
