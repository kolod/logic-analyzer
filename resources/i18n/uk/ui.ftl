### status: draft
### Чернетка, перекладена машинним способом; потребує перевірки носієм української мови.

### Застосунок

app-title = Logic Analyzer

### Стан і результати захоплення

status-idle = Очікування
status-capturing = Триває захоплення
status-saving = Збереження
status-loading = Завантаження
status-memory = { $used } / { $limit }
end-reason-completed = Захоплення завершено
end-reason-stopped = Зупинено вами
end-reason-memory-limit = Досягнуто обмеження пам'яті
end-reason-disconnected = Пристрій від'єднано
end-reason-device-error = Помилка пристрою
end-reason-samples-lost = Завершено з утраченими відліками
end-reason-loaded = Завантажено з файлу
notice-device-removed = Вибраний пристрій «{ $text }» більше недоступний, тому вибрано типовий пристрій.

### Меню

menu-file = Файл
menu-file-new = Нове захоплення
menu-file-quit = Вийти
menu-capture = Захоплення
menu-capture-start = Почати
menu-capture-stop = Зупинити

### Панель інструментів, вигляд і діалоги

toolbar-device = Пристрій
toolbar-rate = Частота дискретизації (Гц)
toolbar-length = Довжина (відліків)
toolbar-seed = Зерно
toolbar-seed-random = випадкове
toolbar-profile = Сигнал
profile-sparse = Рідкісний
profile-normal = Звичайний
profile-dense = Щільний
toolbar-realtime = Реальний час
toolbar-start = Почати
toolbar-stop = Зупинити
status-simulated = Симульовано
status-incomplete = Неповне
status-unsaved = Незбережені зміни
view-no-capture = Захоплення ще немає. Виберіть пристрій і натисніть «Почати».
view-empty = У захопленні немає відліків.
view-cannot-draw = Сигнали неможливо намалювати.
channel-default = Канал { $number }
dialog-unsaved-title = Незбережене захоплення
dialog-unsaved-body = Захоплення не збережено. Що з ним зробити?
dialog-save = Зберегти
dialog-discard = Відкинути
dialog-cancel = Скасувати

### Вигляд і канали

menu-view = Вигляд
menu-view-zoom-in = Збільшити
menu-view-zoom-out = Зменшити
menu-view-fit = Показати все
menu-view-start = До початку
menu-view-end = До кінця
menu-view-pan-left = Прокрутити ліворуч
menu-view-pan-right = Прокрутити праворуч
menu-view-page-left = Сторінка назад
menu-view-page-right = Сторінка вперед
menu-capture-follow = Стежити за новими даними
panel-channels = Канали
channel-move-up = Вгору
channel-move-down = Вниз
channel-colour = Колір
channel-reset = Скинути канали

### Повідомлення

notice-dismiss = Закрити
notice-dismiss-all = Закрити всі
notice-log = Показати розташування журналу
notice-list = Повідомлення
menu-help = Довідка
menu-help-log = Показати розташування журналу

### Позначки та вимірювання

menu-markers = Позначки
menu-markers-place = Поставити позначку
menu-markers-previous = Попередня позначка
menu-markers-next = Наступна позначка
menu-markers-remove = Видалити позначку
menu-markers-remove-all = Видалити всі позначки
menu-markers-snap = Прилипати до переходів
panel-markers = Позначки
marker-default = Позначка { $number }
marker-go = До позначки
marker-remove = Видалити
marker-measure-first = Перша позначка вимірювання
marker-measure-second = Друга позначка вимірювання
measure-distance = Відстань
measure-frequency = Частота
measure-pick = Виберіть дві позначки, щоб виміряти час між ними.

### Файли сесій

menu-file-open = Відкрити…
menu-file-save = Зберегти
menu-file-save-as = Зберегти як…
menu-file-recent = Нещодавні файли
dialog-saving = Збереження сесії…
dialog-opening = Відкриття сесії…
dialog-open-title = Відкрити сесію
dialog-save-title = Зберегти сесію
dialog-filter-session = Сесії логічного аналізатора
title-untitled = Захоплення без назви
notice-view-newer = Налаштування вигляду в цьому файлі походять із новішої версії й не були застосовані. Вони залишаються у файлі без змін.

### Налаштування та інформація

menu-file-settings = Налаштування…
menu-view-scale-up = Збільшити інтерфейс
menu-view-scale-down = Зменшити інтерфейс
menu-help-about = Про програму
settings-title = Налаштування
settings-language = Мова
settings-theme = Тема
theme-system = Як у системі
theme-light = Світла
theme-dark = Темна
theme-high-contrast = Висока контрастність
settings-scale = Масштаб інтерфейсу
settings-memory = Ліміт пам’яті захоплення
settings-memory-unit = МіБ
settings-memory-note = Застосовується до наступного захоплення.
settings-memory-invalid = Введіть ціле число МіБ від { $min } до { $max }.
dialog-close = Закрити
about-title = Про програму
about-body = Logic Analyzer { $version }. Записує цифрові сигнали та показує їх на часовій шкалі; змодельований пристрій дозволяє спробувати програму без обладнання.
about-license = Вільне програмне забезпечення за ліцензією GNU General Public License, версія 3.
