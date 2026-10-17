import { test,expect } from '@playwright/test'
test('首次显示中文登录入口，授权阻塞不进入文件页',async({page})=>{
  await page.goto('/');await expect(page.getByRole('heading',{name:'连接 PikPak 账号'})).toBeVisible()
  await page.getByRole('button',{name:'使用 PikPak 授权登录'}).click()
  await expect(page.locator('.login-error')).toContainText('注册与回调规则仍待确认')
  await expect(page.getByRole('button',{name:'退出登录'})).toHaveCount(0)
  await page.screenshot({path:'test-results/login-light.png'})
})
test('主题立即生效、重载保留、滑块不透明化文字、跟随系统响应变化',async({page})=>{
  await page.goto('/');await page.getByRole('button',{name:'外观设置'}).click()
  await expect(page.getByRole('heading',{name:'个性化',exact:true})).toBeVisible()
  await page.getByRole('button',{name:'深色',exact:true}).click();await expect(page.locator('.app')).toHaveAttribute('data-theme','dark')
  await expect(page.locator('.save-state')).toContainText('已保存');await page.reload();await page.getByRole('button',{name:'外观设置'}).click()
  await expect(page.locator('.app')).toHaveAttribute('data-theme','dark')
  await page.locator('#imageOpacity').fill('15');await expect(page.locator('output').first()).toHaveText('15%')
  expect(await page.locator('.app').evaluate(element=>getComputedStyle(element).opacity)).toBe('1')
  await page.screenshot({path:'test-results/personalization-dark.png'})
  await page.getByRole('button',{name:'跟随系统',exact:true}).click();await page.emulateMedia({colorScheme:'light'});await expect(page.locator('.app')).toHaveAttribute('data-theme','light')
  await page.emulateMedia({colorScheme:'dark'});await expect(page.locator('.app')).toHaveAttribute('data-theme','dark')
  await page.emulateMedia({reducedMotion:'reduce'});expect(await page.locator('.primary').count()).toBe(0)
})
test('窄窗口设置可滚动，界面不横向溢出',async({page})=>{
  await page.setViewportSize({width:840,height:600});await page.goto('/');await page.getByRole('button',{name:'外观设置'}).click()
  await page.getByRole('button',{name:'恢复默认外观'}).scrollIntoViewIfNeeded();await expect(page.getByRole('button',{name:'恢复默认外观'})).toBeVisible()
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
})
test('最低面板不透明度下，两种主题文字在极端背景仍满足对比度',async({page})=>{
  await page.goto('/');await page.getByRole('button',{name:'外观设置'}).click()
  for(const theme of ['浅色','深色']){
    await page.getByRole('button',{name:theme,exact:true}).click()
    const contrasts=await page.locator('.app').evaluate(element=>{
      const style=getComputedStyle(element);const surface=style.getPropertyValue('--surface-rgb').split(',').map(Number)
      const luminance=(rgb:number[])=>rgb.map(value=>{const n=value/255;return n<=.04045?n/12.92:((n+.055)/1.055)**2.4}).reduce((sum,n,i)=>sum+n*[.2126,.7152,.0722][i]!,0)
      return ['--text','--muted','--accent'].flatMap(token=>{const hex=style.getPropertyValue(token).trim().slice(1);const rgb=[0,2,4].map(index=>parseInt(hex.slice(index,index+2),16));return [0,255].map(background=>{const panel=surface.map(channel=>channel*.8+background*.2);const a=luminance(rgb),b=luminance(panel);return (Math.max(a,b)+.05)/(Math.min(a,b)+.05)})})
    })
    for(const contrast of contrasts)expect(contrast).toBeGreaterThanOrEqual(4.5)
  }
})
