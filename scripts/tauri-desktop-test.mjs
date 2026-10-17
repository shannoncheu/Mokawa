// Real Windows WebView2, not a browser mock. CDP is enabled only in this test process.
import { chromium, expect } from '@playwright/test'
import { spawn, execFileSync } from 'node:child_process'
import { setTimeout as delay } from 'node:timers/promises'
import { writeFile, readdir } from 'node:fs/promises'
import { resolve } from 'node:path'
const executable = process.env.MOKAWA_TEST_EXECUTABLE
if (!executable) throw Error('Windows test executable missing')
const results = { nativeWebView2: true }
async function launch() {
  const child = spawn(executable, [], {
    stdio: 'inherit',
    env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9222' },
  })
  let browser
  for (let i = 0; i < 60; i++) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9222', { timeout: 1000 })
      break
    } catch {
      await delay(500)
    }
  }
  if (!browser) {
    child.kill()
    throw Error('WebView2 debug connection unavailable')
  }
  const context = browser.contexts()[0]
  let page = context.pages()[0]
  for (let i = 0; i < 40 && !page; i++) {
    await delay(200)
    page = context.pages()[0]
  }
  if (!page) throw Error('No native WebView2 page')
  await page.getByRole('heading', { name: '连接 PikPak 账号' }).waitFor({ timeout: 20000 })
  return { child, browser, page }
}
async function close(app) {
  const exited =
    app.child.exitCode !== null
      ? Promise.resolve()
      : new Promise((resolve) => app.child.once('exit', resolve))
  await app.page
    .getByRole('button', { name: '关闭窗口' })
    .click()
    .catch(() => {})
  await Promise.race([
    exited,
    delay(10000).then(() => {
      throw Error('Native process did not exit')
    }),
  ])
  await app.browser.close().catch(() => {})
  await delay(700)
}
function chooseFile(path) {
  // All paths are committed test fixtures, never user text or account credentials.
  const encoded = Buffer.from(
    `$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.Windows.Forms; $shell=New-Object -ComObject WScript.Shell; if (!$shell.AppActivate('选择本地图片')) { throw 'Native image dialog missing' }; Start-Sleep -Milliseconds 300; [System.Windows.Forms.SendKeys]::SendWait('%n'); [System.Windows.Forms.SendKeys]::SendWait('^a'); [System.Windows.Forms.SendKeys]::SendWait('${path.replaceAll("'", "''")}'); [System.Windows.Forms.SendKeys]::SendWait('{ENTER}');`,
    'utf16le',
  ).toString('base64')
  execFileSync('powershell', ['-NoProfile', '-EncodedCommand', encoded], {
    stdio: 'inherit',
    timeout: 15000,
  })
}
let app
try {
  app = await launch()
  await app.page.screenshot({ path: 'dist/tauri-login.png' })
  results.login = true
  // Synthetic metadata stays inside the isolated Worker; it never impersonates cloud files.
  const workerAsset = (await readdir('web-dist/assets')).find((name) =>
    /^fileIndex\.worker-.*\.js$/.test(name),
  )
  if (!workerAsset) throw Error('Production file Worker missing')
  results.productionWorker = await app.page.evaluate(
    (asset) =>
      new Promise((resolve, reject) => {
        const worker = new Worker(`/assets/${asset}`, { type: 'module' })
        const timeout = setTimeout(() => {
          worker.terminate()
          reject(Error('Production Worker timed out'))
        }, 10000)
        worker.onerror = () => {
          clearTimeout(timeout)
          worker.terminate()
          reject(Error('Production Worker rejected by WebView2 or CSP'))
        }
        worker.onmessage = (event) => {
          clearTimeout(timeout)
          worker.terminate()
          event.data.indices.length === 10000
            ? resolve(true)
            : reject(Error('Invalid Worker result'))
        }
        worker.postMessage({
          version: 1,
          query: '',
          descending: false,
          rows: Array.from({ length: 10000 }, (_, i) => ({ name: `文件${i}`, folder: false })),
        })
      }),
    workerAsset,
  )
  await expect(app.page.getByLabel('PikPak 个人访问令牌')).toBeVisible()
  await expect(
    app.page.getByRole('button', { name: '使用 PikPak 浏览器授权（尚未开放）' }),
  ).toBeDisabled()
  results.personalAccessTokenEntry = true
  results.browserAuthorizationClearlyUnavailable = true
  await app.page.getByRole('button', { name: '外观设置' }).click()
  await app.page.getByRole('heading', { name: '个性化', exact: true }).waitFor()
  await app.page.getByRole('button', { name: '深色', exact: true }).click()
  await expect(app.page.locator('.app')).toHaveAttribute('data-theme', 'dark')
  await expect(app.page.locator('.save-state')).toContainText('已保存')
  results.themeClick = true
  await app.page.getByRole('button', { name: '选择图片', exact: true }).click()
  await delay(1000)
  chooseFile(resolve('tests/fixtures/avatar.png'))
  await expect(app.page.locator('.avatar-editor img')).toBeVisible({ timeout: 20000 })
  expect(await app.page.locator('.avatar-editor img').evaluate((img) => img.naturalWidth)).toBe(256)
  results.nativeAvatarImport = true
  await app.page.locator('.profile').click()
  expect(
    new Set(
      await app.page
        .locator('.avatar img')
        .evaluateAll((images) => images.map((image) => image.src)),
    ).size,
  ).toBe(1)
  await app.page.locator('.settings-heading').click()
  results.avatarConsistent = true
  await app.page.getByRole('button', { name: '选择背景图片', exact: true }).click()
  await delay(1000)
  chooseFile(resolve('tests/fixtures/background.jpg'))
  await expect(app.page.locator('.background-picker')).toHaveClass(/has/, { timeout: 20000 })
  results.nativeBackgroundImport = true
  await app.page.locator('#imageOpacity').fill('38')
  await app.page.locator('#panelOpacity').fill('80')
  await app.page.locator('#blur').selectOption('14')
  await expect(app.page.locator('.save-state')).toContainText('已保存')
  results.slidersSave = true
  await app.page.screenshot({ path: 'dist/tauri-background-controls.png' })
  await app.page.locator('.settings-page').evaluate((element) => element.scrollTo(0, 0))
  await app.page.screenshot({ path: 'dist/tauri-personalization-dark.png' })
  await close(app)
  app = await launch()
  await app.page.getByRole('button', { name: '外观设置' }).click()
  await expect(app.page.locator('.app')).toHaveAttribute('data-theme', 'dark')
  await expect(app.page.locator('.avatar-editor img')).toBeVisible()
  await expect(app.page.locator('.background-picker')).toHaveClass(/has/)
  await expect(app.page.locator('#imageOpacity')).toHaveValue('38')
  await expect(app.page.locator('#panelOpacity')).toHaveValue('80')
  results.restartPersistence = true
  await app.page.getByRole('button', { name: '最大化', exact: true }).click()
  await expect(app.page.getByRole('button', { name: '还原窗口', exact: true })).toBeVisible()
  await app.page.getByRole('button', { name: '还原窗口', exact: true }).click()
  await expect(app.page.getByRole('button', { name: '最大化', exact: true })).toBeVisible()
  results.nativeMaximizeRestore = true
  await app.page.getByRole('button', { name: '恢复默认头像', exact: true }).click()
  await expect(app.page.locator('.avatar-editor img')).toHaveCount(0)
  await app.page.getByRole('button', { name: '移除背景', exact: true }).click()
  await expect(app.page.locator('.background-picker')).not.toHaveClass(/has/)
  results.removeImages = true
  await app.page.getByRole('button', { name: '浅色', exact: true }).click()
  await app.page.locator('.settings-page').evaluate((element) => element.scrollTo(0, 0))
  await expect(app.page.locator('.app')).toHaveAttribute('data-theme', 'light')
  await app.page.screenshot({ path: 'dist/tauri-personalization-light.png' })
  await close(app)
  app = undefined
  await writeFile('dist/tauri-native-results.json', JSON.stringify(results, null, 2))
} catch (error) {
  await app?.page.screenshot({ path: 'dist/tauri-error.png' }).catch(() => {})
  await writeFile(
    'dist/tauri-native-results.json',
    JSON.stringify({ ...results, error: String(error) }, null, 2),
  )
  throw error
} finally {
  if (app) {
    await app.browser.close().catch(() => {})
    if (app.child.exitCode === null)
      try {
        execFileSync('taskkill', ['/PID', String(app.child.pid), '/T', '/F'])
      } catch {}
  }
}
