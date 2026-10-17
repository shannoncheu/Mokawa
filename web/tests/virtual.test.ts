import { mount } from '@vue/test-utils'
import { expect,it,vi } from 'vitest'
import VirtualFiles from '../src/components/VirtualFiles.vue'
import type { CloudFile } from '../src/types'
it('一万条文件只挂载可见行，销毁时释放尺寸订阅',async()=>{
  const disconnect=vi.fn();vi.stubGlobal('ResizeObserver',class{observe(){}disconnect=disconnect})
  const files:CloudFile[]=Array.from({length:10_000},(_,i)=>({id:String(i),name:`测试文件${i}`,kind:'drive#file',size:i,modified_time:'',mime_type:''}))
  for(let cycle=0;cycle<10;cycle++){
    const wrapper=mount(VirtualFiles,{props:{files,grid:false,selected:new Set<string>()}})
    expect(wrapper.findAll('.file-item').length).toBeLessThan(25)
    await wrapper.setProps({grid:true});expect(wrapper.findAll('.file-item').length).toBeLessThan(40)
    await wrapper.find('.file-item').trigger('click',{ctrlKey:true});expect(wrapper.emitted('select')?.[0]?.[1]).toBe(true)
    wrapper.unmount()
  }
  expect(disconnect).toHaveBeenCalledTimes(10)
})
