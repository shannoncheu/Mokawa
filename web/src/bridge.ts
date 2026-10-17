import { invoke, isTauri } from '@tauri-apps/api/core'
import { defaults, type AppError } from './types'
export const desktop = isTauri()
export async function call<T>(command:string,args?:Record<string,unknown>):Promise<T> {
  if(desktop) return invoke<T>(command,args)
  // Browser preview is explicitly labeled and never claims a successful account/API call.
  if(command==='bootstrap') return {appearance:{settings:JSON.parse(localStorage.getItem('mokawa-preview-appearance')||'null')||defaults(),avatar:null,background:null,warnings:[]},hasSession:false,authorizationReady:false,warning:null} as T
  if(command==='save_appearance'){localStorage.setItem('mokawa-preview-appearance',JSON.stringify(args?.settings));return undefined as T}
  if(command==='cancel_page') return undefined as T
  if(command==='transfer_snapshot') return {records:[],errors:{},speeds:{},persistence_error:null} as T
  throw {code:command==='list_files'?'authentication':'operation',message:command==='begin_authorization'?'PikPak 授权尚未接入：Mokawa 客户端注册与回调规则仍待确认。':'此操作需要 Windows 桌面版本；浏览器预览不连接网盘或访问本机图片。'} satisfies AppError
}
export function errorMessage(error:unknown):string { return typeof error==='object'&&error!==null&&'message'in error?String(error.message):'操作未完成，请重试。' }
