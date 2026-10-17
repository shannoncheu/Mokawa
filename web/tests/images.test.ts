import {mount} from '@vue/test-utils'
import {defineComponent} from 'vue'
import {expect,it,vi} from 'vitest'
import {useAppearance} from '../src/useAppearance'
import {defaults} from '../src/types'
vi.mock('../src/bridge',()=>({desktop:true,call:vi.fn(),errorMessage:()=> 'error'}))
vi.mock('@tauri-apps/api/core',()=>({convertFileSrc:(path:string)=>`http://asset.localhost/${path}`}))
it('反复更换头像仅保留当前 URL，销毁时释放图片与请求',async()=>{
  let counter=0;const revoke=vi.fn();URL.createObjectURL=vi.fn(()=>`blob:owned-${++counter}`);URL.revokeObjectURL=revoke
  const signals:AbortSignal[]=[]
  vi.stubGlobal('fetch',vi.fn(async(_url:string,options:{signal:AbortSignal})=>{signals.push(options.signal);return {ok:true,blob:async()=>new Blob(['image'])}}))
  vi.stubGlobal('Image',class{src='';naturalWidth=256;naturalHeight=256;async decode(){}})
  let appearance!:ReturnType<typeof useAppearance>
  const wrapper=mount(defineComponent({setup(){appearance=useAppearance(vi.fn());return()=>null}}))
  for(let i=0;i<20;i++)await appearance.initialize({settings:defaults(),avatar:`avatar-${i}.png`,background:null,warnings:[]})
  expect(appearance.avatar.value).toBe('blob:owned-20');expect(revoke).toHaveBeenCalledTimes(19)
  wrapper.unmount();expect(revoke).toHaveBeenCalledTimes(20);expect(signals.every(signal=>signal.aborted)).toBe(true)
})
