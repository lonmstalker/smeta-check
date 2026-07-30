# Вход через VK ID и Яндекс ID

Провайдер подключается только конфигурацией (env) — без изменения кода.
Схема стандартная (OAuth2 authorization code): `/api/auth/oauth/{имя}/start`
уводит к провайдеру, callback создаёт/находит пользователя и ставит
refresh-cookie.

Общий шаблон переменных (`<P>` — имя провайдера заглавными):

```
OAUTH_<P>_CLIENT_ID=...
OAUTH_<P>_CLIENT_SECRET=...
OAUTH_<P>_AUTH_URL=...        # страница согласия
OAUTH_<P>_TOKEN_URL=...       # обмен кода на токен
OAUTH_<P>_USERINFO_URL=...    # профиль пользователя
OAUTH_<P>_SCOPE=...
OAUTH_<P>_ID_POINTER=...      # JSON-указатель на id в ответе профиля
OAUTH_<P>_EMAIL_POINTER=...   # (необязательно) указатель на email
OAUTH_<P>_TRUST_EMAIL=true    # провайдер сам проверяет адрес почты
```

`TRUST_EMAIL` решает, можно ли по адресу из профиля войти в СУЩЕСТВУЮЩИЙ
аккаунт с тем же адресом. Ставьте `true` только тем провайдерам, которые
адрес проверяют (VK ID и Яндекс ID — проверяют). Без флага заводится
отдельный аккаунт с синтетическим адресом вида
`<провайдер>.<id>@oauth.local`: иначе провайдер, отдающий непроверенный
email, пускал бы кого угодно в чужую учётную запись.

Второй фактор провайдером не отменяется: если у пользователя включена 2FA,
callback возвращает его на `/login` с pending-токеном во фрагменте адреса,
и вход завершается вводом кода — как при обычном входе.

## Яндекс ID

Создать приложение: https://oauth.yandex.ru (Redirect URI:
`https://<домен>/api/auth/oauth/yandex/callback`).

```
OAUTH_YANDEX_CLIENT_ID=<из кабинета>
OAUTH_YANDEX_CLIENT_SECRET=<из кабинета>
OAUTH_YANDEX_AUTH_URL=https://oauth.yandex.ru/authorize
OAUTH_YANDEX_TOKEN_URL=https://oauth.yandex.ru/token
OAUTH_YANDEX_USERINFO_URL=https://login.yandex.ru/info?format=json
OAUTH_YANDEX_SCOPE=login:email login:info
OAUTH_YANDEX_ID_POINTER=/id
OAUTH_YANDEX_EMAIL_POINTER=/default_email
OAUTH_YANDEX_TRUST_EMAIL=true
```

## VK ID

Создать приложение: https://id.vk.com/about/business (Redirect URI:
`https://<домен>/api/auth/oauth/vk/callback`).

```
OAUTH_VK_CLIENT_ID=<app id>
OAUTH_VK_CLIENT_SECRET=<защищённый ключ>
OAUTH_VK_AUTH_URL=https://id.vk.com/authorize
OAUTH_VK_TOKEN_URL=https://id.vk.com/oauth2/auth
OAUTH_VK_USERINFO_URL=https://id.vk.com/oauth2/user_info
OAUTH_VK_SCOPE=email
OAUTH_VK_ID_POINTER=/user/user_id
OAUTH_VK_EMAIL_POINTER=/user/email
OAUTH_VK_TRUST_EMAIL=true
```

Примечание: VK ID может требовать PKCE для публичных клиентов — наш поток
серверный (confidential), с client_secret. Если провайдер вернёт ошибку про
code_challenge, включите в кабинете режим confidential client. Сверяйте
URL с актуальной документацией провайдера при подключении.

Кнопка на фронте — обычная ссылка на `/api/auth/oauth/yandex/start`
(или `vk`). После callback пользователь возвращается на `/`, фронт делает
refresh и получает сессию.
