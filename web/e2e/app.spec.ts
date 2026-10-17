import { test, expect } from '@playwright/test'
test('首次显示中文 PAT 登录入口，不伪装尚未开放的授权方式', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: '连接 PikPak 账号' })).toBeVisible()
  await expect(page.getByLabel('PikPak 个人访问令牌')).toBeVisible()
  await expect(
    page.getByRole('button', { name: '使用 PikPak 浏览器授权（尚未开放）' }),
  ).toBeDisabled()
  await expect(page.getByRole('button', { name: '退出登录' })).toHaveCount(0)
  await page.screenshot({ path: 'test-results/login-light.png' })
})
test('主题立即生效、重载保留、滑块不透明化文字、跟随系统响应变化', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '外观设置' }).click()
  await expect(page.getByRole('heading', { name: '个性化', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '深色', exact: true }).click()
  await expect(page.locator('.app')).toHaveAttribute('data-theme', 'dark')
  await expect(page.locator('.save-state')).toContainText('已保存')
  await page.reload()
  await page.getByRole('button', { name: '外观设置' }).click()
  await expect(page.locator('.app')).toHaveAttribute('data-theme', 'dark')
  await page.locator('#imageOpacity').fill('15')
  await expect(page.locator('output').first()).toHaveText('15%')
  expect(await page.locator('.app').evaluate((element) => getComputedStyle(element).opacity)).toBe(
    '1',
  )
  await page.screenshot({ path: 'test-results/personalization-dark.png' })
  await page.getByRole('button', { name: '跟随系统', exact: true }).click()
  await page.emulateMedia({ colorScheme: 'light' })
  await expect(page.locator('.app')).toHaveAttribute('data-theme', 'light')
  await page.emulateMedia({ colorScheme: 'dark' })
  await expect(page.locator('.app')).toHaveAttribute('data-theme', 'dark')
  await page.emulateMedia({ reducedMotion: 'reduce' })
  expect(await page.locator('.primary').count()).toBe(0)
})
test('窄窗口设置可滚动，界面不横向溢出', async ({ page }) => {
  await page.setViewportSize({ width: 840, height: 600 })
  await page.goto('/')
  await page.getByRole('button', { name: '外观设置' }).click()
  await page.getByRole('button', { name: '恢复默认外观' }).scrollIntoViewIfNeeded()
  await expect(page.getByRole('button', { name: '恢复默认外观' })).toBeVisible()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
})
test('最低面板不透明度下，两种主题文字在极端背景仍满足对比度', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '外观设置' }).click()
  for (const theme of ['浅色', '深色']) {
    await page.getByRole('button', { name: theme, exact: true }).click()
    const contrasts = await page.locator('.app').evaluate((element) => {
      const style = getComputedStyle(element)
      const surface = style.getPropertyValue('--surface-rgb').split(',').map(Number)
      const luminance = (rgb: number[]) =>
        rgb
          .map((value) => {
            const n = value / 255
            return n <= 0.04045 ? n / 12.92 : ((n + 0.055) / 1.055) ** 2.4
          })
          .reduce((sum, n, i) => sum + n * [0.2126, 0.7152, 0.0722][i]!, 0)
      return ['--text', '--muted-text', '--accent-text'].flatMap((token) => {
        const hex = style.getPropertyValue(token).trim().slice(1)
        const rgb = [0, 2, 4].map((index) => parseInt(hex.slice(index, index + 2), 16))
        return [0, 255].map((background) => {
          const panel = surface.map((channel) => channel * 0.8 + background * 0.2)
          const a = luminance(rgb),
            b = luminance(panel)
          return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
        })
      })
    })
    for (const contrast of contrasts) expect(contrast).toBeGreaterThanOrEqual(4.5)
  }
})
test('十万条文件在 Worker 排序，主线程继续响应，支持筛选与降序', async ({ page }) => {
  await page.goto('/')
  const result = await page.evaluate(async () => {
    const worker = new Worker('/src/fileIndex.worker.ts', { type: 'module' })
    let ticks = 0
    const timer = setInterval(() => ticks++, 5)
    const rows = Array.from({ length: 100_000 }, (_, i) => ({
      name: `测试文件 ${String(i).padStart(6, '0')}`,
      folder: i % 2 === 0,
    }))
    const run = (job: unknown) =>
      new Promise<{ indices: Uint32Array; elapsed: number }>((resolve) => {
        worker.onmessage = (event) => resolve(event.data)
        worker.postMessage(job)
      })
    try {
      const first = await run({ version: 1, query: '099', descending: false, rows })
      const second = await run({ version: 2, query: '099', descending: true })
      return {
        ticks,
        count: first.indices.length,
        first: first.indices[0],
        last: second.indices[0],
        elapsed: first.elapsed,
      }
    } finally {
      clearInterval(timer)
      worker.terminate()
    }
  })
  expect(result.count).toBeGreaterThan(1000)
  expect(result.first! % 2).toBe(0)
  expect(result.last! % 2).toBe(0)
  expect(result.ticks).toBeGreaterThan(0)
  console.log('100k worker index', result)
})
