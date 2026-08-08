# Changelog - RaceEngineer (AC Pro Engineer)

All notable changes to this project will be documented in this file.

## [Unreleased]

### 🗑 Удалено

- **Оверлей на F10 и «центр управления» на F11.** Это был второй оверлей —
  своё окно Win32 поверх игры, которое рисовало само приложение. На Linux у
  него вообще не было исполнителя: `OverlayManager` выбирал `None`, F10 писала
  строчку в лог и больше ничего не делала. На Windows он рисовал худшую копию
  того, что уже рисует Lua-панель, не переживал полноэкранный режим, не
  появлялся в VR и был невидим для скриншотов самой AC. Вместе с ним ушли
  `OverlayManager`, `native_window.rs` (422 строки Win32), `openxr.rs`
  (заглушка, которая никогда ничего не нарисовала), `provider.rs`, `state.rs`,
  `ui/overlay.rs`, режимы `OverlayMode`, флаги `--overlay-test-d` и
  `--overlay-test-vr` и две привязки клавиш. Оверлей теперь один — панель CSP.
- Побочный эффект: флаги `SHOW_TELEMETRY` и `SHOW_ENGINEER` в кадре брались как
  «И» настройки и второго переключателя на стороне того менеджера. Второй
  никто никогда не выключал, но выяснить это можно было только через две
  структуры. Теперь в кадр идёт ровно то, что выбрано в настройках.

### 🖼 Скриншоты

- **SVG больше нет — только PNG.** Каждый экран лежал в двух файлах: SVG «как
  точная запись» и PNG «чтобы показать». SVG не читал никто, GitHub его в
  README всё равно не покажет, а обновление скриншотов клало в диф вдвое
  больше. SVG остался как промежуточное представление в памяти, из которого
  растрируется PNG, — рисовать сетку цветных глифов иначе означало бы тащить
  растеризатор шрифтов. `Ctrl+S` в программе тоже сохраняет PNG.
- **Скриншоты самой панели, по окну на картинку.** Раньше в README была ровно
  одна картинка «оверлея» — и на ней был терминальный центр управления тем
  оверлеем, которого больше нет. Теперь есть все пять окон панели и все шесть
  вкладок её настроек, отрисованные настоящим кодом панели.
  `apps/lua/love/portraits.sh` генерирует их заново: каждый прогон рисует одно
  окно в окне LÖVE ровно его размера, так что обрезать нечего.

### 🐞 Исправлено в панели и стенде

- **Вложенная `ui.tabBar` ломала внешнюю.** Стенд сбрасывал текущую панель
  вкладок в `nil` вместо того, чтобы вернуть родительскую, — поэтому все
  `ui.tabItem` внешней панели *после* вложенной не рисовали ярлык и выполняли
  своё тело безусловно. Окно настроек теряло четыре вкладки из шести, а их
  содержимое рисовалось стопкой под выбранной.
- **`##id` печатался как часть подписи.** ImGui прячет всё от `##` и дальше;
  панель на этом держится — почти каждый переключатель это
  `ui.checkbox('##showHeader')` с подписью, нарисованной рядом нужным
  размером, потому что тиры шрифтов CSP не масштабируются. В стенде перед
  каждой настройкой стояло `##showHeader`.
- **Показания по колёсам уезжали за правый край окна телеметрии.** `row`
  рассчитан на «44.82 L»: значение на 46 % ширины крупным шрифтом. Строка
  «26.8 psi 90°C 521°C 98%» не влезала ни при каком размере окна — текст
  масштабируется вместе с шириной, поэтому доля вылета одна и та же. Теперь у
  таких строк узкая колонка подписи и мелкий шрифт значения. Имя маппинга в
  окне состояния теряло `.v1` по той же причине — а это ровно тот символ,
  ради которого туда и смотрят.
- **Окно `status` в стенде отсутствовало**, хотя манифест объявляет его наравне
  с остальными. Пятое окно рисовалось только в игре.

### 🧱 Структура

- **Lua-панель разложена по модулям.** Был один файл на 2 429 строк; стало
  `ac_pro_engineer.lua` (точка входа, 138 строк) и `acpe/` — настройки, язык,
  тема, вёрстка, форматирование, кадр, блоки, виджеты, консоль и по файлу на
  каждое окно в `acpe/windows/`. Слои идут в одну сторону:
  settings → i18n/theme → layout → format → frame → blocks → windows.
  Установщик кладёт дерево целиком (19 файлов), удаление убирает и папки;
  тест `every_lua_file_in_the_app_folder_is_shipped` не даст забыть новый
  модуль.
- Версия панели по-прежнему равна версии релиза и проверяется тестами —
  `PANEL_VERSION`, `VERSION` в манифесте и `Cargo.toml` обязаны совпадать.

### 🐞 Исправлено

- **«Прогноз жизни шин» рисовал подписи поверх собственных полосок.**
  `Gauge::label` центрирует текст по бару, поэтому строка наполовину тонула в
  цветном прямоугольнике. Теперь три колонки: колесо и процент слева, полоса в
  середине, круги справа. Полоса масштабируется между вашим порогом
  `wear_critical` и новой шиной, а не между 94 % и 100 % — так пустая полоса
  значит «конец по вашему же порогу», а не «ниже 94».
- **`Calc...`** — оборванная на середине фраза, висевшая весь стинт. Прогноз
  требует завершённого круга; пока его нет, стоит прочерк.
- **Цвета полос берутся из тех же порогов**, что и советы инженера, — раньше в
  них было зашито 98/96.
- **«Кругов осталось» считалось до `wear_warning − 2`**, а не до реального
  конца шины, и «нет данных» кодировалось значением 99.0 — то есть свежий
  комплект на коротком круге выглядел как отсутствие данных.

### 📖 Документация

- **README переписан целиком.** Установка отдельно для Windows и для Linux,
  раздел про игровую панель и мост, все девять экранов со скриншотами, полная
  таблица клавиш, **все параметры запуска** (их не было ни одного) и все
  переменные окружения, справочник по `config.json` — каждый ключ со значением
  по умолчанию, и раздел «что делать, если» с симптомами, причинами и
  проверками по порядку. Ключевые слова для поиска и ссылки на разделы —
  чтобы на вопрос про программу можно было ответить, не открывая исходники.
- **Окна панели описаны там же, где остальные экраны.** Раздел «Every screen»
  теперь охватывает обе половины программы: девять вкладок терминала, потом
  пять окон панели и все шесть вкладок её настроек — каждое с описанием того,
  что в нём лежит и зачем оно, а не с подписью в строчку. Описания написаны по
  коду панели: что означает каждый порог, почему цветовые пределы ваши, почему
  палитре нужны и образец, и пипетка, и какая команда консоли — единственный
  способ включить режим разработчика. В разделе про оверлей остались установка
  и мост.
- **`ac_pro_engineer --help` наконец что-то говорит.** У пяти флагов из семи не
  было ни строчки описания.
- **Скриншоты — PNG вместо SVG**, отрисованные из тех же буферов; GitHub
  показывает их одинаково у всех. Заодно в них появились износ шин, времена
  кругов, остаток сессии и карта двигателя — раньше всё это было нулями и
  выглядело как сломанный интерфейс.

### ✨ Добавлено

- **`shm-bridge.exe --verify`** — открывает маппинг оверлея ровно тем вызовом,
  которым его открывает CSP, и печатает, что в нём лежит: версию кадра, счётчик
  и версию приложения. Это единственный вопрос, на который изнутри префикса
  ответить было нельзя, — «видит ли вообще Windows-процесс здесь наш кадр».
  ```
  protontricks-launch --appid 244210 shm-bridge.exe --verify
  ```
  Если маппинга нет — говорит, что запускать; если он есть и пустой — что
  приложение на стороне Linux не публикует.

## [v0.3.5] - 2026-08-07

**Главное:** оверлей перестал быть «панелькой, которая иногда работает». Он
открывается до заезда и в боксах, помнит настройки после закрытия окна, и
показывает до восьми советов вместо четырёх. В терминале клавиши наконец
делают то, что написано внизу вкладки — и переназначаются.

> Оверлей всё ещё не проверялся автором изменений внутри игры: панель гоняется
> под LuaJIT и LÖVE, каждый её вызов `ui.*` сверяется с установленным CSP, но
> это не то же самое, что сессия. Сообщайте, что сломалось.

### ⚠️ Ломающее

- **Кадр оверлея — версия 5.** Советов теперь восемь, а не четыре, структура
  выросла с 440 до 712 байт. **`shm-bridge.exe` нужно обновить** — мост,
  собранный под 440 байт, маппит слишком мало, CSP молча отказывается открыть
  маппинг, и панель бесконечно «ждёт AC Pro Engineer». Кнопка **[B]** на
  карточке оверлея тянет свежий.

### ✨ Добавлено

- **Сколько строк совета выводить — ползунок 1–8**, а не четыре радиокнопки.
  Рядом написано, сколько строк реально прислало приложение: «поставил 8, вижу
  3» — это инженеру есть что сказать только три раза, а не сломанная настройка.
  В приложении тот же предел поднят до 8 (Настройки → ОВЕРЛЕЙ).
- **Панель работает до заезда и в боксах.** Приложение публикует кадр и с
  экрана лаунчера, и когда AC ещё ничего не отдаёт. Панель различает два
  состояния: «жду машину» (всё в порядке, телеметрия начнётся на трассе) и «жду
  AC Pro Engineer» (приложение не запущено). Раньше было только второе — и оно
  отправляло искать несуществующую поломку в мосте и префиксе Proton. В окне
  статуса появилась строка `машина: в боксах / на трассе`.
- **Свои клавиши** — новая категория **KEYS [G]** в настройках терминала.
  ENTER назначает, DEL возвращает стандарт, ESC отменяет; клавиша, уже занятая
  другим действием, не записывается молча, а объясняет, кем занята. Работает
  одинаково на Linux и Windows, хранится в `config.json` текстом
  (`f10`, `ctrl+s`, `shift+tab`) — можно править руками. Раскладка учитывается:
  привязал на `s` — работает и `ы`.

### 🐞 Исправлено

- **Подсказки клавиш врали.** Справа снизу теперь на **каждой** вкладке своя
  строка, и собирается она из тех же привязок, которые обрабатывают нажатие —
  соврать физически не может, а тест `the_hints_only_name_keys_that_do_something`
  проходит по каждой вкладке и требует, чтобы названная клавиша делала именно
  то, что написано. Раньше подсказки были на двух вкладках из девяти, и одна из
  двух обещала `'D' — Download` на экране, где `D` не обрабатывалась вовсе.
  Заодно: `D` в списке сетапов теперь открывает браузер (там есть что качать),
  а в подсказке Analysis появился `E` — экспорт CSV, который работал всё это
  время и нигде не был написан.
- **Строка помощи (F1) и строка лаунчера** тоже печатаются из привязок.
  Лаунчер называл две клавиши из шести: `←/→`, `O`, `H` и `Q` работали и нигде
  не упоминались.
- **«F1: Dashboard», «F5: Analysis» — вкладки никогда не были на F-клавишах.**
  Так было написано в заголовках README, в заголовках страниц помощи и в
  тексте руководства («Look at the Analysis Tab (F5)»), а переключались вкладки
  цифрами. Заголовки страниц помощи теперь печатаются из привязки, README и
  руководство говорят про цифры и про вкладку, а не про клавишу.
- **Панель забывала все настройки при закрытии окна.** В манифесте стояло
  `LAZY = FULL` — CSP выгружает скрипт, когда закрыто последнее окно. Закрыл
  панель посмотреть на трассу, открыл — настройки по умолчанию. Плюс сохранение
  полагалось на самоприсваивание значения в прокси хранилища, чего проверить
  изнутри нельзя. Теперь `LAZY = ON`, пишутся только реально изменившиеся ключи,
  и каждый перечитывается после записи. Кнопка «Сохранить» говорит, сколько
  записалось.
- **Плашка под советами инженера занимала угол окна.** Прямоугольник рисовался
  высотой ровно 140 пикселей: в окне советов это полоса сверху, а текст уходил
  за её нижний край. Теперь в окне советов плашка занимает окно, в панели —
  высоту самого блока, и поля симметричные (было 4 слева, 3 сверху, 0 справа).
- **`A / S / D` в подписи категорий настроек** — категорий пять, а названы были
  три. Две новые открывались только стрелками. Теперь `A/S/D/F/G`.
- **Названия категорий обрезались**: ширина считалась в байтах, поэтому
  «ОВЕРЛЕЙ» занимал вдвое больше, чем думала арифметика, и метка клавиши
  вылезала за край как `[F`.
- **«Dump settings to console»** писал семьдесят строк в буфер на двенадцать,
  так что видно было только конец алфавита. Три ключа в строку, сорок строк.

### 🔧 Ядро инженера

- **Четыре угла одной проблемы — теперь одна строка.** «FL COLD / FR COLD /
  RL COLD / RR COLD» занимало все слоты кадра и читалось как шум. Теперь это
  «All four COLD: 55 °C», а два колеса называются осью или стороной («Fronts»,
  «Rears», «Left side»). Так же свёрнуты давления, износ и тормоза. Гистерезис
  остался поколёсным — плоское пятно на одном колесе не сбрасывает таймеры
  остальных.
- **Износ больше не кричит на третьем круге.** Критичность считалась как
  `wear_warning − 2`, поэтому при стандартных настройках шина с остатком 93.9 %
  — то есть в середине первого стинта — приходила как CRITICAL «WORN OUT».
  Появился отдельный порог `wear_critical` (по умолчанию 85 %) и своя строка в
  Настройки → ИНЖЕНЕР.
- **Советы о давлении — в выбранных единицах.** Совет про температуру давно шёл
  через форматтер, а совет про давление печатал сырые psi: у тех, кто работает
  в bar, на дашборде было одно, а в совете о нём — другое.
- **Тормоза называются колёсами, а не номерами.** Было «Brake 1»…«Brake 4» —
  единственное место в приложении, где углы нумеровались.

## [v0.3.4] - 2026-08-05

> ### ⚠️ Оверлей в этом релизе — ДЕМО
>
> Превью, а не готовая функция. Публикуется, чтобы его можно было проверить на
> живых машинах — это единственное, чего нельзя сделать при разработке.
>
> - **На Windows не запускалось ни разу.** Тесты проходят кросс-компиляцией,
>   clippy чист под Windows-таргет, но ни одна строка там не выполнялась.
> - **Внутри игры автором изменений не проверялось.** Панель прогоняется под
>   LuaJIT и LÖVE, каждый её вызов `ui.*` сверяется с установленным CSP — это
>   не то же самое, что сессия.
> - Часть подписей консоли и префиксы `Wear:`, `T:`, `B:` пока не переведены.
>
> Официальный релиз — после реальных заездов на обеих системах. Сообщайте, что
> сломалось: окно статуса панели теперь показывает все версии сразу.

**Главное:** до этого релиза оверлей на Linux не мог работать ни у кого.
v0.3.3 был затегирован за одиннадцать минут до коммита, научившего
`shm-bridge` маппингу оверлея, поэтому все опубликованные мосты создавали
только собственные страницы AC. Проверено сканированием артефакта.

### ⚠️ Ломающее

- **Кадр оверлея — версия 4**: добавилась версия приложения, чтобы панель
  понимала, что игра рисует её старую копию. Поле последнее, смещения не
  сдвинулись. Приложение, панель и `shm-bridge.exe` должны быть из одного
  релиза; панель ставится сама, мост — по **[B]** на карточке оверлея.

### 🐞 Исправлено

- **Панель не загружалась вовсе.** `ui.Icons and ui.Icons.Settings` на уровне
  файла: `ui.Icons` достаточно быть truthy, чтобы его проиндексировали, а
  таблица-аргумент строится до `pcall`, так что тот её не защищает. Все окна
  рисовали текст ошибки вместо панели.
- **Оба переключателя dev-режима падали в nil.** `applyDemo` и `DEMO_ADVICE`
  стояли ниже вызывающих, то есть были для них глобалами. Четвёртый случай
  этой ловушки здесь.
- **Обе версии панели врали.** `manifest.ini` показывал `1.0` одиннадцать
  релизов подряд, собственной версии у панели не было.
- **Оба харнесса рапортовали OK на сломанной панели** — LuaJIT рисовал 27
  строк вместо 140, LÖVE не считал ошибкой падение при загрузке.
- **Карточка оверлея обрезала свою же диагностику** на 66 колонках.

### 🚀 Добавлено

- **Мост сообщает, кто он.** Пишет `/dev/shm/acpe-bridge.info` и вкомпиливает
  свою версию в бинарник, чтобы его можно было опознать и не запуская.
- **Карточка судит все три части**, а **[B]** качает опубликованный мост,
  проверяя его перед заменой. Старый сохраняется как `.previous`.
- **Проверка обновления моста при старте** — только смотрит; качает по клавише.
  Версия самого приложения этим путём не трогается.
- **Панель говорит, что игра держит её старую копию**, и предлагает
  перезапустить AC. Отключается в Panel → Blocks.
- **`bridge_probe`** — какой мост на диске, какой запущен, заработает ли оверлей.
- **`--export-overlay <папка>`** — выгрузить панель для ручной установки.
- **`proton-setup.sh` в архиве** — команды `protontricks`, без которых CSP не
  грузится вовсе. Шрифтов в архиве нет и быть не может: терминал рисует своим
  шрифтом, панель — через DirectWrite от CSP, а шрифты ставятся в префикс
  (`corefonts`), что и делает скрипт.


## [v0.3.2] - 2026-08-04

A small follow-up to v0.3.1. Four pieces of functionality that were fully
implemented but had no way to reach the user are now wired up, one wrong
number in the analysis tab is corrected, and three things that ran far more
often than they needed to no longer do.

### 🚀 New Features

- **Screenshot the interface with Ctrl+S.** A complete SVG renderer for a
  drawn terminal buffer already existed inside `tui_tester`, where it
  generates the images in the README; the application itself had no way to
  capture what it was showing. Frames are written to
  `<data>/screenshots/<timestamp>.svg` and the path is reported in the status
  line. SVG keeps the text selectable and needs no image encoder.
- **Tyre pressure targets are on screen.** `ColdPressureCalculator` and
  `TyrePressureOptimizer` were both fully implemented in `ac_core` and called
  only by the test suite. A third Engineer sub-tab shows what to set the tyres
  to cold so they reach the configured hot target at the current air
  temperature and track grip, and what each corner's inner-versus-outer
  temperature spread says to change.
- **Frame and tick timing in the footer.** The render loop and the background
  tick thread contend for the same state mutex, so when one stalls it is
  usually because the other holds the lock — and from the outside both look
  identical, because the numbers stop moving either way. The footer now shows
  frames per second and how long ago the tick completed, in red past 500ms.

### 🛡️ Fixed

- **A missing sector split no longer zeroes the best sector.** The analysis tab
  computed each best sector as a plain minimum over the raw values, which
  includes the zeroes left by a lap whose split was never captured and by the
  unused third slot of a two-sector track. One such lap pinned that sector to
  0.000 and made the "Optimal" row a lap time no car could set. The analyzer's
  own `theoretical_best_lap_ms` — which filters those out and had no callers
  outside its unit test — is used instead, and a sector with nothing recorded
  renders as a dash rather than as a time.
- **The config is no longer rewritten on every launch.** The decision to save
  compared the file's text against a re-serialisation, so different
  indentation, a different key order, or a serialisation failure all triggered
  a write. The comparison is now between values, and formatting stops
  mattering. Migration and validation still write, which they must.
- **The mouse is no longer captured.** Capture was enabled at startup and no
  mouse event was ever handled, so the only effect was taking selection and
  copy away from the terminal — which is how anyone gets a lap time or an
  error message out of a TUI and into a bug report.
- **The timing readout stays blank until a frame is measured**, rather than
  reporting a fabricated "0fps" before anything has been drawn.

### ⚡ Performance

- **The delta-versus-best series is cached.** It was recomputed every frame,
  and computing it resamples two telemetry traces — cloning and fully sorting
  up to 7200 points each — to arrive at an answer that cannot change, since
  both laps are finished.
- **Setup folders are rescanned on a ten second heartbeat** instead of twice a
  second. The scan walks three directory trees and parses every setup ini in
  them, for a directory that changes only when the user saves a setup from
  inside the game.

### 🧹 Internal

185 tests, up from 171. The SVG renderer moved out of `tui_tester` into
`ui::screenshot` so the binary and the application share one implementation;
the README screenshots regenerate byte-identical from it.

## [v0.3.1] - 2026-08-03

A bug-fix release, and a large one. Three features that the interface has
always advertised — the version carousel, saving your settings, and the Setup
Cloud browser — did not work at all and now do. Four reachable crashes are
gone. Assetto Corsa is finally found on Linux.

47 commits, 171 tests (up from 130), green on Linux and Windows.

### ⚠️ Read This First

- **Your cold tyre pressure targets will change.** The calculator scales its
  recommendation by `surface_grip`, which used to read a constant `0.0` and
  clamp to a floor of `0.80` — so every recommendation carried the same fixed
  compensation regardless of track state. With real grip being read, a
  well-rubbered track (≈0.94) produces roughly a third of the previous
  adjustment. Numbers will differ from v0.3.0 for the same car and track.
  This is the fix working, not a regression.
- **Any settings you saved before this release were never written to disk.**
  The Settings tab did not persist anything, so it comes up with defaults one
  last time. From now on it saves as you edit.
- **Lap records saved before this release may be missing.** Personal bests
  were compared against the world record rather than your own history, so
  `records.json` only ever gained an entry from someone who had beaten it.

### 🚀 New Features

- **Assetto Corsa is found on Linux.** The install root was probed as four
  hardcoded Windows drive letters, so `content/cars` was never located and
  every car-spec lookup returned nothing. Setups were looked for in
  `~/Documents`, but under Proton the game is a Windows process writing inside
  its own prefix. The new `ac_paths` module walks the real Steam roots
  (`~/.steam/steam`, `~/.local/share/Steam`, Flatpak and Snap homes, Program
  Files on Windows), reads Steam's `libraryfolders.vdf` so a library on any
  drive is found rather than guessed at, and locates the Proton prefix by app
  id. `ac_install_path` and `ac_documents_path` in the config override both.
- **The Setup Cloud browser works.** The Setup tab handled only Up, Down and
  B, so pressing B opened a browser onto a permanently empty setup list with
  no way to install anything — while the tab's own hint line, the help overlay
  and the README all documented `D` to download. Arrows navigate, Enter
  reloads a car, `D` installs, PgUp/PgDn scroll the details. Fetching runs off
  the render thread, so the UI no longer freezes on a five-second HTTP call.
- **Fuel strategy no longer waits on AC.** Every fuel figure was gated on
  `gfx.fuel_x_lap`, which reads zero for the whole of lap one and sits in the
  part of the graphics page not yet confirmed against a live capture.
  Consumption measured across completed laps now fills in, so the strategy tab
  works from lap two regardless of that field.
- **Honest connection status.** The footer distinguishes `LIVE`,
  `AC RUNNING - NO DATA` and `AC NOT RUNNING` rather than collapsing three
  tracked states into ONLINE/OFFLINE. Panels with no telemetry say which it is
  instead of drawing nothing.
- **Richer CSV export.** RPM, lateral G, longitudinal G and average slip were
  being dropped even though the trace carries them — the three things an
  external tool is most often opened for. Files are named after the car, track
  and lap instead of colliding on `lap_3_export.csv`, and a failed export now
  reports itself instead of failing silently.
- **Terminal-too-small screen.** Below 80x20 the app shows its current and
  required size instead of drawing into an area that cannot hold the layout.
  The startup resize is now grow-only, so it stops shrinking the window of
  anyone running maximised.
- **Ghost delta.** The `show_ghost_delta` toggle now selects the delta source:
  with it on, the readout compares against your own recorded best lap through
  `calculate_ghost_delta`, which was fully implemented and had no caller.

### 🛡️ Crashes Fixed

- **Narrow terminals.** Four `Rect` fields in the Setup tab subtracted
  constants from a `u16` width and height. Below 20 columns they wrapped to
  around 65530 and indexed out of the render buffer.
- **Mid-download panic.** The updater's progress bar built its trailing
  segment with `"░".repeat(20 - filled)` on an unclamped percentage, so a
  response body longer than its Content-Length aborted the app while the user
  watched it update.
- **NaN from stale shared memory.** `Gauge::ratio` asserts its input is within
  0.0..=1.0 and `clamp` returns NaN unchanged, so a single garbage float from
  a zeroed `/dev/shm` page took the app down. All nine gauge call sites reject
  non-finite input first.
- **100% CPU from a config file.** `AppConfig::validate` had no caller outside
  its own unit test, so `update_rate: 0` reached `event::poll` and
  `thread::sleep` and spun two cores. Validation now runs on load, and covers
  the pressure targets, alert bands, temperature limits and shift point that
  previously had no bounds at all.

### 🛡️ Things That Silently Did Nothing

- **Version carousel arrows.** `check_for_updates` dropped every release older
  than the running one, so on the newest build the list held a single entry
  and Left/Right had nowhere to move — while the launcher rendered a "you
  won't be able to switch back" warning for versions that could never appear.
- **Update checks after being offline.** The check ran once at startup, so a
  machine behind a captive portal kept an empty carousel for the whole session
  with no way to retry. Selecting the UPDATE item now re-checks, debounced to
  once a minute.
- **Saving settings.** `handle_input` mutated the config and nothing wrote it
  back; `apply_config` had no callers, so changes did not take effect until a
  restart. The `auto_save` and `show_ghost_delta` toggles were read by nothing.
- **Personal bests.** Compared against the world record, and the whole block
  was nested inside a car-specs lookup that always failed on Linux — so no
  record was created, compared or saved there at all, which also left
  `world_record` as None and disabled the off-pace advice.
- **Setup auto-detection.** `match_score` can only produce 0/20/25/30/45/50/
  55/75 and the threshold was `> 60`, so only a perfect three-way match ever
  qualified. One lap of burnt fuel dropped it to 55 and silently blanked the
  "(NOW: x%)" hints in the brake-bias and camber advice.
- **Suspension roll-asymmetry warning.** It compared `avg_ride_height[0]`
  against itself, so the difference was always exactly zero. AC publishes ride
  height per axle, not per corner, so the check cannot be written against this
  data and has been removed rather than left looking functional.
- **Simulator detection on Linux.** `is_process_running` matched only
  `simulator.exe`, but the Linux build is called `simulator`, so the launcher
  waited forever on the platform the bridge exists for.

### 🛡️ Wrong Numbers

- **Driving-style aggression** combined the lateral and *vertical* G axes, so
  a stationary car scored 40% and braking or acceleration was invisible to it.
- **Out-laps scored perfect tyre management.** With no sample above the speed
  gate, pressure deviation computed to 0.0 and the score to a perfect 100 — an
  out-lap rated better than a hot lap, and the advice recommended inflating by
  27.5 psi against a 0.0 psi reading.
- **Mistake counts scaled with Update Rate.** Oversteer, understeer, lockup
  and scrubbing counters were divided by a fixed sample count, so changing the
  rate in Settings halved every score and made laps recorded at different
  rates incomparable.
- **The final sector split raced the lap counter** and could land in the
  following lap. It is derived from the lap time now. `AcStatic::sector_count`
  is honoured too, so 2- and 4-sector mod tracks produce a theoretical best.
- **Fuel targets under-fuelled.** A timed race ends when the leader
  *completes* the lap the clock ran out on, and the lap already in progress
  still has to be finished; the target accounted for neither.
- **Stale fuel warnings.** `fuel_laps_remaining` was never cleared, so
  BOX BOX BOX could fire after a refuel on a value measured before the stop.
- **Torn shared-memory reads.** The physics page is rewritten at 333 Hz while
  ~600 bytes are copied out of it. Pages are re-read when AC's `packet_id`
  moves mid-copy, so a frame spliced from two game ticks no longer reaches the
  jerk accumulators and peak-G tracking as a phantom lockup.
- **Track-map bounds** were serialised as `f32::MAX`/`f32::MIN` sentinels when
  a lap had no usable coordinates, so anything computing `max - min` from a
  saved lap got -6.8e38.
- **Units were ignored.** Target pressures printed a hardcoded "PSI" and
  ambient temperatures a hardcoded "C" whatever the Display settings said;
  alert thresholds printed no unit at all. Tyre temperature *spreads* were
  converted as absolute temperatures, adding a 32°F offset that does not
  belong to a difference. Min Speed was folded from a seed of 999.0, so an
  empty trace displayed "999.0 km/h" as if it were a measurement.

### 🛡️ Keys, Text and Alerts

- The first-run prompt could not be exited with Ctrl+C, q or Esc — the first
  screen every new user sees, and Enter was the only way out.
- F1 did not close the help modal that says "PRESS ESC, ?, Q, OR F1 TO CLOSE"
  in nine places.
- Esc in the analysis load menu quit the whole session back to the launcher,
  while the menu's own footer promised "ESC: Close".
- Held keys were dropped on Windows, which reports them as `Repeat` rather
  than `Press`.
- `S` in the analysis tab saved the fastest lap rather than the selected one.
- Tabs were documented as F1–F9 in nine screen titles, the navigation summary
  and the README; they are 1–9. The footer advertised "[H: Help]" for a key
  that is not handled, and F10 was described as a compact UI mode when it
  toggles the game overlay. Keys documented nowhere — Tab/Shift+Tab, F11,
  Ctrl+L, E, PgUp/PgDn, the A/S/D category switches — are now listed.
- Brake and tyre-temperature alerts pushed a fresh recommendation on every
  frame the condition held — roughly sixty a second per corner, burying every
  other message. They now use the same hysteresis as every comparable alert.
- Status messages never cleared, so "Exported CSV: ..." stayed pinned to the
  footer for the session and a stale message looked like a fresh one.
- Twelve locale keys existed only in Russian; a test now enforces parity. A
  malformed locale override produced an empty dictionary in silence,
  degrading the whole UI to raw key names.

### 🛡️ Data, Shutdown and Security

- **Durability.** The records file, config and CSV export renamed a temp file
  into place without flushing it first, so a power loss could publish a
  correctly-named empty file. Two instances saving at once also shared a temp
  path, which is the one way that pattern corrupts rather than merely loses.
- **Records validation.** A zero or negative lap time was accepted, written to
  disk, then dropped by the read path on next load — which reads to the driver
  as a personal best vanishing between sessions.
- **Crash reports and logs** were written relative to the working directory,
  unwritable when launched from a shortcut or installed under Program Files.
  The crash report was then dropped in silence. A logging failure also aborted
  startup before the TUI was drawn.
- **Stale `/dev/shm` mappings.** shm-bridge's cleanup returned on the first
  failure, leaving the remaining pages behind zero-filled — and the app maps
  those without complaint, reporting a healthy connection to a dead feed.
- **Quitting could hang forever** waiting on a bridge that never acknowledged
  the exit request. Bounded to five seconds, and errors inside that task are
  no longer discarded.
- **A missing `protontricks-launch` was fatal**, so anyone running AC natively,
  through another launcher, or simply reviewing saved laps offline could not
  start the app at all.
- **INI injection.** A newline in a downloaded setup's notes field opened a new
  line in the file AC parses as a car setup, letting a `[SECTION]` be smuggled
  past everything the downloader validates.

### ⚡ Performance

- `is_process_running` reads every process on the system and was called twice
  per frame from the launcher — roughly 124 full process-table scans a second
  while sitting in a menu. Cached for one second.
- Loading a car's cloud setups no longer blocks the render thread.

### 🧹 Internal

- **Shared-memory layout tests** parse graphics, physics and static pages
  captured verbatim from a live AC 1.16.4 session through the same zerocopy
  call the app uses. Previously every test built an `Ac*` value in Rust and
  read it back, so none could detect a mismatch with the game.
- **The test suite now compiles under the workspace edition and lints.** It
  was pinned to edition 2021 against the workspace's 2024 and omitted
  `[lints] workspace = true`, so `unwrap_used` and `panic` were silently
  unenforced across it. Two modules that asserted nothing about this project
  were removed — one never imported the crate under test, the other spawned
  `sh` and checked its exit status.
- **CI builds with `--locked`** and runs on `actions/checkout@v6`, matching the
  release workflow.
- **Version numbers come from the manifest.** The release scripts and the
  generated screenshots hardcoded `v0.2.3`, two releases behind.
- **Screenshots regenerated**, including `Help_Modal.svg`, which was
  byte-identical to `Analysis_Radar.svg` because the tester set a field the
  renderer does not read.
- **The commit convention is written down** in AGENTS.md.

## [v0.3.0] - 2026-08-02

### 🚀 New Features & Enhancements
- **Automated Release Pipeline**: Added `cargo-dist` configuration and a GitHub Actions workflow that builds and publishes Linux and Windows binaries, with shell and PowerShell installers.
- **Continuous Integration**: Added a CI workflow running `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` on Linux and Windows.

### 🛡️ Bug Fixes & Stability
- **In-App Updater Platform Selection**: The updater looked for a `-linux` asset suffix that no release has ever published, so on Linux no update was ever offered. Asset selection is now based on the running OS, rejects artifacts that are not the application (`shm-bridge`, installers, checksums), and refuses to install a build for a foreign platform.
- **In-App Updater Archive Support**: The updater now unpacks the application binary out of release archives (`.tar.gz` on Linux, `.zip` on Windows) instead of only handling bare binaries.

---

## [v0.2.3] - 2026-07-30

### 🚀 New Features & Enhancements
- **Live Micro-Sector & Predictive Lap Time Engine**: Real-time sector split analytics (S1, S2, S3) and predictive delta estimation built into `ac_core::analyzer` and UI tabs.
- **Crash Diagnostic Logging & Panic Hook**: Added custom `std::panic::set_hook` diagnostic logger that captures unhandled exceptions and exports detailed crash trace dumps (`crash_report_<timestamp>.log`) to the logs directory.
- **External JSON Localization System**: Moved all UI translations to external `data/locales/en.json` and `data/locales/ru.json` files with embedded compile-time fallbacks.
- **Linux Bash Build Script (`build_release.sh`)**: Added executable Linux release packaging script for native Linux TUI binaries and Wine/Proton `shm-bridge.exe`.
- **Pixel-Perfect PNG Text Glyph Renderer**: Enhanced `tui_tester` tool with bitmap text glyph rendering for readable English PNG screenshots.

### 🛡️ Bug Fixes & Stability
- **Safe Lock Protection**: Converted `SafeLock` mutex primitives to handle poisoned locks without crashing or panicking.
- **Cross-Platform Compatibility**: Gated Win32 file mapping APIs cleanly under Linux target stubs so `cargo check --workspace` passes without errors on all platforms.
- **Clippy Cleanliness**: Resolved all Clippy warnings and enforced strict workspace linting rules (`unwrap_used = "deny"`, `panic = "deny"`).

---

## [v0.2.2] - 2026-07-30

### 🌟 Features
- Added ratatui TUI dashboard, telemetry analyzer, setup manager, and overlay manager.
- Added cross-platform shared memory reader for Assetto Corsa.
