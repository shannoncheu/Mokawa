import { chromium } from '@playwright/test'
import { spawn } from 'node:child_process'
import { setTimeout as delay } from 'node:timers/promises'
import { writeFile } from 'node:fs/promises'
const executable=process.env.MOKAWA_TEST_EXECUTABLE
if(!executable)throw Error('Windows test executable missing')
const child=spawn(executable,[],{env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=9222'}})
let browser
try{
  for(let i=0;i<60;i++){try{browser=await chromium.connectOverCDP('http://127.0.0.1:9222',{timeout:1000});break}catch{await delay(500)}}
  if(!browser)throw Error('WebView2 debug connection unavailable')
  const context=browser.contexts()[0];let page=context.pages()[0]
  for(let i=0;i<40&&!page;i++){await delay(200);page=context.pages()[0]}
  if(!page)throw Error('No native WebView2 page')
  await page.getByRole('heading',{name:'连接 PikPak 账号'}).waitFor({timeout:20000})
  await page.screenshot({path:'dist/tauri-login.png'})
  await page.getByRole('button',{name:'外观设置'}).click()
  await page.getByRole('heading',{name:'个性化',exact:true}).waitFor()
  await page.getByRole('button',{name:'深色',exact:true}).click()
  await page.waitForFunction(()=>document.querySelector('.app')?.getAttribute('data-theme')==='dark')
  await page.waitForFunction(()=>document.querySelector('.save-state')?.textContent?.includes('已保存'))
  await page.screenshot({path:'dist/tauri-personalization-dark.png'})
  await writeFile('dist/tauri-native-results.json',JSON.stringify({nativeWebView2:true,login:true,settings:true,themeClick:true,save:true},null,2))
  await page.getByRole('button',{name:'关闭窗口'}).click()
  await Promise.race([new Promise(resolve=>child.once('exit',resolve)),delay(10000).then(()=>{throw Error('Native process did not exit')})])
}finally{await browser?.close().catch(()=>{});if(child.exitCode===null)child.kill()}
