// Логи фронтенда. info — только в консоль; warn/error дублируются на бэк
// (/api/logs) и попадают в общий поток логов сервера с пометкой frontend.

type Context = Record<string, unknown>

function ship(level: 'warn' | 'error', message: string, context: Context = {}) {
  try {
    // keepalive: лог долетит, даже если вкладка закрывается
    void fetch('/api/logs', {
      method: 'POST',
      keepalive: true,
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ level, message, context }),
    }).catch(() => {})
  } catch {
    // логирование никогда не должно ломать приложение
  }
}

export const log = {
  info: (message: string, context?: Context) => console.info(message, context ?? ''),
  warn: (message: string, context?: Context) => {
    console.warn(message, context ?? '')
    ship('warn', message, context)
  },
  error: (message: string, context?: Context) => {
    console.error(message, context ?? '')
    ship('error', message, context)
  },
}

/** Глобальные обработчики: необработанные ошибки страницы уходят на бэк */
export function installGlobalErrorLogging() {
  window.addEventListener('error', (event) => {
    ship('error', event.message, { source: event.filename, line: event.lineno })
  })
  window.addEventListener('unhandledrejection', (event) => {
    ship('error', `unhandled rejection: ${String(event.reason)}`)
  })
}
