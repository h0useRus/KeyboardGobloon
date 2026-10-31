# Keyboard Gobloon — русский. Every key must exist in every lang/*.ftl.

# Tray icon menu
tray-enabled = Включён
tray-settings = Настройки…
tray-exit = Выход

# Settings window: tabs
tab-general = Общие
tab-hotkeys = Горячие клавиши
tab-languages = Языки
tab-about = О программе

# General tab
general-startup = Запуск
general-autostart = Запускать вместе с Windows
general-typing = Набор текста
general-auto-switch = Исправлять слова, набранные не в той раскладке
general-auto-switch-tip = Когда слово закончено, Keyboard Gobloon перепечатывает его в нужной раскладке и переключается на неё. Pause сразу после этого отменяет исправление. Работает для языков с языковым пакетом (английский, русский).
general-appearance = Оформление
general-language = Язык
general-language-system = Как в Windows
theme-system = Системная
theme-system-tip = Светлая или тёмная, как настроено в Windows
theme-light = Светлая
theme-dark = Тёмная

# Hotkeys tab
hotkeys-intro = Нажмите кнопку записи и затем новые клавиши. Модификатор, нажатый и отпущенный отдельно (например, правый Ctrl), работает как одиночное нажатие.
action-switch-layout = Сменить раскладку
action-convert-last = Исправить последнее слово
action-convert-selection = Исправить выделенный текст
action-change-case = Сменить регистр выделенного текста
action-toggle-enabled = Включить или выключить Keyboard Gobloon
switch-system = Windows: { $keys }
keys-none = Не задано
keys-press = Нажмите клавиши…
keys-record = Записать сочетание
keys-clear = Убрать сочетание
keys-cancel = Отмена
keys-moved = { $keys } было у действия «{ $action }»: теперь оно здесь.
keys-reset = Вернуть сочетания по умолчанию

# About tab
about-text = Исправляет текст, набранный не в той раскладке.
about-alpha = альфа
about-created = Автор: { $name }
about-support = Поддержать проект
about-support-tip = Внести вклад через PayPal
about-website = Сайт
about-source = Исходный код
about-issues = Сообщить об ошибке
about-license = Бесплатно и с открытым кодом, лицензия MIT.

# Languages tab
languages-intro = Языковые пакеты помогают Keyboard Gobloon замечать слова, набранные не в той раскладке. Английский и русский входят в программу, остальные можно скачать здесь. Пакет работает, если в Windows есть раскладка для его языка.
languages-included = Встроен
languages-installed = Версия { $version }
languages-download = Скачать
languages-update = Обновить до новой версии
languages-delete = Удалить
languages-no-layout = В Windows нет раскладки для языка «{ $name }»: добавьте её в параметрах Windows, чтобы пакет работал.
languages-loading = Загрузка списка языков…
languages-offline = Не удалось загрузить список языков: { $error }
languages-retry = Повторить

# Status line
status-saved = Сохранено.
status-not-running = Сохранено. Keyboard Gobloon не запущен: настройки вступят в силу при запуске.
status-autostart-failed = Не удалось изменить запуск вместе с Windows.
status-pack-downloaded = «{ $name }»: скачан.
status-pack-failed = Не удалось скачать «{ $name }»: { $error }
status-pack-deleted = «{ $name }»: удалён.
