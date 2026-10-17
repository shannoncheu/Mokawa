import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { call, desktop, errorMessage } from './bridge'
import { defaults, type AppearanceSettings, type Appearance } from './types'
export function useAppearance(notice:(message:string)=>void) {
  const settings=reactive(defaults()); const avatar=ref(''); const background=ref(''); const saveStatus=ref('已保存'); const importing=ref(false)
  const system=window.matchMedia('(prefers-color-scheme: dark)'); const systemDark=ref(system.matches)
  const changed=(event:MediaQueryListEvent)=>{systemDark.value=event.matches}; system.addEventListener('change',changed)
  const dark=computed(()=>settings.theme==='dark'||settings.theme==='system'&&systemDark.value)
  let initialized=false, timer:ReturnType<typeof setTimeout>|undefined, dirty=false, saving:Promise<void>|undefined, disposed=false
  let avatarGeneration=0,backgroundGeneration=0
  async function flush():Promise<void> {
    if(timer){clearTimeout(timer);timer=undefined}
    if(saving){await saving;return dirty?flush():undefined}
    if(!dirty)return
    dirty=false; const snapshot:AppearanceSettings={...settings}; saveStatus.value='正在保存…'
    saving=call<void>('save_appearance',{settings:snapshot}).then(()=>{saveStatus.value=dirty?'等待保存…':'已保存'},error=>{dirty=true;saveStatus.value='保存失败';notice(errorMessage(error))}).finally(()=>{saving=undefined})
    await saving
    if(dirty&&saveStatus.value!=='保存失败') await flush()
  }
  watch(settings,()=>{if(!initialized)return;dirty=true;saveStatus.value='等待保存…';if(timer)clearTimeout(timer);timer=setTimeout(()=>void flush(),450)})
  async function imageUrl(path:string|null,kind:'avatar'|'background') {
    const generation=kind==='avatar'?++avatarGeneration:++backgroundGeneration
    const target=kind==='avatar'?avatar:background
    if(!path){if(target.value)URL.revokeObjectURL(target.value);target.value='';return}
    let url=''
    try {
      if(!desktop)return
      const response=await fetch(`${convertFileSrc(path)}?v=${Date.now()}`); if(!response.ok)throw Error()
      const blob=await response.blob();if(blob.size>12*1024*1024)throw Error()
      url=URL.createObjectURL(blob);const image=new Image();image.src=url;await image.decode()
      if(image.naturalWidth*image.naturalHeight>4_096_000)throw Error()
      if(disposed||generation!==(kind==='avatar'?avatarGeneration:backgroundGeneration)){URL.revokeObjectURL(url);return}
      const old=target.value;target.value=url;if(old)URL.revokeObjectURL(old)
    } catch {if(url)URL.revokeObjectURL(url);if(generation===(kind==='avatar'?avatarGeneration:backgroundGeneration)){if(target.value)URL.revokeObjectURL(target.value);target.value='';notice('本地图片丢失、损坏或超出限制，已回退默认外观。')}}
  }
  async function initialize(appearance:Appearance){Object.assign(settings,appearance.settings);initialized=true;await Promise.all([imageUrl(appearance.avatar,'avatar'),imageUrl(appearance.background,'background')]);appearance.warnings.forEach(notice)}
  async function choose(kind:'avatar'|'background') {if(importing.value)return;importing.value=true;try{const path=await call<string|null>('choose_image',{kind});if(path)await imageUrl(path,kind)}catch(e){notice(errorMessage(e))}finally{importing.value=false}}
  async function remove(kind:'avatar'|'background'){try{await call('remove_image',{kind});await imageUrl(null,kind)}catch(e){notice(errorMessage(e))}}
  async function reset(){await Promise.all([remove('avatar'),remove('background')]);Object.assign(settings,defaults());await flush()}
  const style=computed(()=>({'--panel-alpha':String(settings.panelOpacity/100),'--image-opacity':String(settings.imageOpacity/100),'--image-blur':`${settings.blur}px`,'--image-brightness':`${settings.brightness}%`,'--overlay-color':settings.overlayColor,'--overlay-alpha':String(background.value?settings.overlayOpacity/100:0),'--background-position':`${settings.positionX}% ${settings.positionY}%`,'--background-size':settings.fit==='stretch'?'100% 100%':settings.fit==='center'?'auto':settings.fit,'--background-image':background.value?`url("${background.value}")`:'none'}))
  onBeforeUnmount(()=>{disposed=true;avatarGeneration++;backgroundGeneration++;system.removeEventListener('change',changed);if(timer)clearTimeout(timer);if(avatar.value)URL.revokeObjectURL(avatar.value);if(background.value)URL.revokeObjectURL(background.value)})
  return {settings,avatar,background,dark,style,saveStatus,importing,initialize,choose,remove,reset,flush}
}
