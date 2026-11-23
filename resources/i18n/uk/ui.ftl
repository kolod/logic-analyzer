### status: draft
### Чернетка, перекладена машинним способом; потребує перевірки носієм української мови.

### Застосунок

app-title = Logic Analyzer

### Стан і результати захоплення

status-idle = Очікування
status-capturing = Триває захоплення
status-saving = Збереження
status-loading = Завантаження
status-cropping = Видалення даних
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
menu-capture-restart-device = Перезапустити пристрій
menu-capture-bootloader = Перезапуск у завантажувач

### Панель інструментів, вигляд і діалоги

toolbar-device = Пристрій
toolbar-rate = Частота дискретизації (Гц)
toolbar-rate-choice = Частота дискретизації
toolbar-seed = Зерно
toolbar-seed-random = випадкове
toolbar-profile = Сигнал
profile-sparse = Рідкісний
profile-normal = Звичайний
profile-dense = Щільний
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
channel-reset = Скинути

### Повідомлення

notice-dismiss = Закрити
notice-dismiss-all = Закрити всі
notice-log = Показати розташування журналу
notice-list = Повідомлення
menu-help = Довідка
menu-help-log = Показати розташування журналу

### Позначки та вимірювання

menu-markers = Позначки
menu-markers-place-single = Поставити одиночну позначку
menu-markers-place-pair = Поставити парну позначку
menu-markers-cancel = Скасувати розміщення
menu-markers-previous = Попередня позначка
menu-markers-next = Наступна позначка
menu-markers-remove = Видалити позначку
menu-markers-remove-all = Видалити всі позначки
menu-markers-snap = Прилипати до переходів
panel-markers = Позначки
marker-single-add = Одиночна позначка
marker-pair-add = Парна позначка
marker-empty = Позначок немає. Додайте їх кнопками вгорі.
marker-placing-single = Клацніть, щоб поставити позначку. Esc скасовує.
marker-placing-pair-first = Клацніть, щоб поставити перший кінець пари. Esc скасовує.
marker-placing-pair-second = Клацніть, щоб поставити другий кінець. Esc скасовує.
marker-zoomed-in = Наближено, щоб знайти перехід. Вигляд повернеться після кліку або Esc.
marker-limit = Досягнуто ліміту в 64 позначки. Видаліть одну, щоб додати іншу.
marker-delete = Видалити
marker-comment = Коментар
marker-make-pair = Утворити пару
marker-join-hint = Клацніть другий одиночний маркер. Esc скасовує.
marker-action-origin = Рахувати час звідси
marker-action-origin-reset = Час рахується від початку
marker-action-cut-left = Видалити дані ліворуч
marker-action-cut-right = Видалити дані праворуч
marker-action-scroll = Прокрутити до неї
marker-action-trim = Обрізати до пари
marker-action-fit = Підігнати вигляд під пару
marker-action-split = Перетворити на дві одиночні позначки
marker-cut-disabled-running = Неможливо під час захоплення
marker-cut-disabled-nothing = З цього боку нічого видаляти
marker-cut-pair-removed = Пари, у яких лишився лише один кінець, видалено: { $text }.
dialog-crop-title = Видалити дані?
dialog-crop-left-body = Дані ліворуч від позначки буде видалено: { $lost } ({ $percent } %). Залишиться { $kept }. Це неможливо скасувати.
dialog-crop-right-body = Дані праворуч від позначки буде видалено: { $lost } ({ $percent } %). Залишиться { $kept }. Це неможливо скасувати.
dialog-crop-pair-body = Буде збережено лише дані між кінцями пари: { $kept }. Буде видалено { $lost } ({ $percent } %). Це неможливо скасувати.
dialog-crop-confirm = Видалити
dialog-cropping = Видалення даних
marker-colour-0 = Червоний
marker-colour-1 = Бурштиновий
marker-colour-2 = Зелений
marker-colour-3 = Бірюзовий
marker-colour-4 = Синій
marker-colour-5 = Фіолетовий
marker-colour-6 = Рожевий
marker-colour-7 = Сірий
panel-capture = Захоплення
panel-view-capture = Налаштування захоплення
panel-view-decoders = Декодери
panel-view-markers = Маркери та вимірювання
capture-length-seconds = Тривалість (секунди)
capture-length-samples = Відліків: { $count }
capture-length-invalid = Введіть тривалість у секундах, наприклад 2,5.
capture-length-decimals = Не більше дев'яти цифр після коми.
capture-length-small = Це менше за один відлік за такої частоти дискретизації.
capture-length-large = Це довше, ніж може бути захоплення.
capture-length-no-rate = Спершу встановіть частоту дискретизації.
# Тригер захоплення (показується для пристрою, що його має).
trigger-type = Тригер
trigger-none = Без тригера
trigger-level-low = Рівень: низький
trigger-level-high = Рівень: високий
trigger-rising = Зростаючий фронт
trigger-falling = Спадний фронт
trigger-any = Будь-який фронт
trigger-pattern = Шаблон
trigger-fast-pattern = Швидкий шаблон
trigger-channel = Канал тригера (перший канал шаблону)
trigger-pattern-levels = Рівні шаблону, спершу перший канал (0 або 1)
capture-pre-trigger = Відліків до тригера
capture-after-trigger = Відліків після тригера: { $count }
capture-bursts = Пакети (0 — одне захоплення)
capture-channels = Канали для запису
capture-channels-all = Всі
capture-channels-none = Жоден
capture-no-channel = Жодного каналу не вибрано: виберіть хоча б один, щоб почати.

### Файли сесій

menu-file-open = Відкрити…
menu-file-save = Зберегти
menu-file-save-as = Зберегти як…
menu-file-recent = Нещодавні файли
menu-file-recent-empty = Немає нещодавніх файлів
menu-file-recent-missing = (немає файлу)
menu-file-recent-remove-missing = Прибрати недоступні записи
menu-file-recent-clear = Очистити список
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
settings-unsaved-title = Незбережений запис
settings-remember-choice = Запам'ятати вибір
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

### Decoders: the sample decoder
decoder-sample-clocked-bits-name = Тактовані біти
decoder-sample-clocked-bits-description = Читає біти з лінії даних за фронтами тактової лінії та збирає їх у байти. Зразковий декодер, що показує можливості декодерів.
decoder-sample-clocked-bits-signal-clock = Такт
decoder-sample-clocked-bits-signal-data = Дані
decoder-sample-clocked-bits-option-clock_edge = Фронт такту
decoder-sample-clocked-bits-choice-clock_edge-rising = Зростаючий
decoder-sample-clocked-bits-choice-clock_edge-falling = Спадний
decoder-sample-clocked-bits-option-bit_order = Порядок бітів
decoder-sample-clocked-bits-choice-bit_order-msb = Старший біт першим
decoder-sample-clocked-bits-choice-bit_order-lsb = Молодший біт першим
decoder-sample-clocked-bits-option-bits = Бітів у байті
decoder-sample-clocked-bits-option-idle = Межа простою (відліків)
ann-sample-clocked-bits-byte = Байт { $value }
ann-sample-clocked-bits-bit = Біт { $value }
ann-sample-clocked-bits-incomplete = Неповний байт: { $value } бітів
ann-sample-clocked-bits-gap = Тут втрачено відліки

### Decoders: I2C
decoder-i2c-name = I2C
decoder-i2c-description = Читає шину I2C (два дроти, такт і дані): адресу, байти та підтвердження кожної транзакції.
decoder-i2c-signal-scl = Такт (SCL)
decoder-i2c-signal-sda = Дані (SDA)
decoder-i2c-option-address = Запис адреси
decoder-i2c-choice-address-addr7 = 7-бітна адреса (без біта напрямку)
decoder-i2c-choice-address-addr8 = 8-бітне значення (з бітом напрямку)
decoder-i2c-option-radix = Система числення
decoder-i2c-choice-radix-hex = Шістнадцяткова
decoder-i2c-choice-radix-dec = Десяткова
decoder-i2c-choice-radix-bin = Двійкова
ann-i2c-start = Старт
ann-i2c-repeated-start = Повторний старт
ann-i2c-stop = Стоп
ann-i2c-transaction-write = Запис до { $value }
ann-i2c-transaction-read = Читання з { $value }
ann-i2c-transaction-write-long = Запис до { $value }: { $second }
ann-i2c-transaction-read-long = Читання з { $value }: { $second }
ann-i2c-address-write = Адреса { $value } (запис)
ann-i2c-address-read = Адреса { $value } (читання)
ann-i2c-data = Дані { $value }
ann-i2c-ack = ACK
ann-i2c-nak = NAK
ann-i2c-incomplete = Незавершена транзакція
ann-i2c-error-byte-cut = Байт обірвано після { $value } бітів
ann-i2c-error-address-nak = Адресу { $value } не підтверджено
ann-i2c-error-data-nak = Дані { $value } не підтверджено
ann-i2c-error-empty = Порожня транзакція: немає адреси
ann-i2c-error-gap = Тут втрачено відліки

### Decoders: menu, panel and the selected annotation
menu-decoders = Декодери
menu-decoders-add = Додати декодер
menu-decoders-previous = Попередня анотація
menu-decoders-next = Наступна анотація
menu-decoders-goto = Перейти до вибраної анотації
panel-decoders = Декодери
decoders-add = Додати декодер
decoders-empty = Декодерів ще немає. Додайте один, щоб побачити, що кажуть сигнали.
decoders-need-capture = Спершу захопіть або відкрийте сигнали.
decoder-state-waiting = Очікує
decoder-state-running = Працює
decoder-state-finished = Завершено
decoder-state-cancelled = Скасовано
decoder-state-failed = Збій
decoder-needs-channels = Оберіть канали, щоб почати декодування.
decoder-channel-none = (немає)
decoder-channel-unavailable = { $name } (не записується)
decoder-channels-unrecorded = Канал цього декодера не записується, тому декодер не запускається.
decoder-annotations = Анотацій: { $number }
decoder-incomplete = Неповні результати

### Decoders: the simulation of signals that are valid for a decoder
decoder-generator-errors = Навмисно додавати помилки в змодельовані сигнали
decoder-generator-rate = Приблизно кожен такий за рахунком байт обривається

### Decoders: running, cancelling and the memory limit
decoder-cancel = Скасувати
settings-decoder-memory = Обмеження пам'яті для результатів декодерів
settings-decoder-memory-note = Спільне для всіх декодерів. Діє з наступного проходу.

### Decoders: stacking
decoder-source = Читає з
decoder-source-capture = Захоплення
dialog-remove-decoder-title = Прибрати декодер
dialog-remove-decoder-body = Інші декодери читають з цього. Якщо його прибрати, їх буде прибрано теж. Захоплення це не зачіпає.
dialog-remove-decoder-confirm = Прибрати всі

### Decoders: saving and opening
notice-decoders-newer = У цьому файлі є декодери з новішої версії застосунку. Їх збережено як є й не показано; захоплення відкривається як зазвичай.
notice-layout-newer = Цей файл містить висоту сигналів, збережену новішою версією застосунку. Її збережено без змін і не показано; захоплення відкривається з автоматичною висотою.
settings-save-decoder-results = Зберігати результати декодування разом із сеансом
settings-save-decoder-results-note = Файл стає більшим. Без цього декодери запускаються знову, коли файл відкривають.
