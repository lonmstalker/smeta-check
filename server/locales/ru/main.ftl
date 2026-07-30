# Тексты ответов API. Ключ = машиночитаемый код ошибки для фронта.
# Числа подставляются с правильными склонениями (см. email-reset-body).

error-internal = Внутренняя ошибка, попробуйте позже
error-unauthorized = Требуется вход в систему
error-forbidden = Недостаточно прав
error-not-found = Не найдено
error-validation-email = Некорректный адрес почты
error-password-short = Пароль должен быть не короче { $min } { $min ->
        [one] символа
       *[other] символов
    }
error-email-taken = Эта почта уже занята
error-invalid-credentials = Неверная почта или пароль
error-invalid-totp = Неверный код подтверждения
error-totp-already-enabled = Двухфакторная защита уже включена
error-totp-not-enabled = Двухфакторная защита не включена
error-invalid-token = Ссылка недействительна или устарела
error-title-empty = Название не может быть пустым
error-too-many-requests = Слишком много попыток, подождите минуту
error-wrong-password = Текущий пароль указан неверно
error-no-password = У аккаунта нет пароля — сначала задайте его через восстановление
error-email-same = Это и есть ваш текущий адрес
error-name-long = Имя не длиннее { $max } символов
error-unknown-locale = Такого языка нет
error-oauth-not-configured = Вход через { $provider } не настроен
error-oauth-failed = Не удалось войти через { $provider }
error-estimate-no-file = Файл не получен — приложите смету и попробуйте ещё раз
error-estimate-format = Пока принимаем только Excel: файлы xlsx и xls
error-estimate-empty = Файл пустой — проверьте, что смета сохранилась
error-estimate-too-large = Файл больше { $max } МБ — пришлите смету без лишних картинок
error-estimate-unreadable = Файл не открылся как таблица Excel — пришлите смету заново
error-estimate-no-data = В файле не нашлось ни одной заполненной строки
error-estimate-too-big = В файле слишком много листов или строк для сметы
error-estimate-limit = Пока можно хранить не больше { $max } { $max ->
        [one] сметы
       *[other] смет
    }

email-verify-subject = Подтвердите адрес почты
email-verify-body = Чтобы подтвердить адрес, откройте ссылку: { $link }
    Ссылка действует { $hours } { $hours ->
        [one] час
        [few] часа
       *[other] часов
    } и сработает один раз.

email-reset-subject = Восстановление пароля
email-reset-body = Чтобы задать новый пароль, откройте ссылку: { $link }
    Ссылка действует { $minutes } { $minutes ->
        [one] минуту
        [few] минуты
       *[other] минут
    } и сработает один раз.

email-change-subject = Подтвердите новый адрес почты
email-change-body = Чтобы новый адрес заработал, откройте ссылку: { $link }
    Ссылка действует { $minutes } { $minutes ->
        [one] минуту
        [few] минуты
       *[other] минут
    } и сработает один раз. Пока вы её не открыли, вход остаётся по старому адресу.

email-change-notice-subject = Запрошена смена адреса почты
email-change-notice-body = В аккаунте запросили смену адреса на { $email }.
    Если это были не вы — смените пароль: смена сработает только после
    подтверждения с нового адреса.
