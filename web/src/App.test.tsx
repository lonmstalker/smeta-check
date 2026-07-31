import { fireEvent, screen } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import App from './App'
import { setAccessToken } from './api/client'
import { authedApi, guestApi, renderApp } from './test-utils'

afterEach(() => {
  vi.unstubAllGlobals()
  setAccessToken(null)
})

const ESTIMATE = {
  id: '00000000-0000-0000-0000-0000000000e1',
  file_name: 'Смета бригады.xlsx',
  size_bytes: 15403,
  status: 'parsed',
  from_photo: false,
  created_at: '2026-07-30T10:00:00Z',
}

const PHOTO = {
  ...ESTIMATE,
  id: '00000000-0000-0000-0000-0000000000e2',
  file_name: 'Смета.jpg',
  from_photo: true,
}

const DETAILS = {
  ...ESTIMATE,
  lines: [
    {
      position: 0,
      sheet: 'Смета',
      raw_text: 'Штукатурка стен | кв.м. | 12 | 740 | 8880',
      title: 'Штукатурка стен',
      unit: 'кв.м.',
      quantity: 12,
      price: 740,
      total: 8880,
    },
    {
      position: 1,
      sheet: 'Смета',
      raw_text: 'Что-то непонятное из середины файла',
      title: null,
      unit: null,
      quantity: null,
      price: null,
      total: null,
    },
  ],
}

test('гость не видит чужих смет, а видит приглашение войти', async () => {
  guestApi({ '/api/estimates': () => [ESTIMATE] })
  renderApp(<App />)
  expect(await screen.findByText('Чтобы загрузить смету, войдите')).toBeInTheDocument()
  expect(screen.queryByText('Смета бригады.xlsx')).not.toBeInTheDocument()
})

test('вошедший видит свои сметы и их состояние', async () => {
  authedApi({
    '/api/estimates': () => [
      ESTIMATE,
      { ...ESTIMATE, id: 'e2', file_name: 'Вторая.xlsx', status: 'parsing' },
    ],
  })
  renderApp(<App />)
  expect(await screen.findByText('Смета бригады.xlsx')).toBeInTheDocument()
  expect(screen.getByText('Разобрана')).toBeInTheDocument()
  expect(screen.getByText('Разбираем')).toBeInTheDocument()
})

test('смета, которую не смогли прочитать, объясняет причину', async () => {
  authedApi({
    '/api/estimates': () => [
      { ...ESTIMATE, status: 'failed', error: 'Файл не открылся как таблица Excel' },
    ],
  })
  renderApp(<App />)
  expect(await screen.findByText('Файл не открылся как таблица Excel')).toBeInTheDocument()
  // у неразобранной сметы нечего показывать
  expect(screen.queryByRole('button', { name: 'Показать строки' })).not.toBeInTheDocument()
})

test('строки сметы открываются по кнопке, непонятое показано отдельно', async () => {
  authedApi({
    '/api/estimates': () => [ESTIMATE],
    [`/api/estimates/${ESTIMATE.id}`]: () => DETAILS,
  })
  renderApp(<App />)
  fireEvent.click(await screen.findByRole('button', { name: 'Показать строки' }))

  expect(await screen.findByText('Штукатурка стен')).toBeInTheDocument()
  // 1 — единственное число: «работа», не «работ»
  expect(screen.getByText('Распознана 1 работа')).toBeInTheDocument()
  expect(screen.getByText('Спросите бригаду, что это за строки')).toBeInTheDocument()
  expect(screen.getByText('Что-то непонятное из середины файла')).toBeInTheDocument()
})

test('у сметы с фотографии есть предупреждение про нейросеть, у Excel — нет', async () => {
  authedApi({
    '/api/estimates': () => [PHOTO],
    [`/api/estimates/${PHOTO.id}`]: () => ({ ...PHOTO, lines: DETAILS.lines }),
  })
  renderApp(<App />)
  expect(await screen.findByText('С фотографии')).toBeInTheDocument()
  fireEvent.click(await screen.findByRole('button', { name: 'Показать строки' }))
  expect(
    await screen.findByText(
      'Строки распознала нейросеть с фотографии — сверьте их с оригиналом сметы',
    ),
  ).toBeInTheDocument()
})

test('у сметы из Excel предупреждения про нейросеть нет', async () => {
  authedApi({
    '/api/estimates': () => [ESTIMATE],
    [`/api/estimates/${ESTIMATE.id}`]: () => DETAILS,
  })
  renderApp(<App />)
  fireEvent.click(await screen.findByRole('button', { name: 'Показать строки' }))
  expect(await screen.findByText('Штукатурка стен')).toBeInTheDocument()
  expect(screen.queryByText('С фотографии')).not.toBeInTheDocument()
  expect(screen.queryByText(/распознала нейросеть/)).not.toBeInTheDocument()
})

test('фото, где распозналась одна строка из трёх, честно советует прислать файл', async () => {
  const lines = [
    DETAILS.lines[0],
    { ...DETAILS.lines[1], position: 1 },
    { ...DETAILS.lines[1], position: 2 },
    { ...DETAILS.lines[1], position: 3 },
  ]
  authedApi({
    '/api/estimates': () => [PHOTO],
    [`/api/estimates/${PHOTO.id}`]: () => ({ ...PHOTO, lines }),
  })
  renderApp(<App />)
  fireEvent.click(await screen.findByRole('button', { name: 'Показать строки' }))
  expect(await screen.findByText(/Распозналось мало строк/)).toBeInTheDocument()
})

test('неизвестный адрес показывает страницу 404', async () => {
  guestApi()
  renderApp(<App />, { route: '/no-such-page' })
  expect(await screen.findByText('Такой страницы нет')).toBeInTheDocument()
})

test('страница входа открывается по /login', async () => {
  guestApi()
  renderApp(<App />, { route: '/login' })
  expect(await screen.findByRole('button', { name: 'Войти' })).toBeInTheDocument()
  expect(screen.getByPlaceholderText('Почта')).toBeInTheDocument()
  expect(screen.getByPlaceholderText('Пароль')).toBeInTheDocument()
})
