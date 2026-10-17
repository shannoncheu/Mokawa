import { mount } from '@vue/test-utils'
import { defineComponent, nextTick } from 'vue'
import { afterEach,expect,it,vi } from 'vitest'
import { useAppearance } from '../src/useAppearance'
import { defaults } from '../src/types'
const call=vi.fn(async()=>{})
vi.mock('../src/bridge',()=>({call:(...args:unknown[])=>call(...args),desktop:false,errorMessage:()=> '保存错误'}))
afterEach(()=>{vi.useRealTimers();call.mockClear()})
it('滑块即时反馈，连续变化仅写一次最新值，并清理系统订阅和计时器',async()=>{
  vi.useFakeTimers();const remove=vi.fn();const add=vi.fn();vi.mocked(window.matchMedia).mockReturnValue({matches:false,addEventListener:add,removeEventListener:remove} as unknown as MediaQueryList)
  let appearance!:ReturnType<typeof useAppearance>
  const wrapper=mount(defineComponent({setup(){appearance=useAppearance(vi.fn());return()=>null}}))
  await appearance.initialize({settings:defaults(),avatar:null,background:null,warnings:[]})
  for(let value=0;value<=100;value++){appearance.settings.imageOpacity=value;await nextTick()}
  expect(appearance.style.value['--image-opacity']).toBe('1');expect(call).not.toHaveBeenCalled()
  await vi.advanceTimersByTimeAsync(500);expect(call).toHaveBeenCalledTimes(1)
  expect(call.mock.calls[0]?.[1]).toMatchObject({settings:{imageOpacity:100}})
  appearance.settings.imageOpacity=40;await nextTick();wrapper.unmount()
  await vi.advanceTimersByTimeAsync(1000);expect(call).toHaveBeenCalledTimes(1)
  expect(remove).toHaveBeenCalledWith('change',add.mock.calls[0]?.[1])
})
it('保存失败明确显示，允许显式重试，不产生无限重试',async()=>{
  vi.useFakeTimers();call.mockRejectedValueOnce(Error('disk'))
  let appearance!:ReturnType<typeof useAppearance>
  const wrapper=mount(defineComponent({setup(){appearance=useAppearance(vi.fn());return()=>null}}))
  await appearance.initialize({settings:defaults(),avatar:null,background:null,warnings:[]})
  appearance.settings.brightness=70;await nextTick();await vi.advanceTimersByTimeAsync(500)
  expect(appearance.saveStatus.value).toBe('保存失败');await vi.advanceTimersByTimeAsync(10000);expect(call).toHaveBeenCalledTimes(1)
  await appearance.flush();expect(appearance.saveStatus.value).toBe('已保存');wrapper.unmount()
})
