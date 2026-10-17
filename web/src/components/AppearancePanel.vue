<script setup lang="ts">
import { ImagePlus, UserRound, RotateCcw, Sun, Moon, Monitor, X, Check } from '@lucide/vue'
import type { AppearanceSettings } from '../types'
defineProps<{settings:AppearanceSettings;avatar:string;background:string;saveStatus:string;importing:boolean}>()
const emit=defineEmits<{choose:[kind:'avatar'|'background'];remove:[kind:'avatar'|'background'];reset:[];retry:[]}>()
const sliders=[{key:'imageOpacity',label:'背景图片不透明度',min:0,max:100,unit:'%'},{key:'brightness',label:'背景亮度',min:40,max:140,unit:'%'},{key:'overlayOpacity',label:'颜色覆盖层不透明度',min:0,max:100,unit:'%'},{key:'panelOpacity',label:'内容面板不透明度',min:80,max:100,unit:'%'},{key:'positionX',label:'水平位置',min:0,max:100,unit:'%'},{key:'positionY',label:'垂直位置',min:0,max:100,unit:'%'}] as const
</script>
<template>
  <div class="settings-heading"><div><p class="eyebrow">让工作空间更像你</p><h1>个性化</h1></div><span class="save-state" role="status"><Check :size="14"/>{{saveStatus}}</span></div>
  <div class="appearance-layout"><div class="settings-sections">
    <section class="setting-section"><h2>主题模式</h2><p>与系统同步，或选择你偏好的明暗。</p><div class="theme-options">
      <button v-for="item in [{mode:'system',label:'跟随系统',icon:Monitor},{mode:'light',label:'浅色',icon:Sun},{mode:'dark',label:'深色',icon:Moon}] as const" :key="item.mode" :class="{active:settings.theme===item.mode}" :aria-pressed="settings.theme===item.mode" @click="settings.theme=item.mode"><span class="theme-mini" :class="item.mode"><i/><b/><em/></span><component :is="item.icon" :size="15"/>{{item.label}}</button>
    </div></section>
    <section class="setting-section"><h2>本地头像</h2><p>仅用于 Mokawa，不会修改或上传 PikPak 账号头像。</p><div class="avatar-editor"><span class="avatar large"><img v-if="avatar" :src="avatar" alt="Mokawa 本地头像"><UserRound v-else :size="32"/></span><div><button :disabled="importing" @click="emit('choose','avatar')">选择图片</button><button class="text-button" @click="emit('remove','avatar')">恢复默认头像</button><small>PNG、JPEG、WebP · 最大 12 MiB · 居中裁剪</small></div></div></section>
    <section class="setting-section"><h2>背景图片</h2><p>图片只保存在本机；重采样后加载，不随界面刷新反复解码。</p><div class="background-picker" :class="{has:background}" :style="{backgroundImage:background?`url(${background})`:undefined}"><button :disabled="importing" @click="emit('choose','background')"><ImagePlus :size="18"/>{{background?'更换背景':'选择背景图片'}}</button><button v-if="background" class="icon-button opaque" aria-label="移除背景" @click="emit('remove','background')"><X :size="16"/></button></div>
      <div class="setting-row"><label for="fit">显示方式</label><select id="fit" v-model="settings.fit"><option value="cover">填充窗口</option><option value="contain">完整显示</option><option value="stretch">拉伸</option><option value="center">居中原尺寸</option></select></div>
      <div class="setting-row"><label for="blur">背景模糊</label><select id="blur" v-model.number="settings.blur"><option :value="0">无模糊</option><option :value="6">轻微</option><option :value="14">中度</option><option :value="24">强模糊</option></select></div>
      <div v-for="slider in sliders" :key="slider.key" class="slider-setting"><label :for="slider.key">{{slider.label}}<output>{{settings[slider.key]}}{{slider.unit}}</output></label><input :id="slider.key" v-model.number="settings[slider.key]" type="range" :min="slider.min" :max="slider.max"></div>
      <div class="setting-row"><label for="overlay">背景覆盖颜色</label><input id="overlay" v-model="settings.overlayColor" type="color"></div>
      <p class="setting-note">图片、面板和覆盖层分别调整。文字和按钮保持清晰；面板不透明度至少 80%。模糊只作用于应用内背景，不是 Windows 系统毛玻璃。</p>
    </section>
    <section class="setting-section"><h2>动态效果</h2><label class="toggle-row"><span>减少动画与过渡<small>同时遵循 Windows 的减少动态效果偏好。</small></span><input v-model="settings.reducedMotion" type="checkbox"></label></section>
    <div class="settings-actions"><button @click="emit('reset')"><RotateCcw :size="15"/>恢复默认外观</button><button v-if="saveStatus==='保存失败'" @click="emit('retry')">重试保存</button></div>
  </div><aside class="live-preview"><h3>实时预览</h3><div class="preview-window"><div class="preview-title"><i class="brand-mark small">M</i>Mokawa<span>− □ ×</span></div><div class="preview-body"><div class="preview-nav"><b>我的文件</b><i/><i/><i/><span class="avatar"><img v-if="avatar" :src="avatar" alt="预览本地头像"><UserRound v-else :size="16"/></span></div><div class="preview-main"><b>我的文件</b><div class="preview-search"/><div class="preview-folders"><i/><i/></div><div class="preview-lines"><i/><i/><i/></div></div></div></div><p>预览中的图形仅用于外观示意，不代表真实网盘数据。</p><div class="privacy-note">本地个性化<br><small>头像与背景不会上传到 PikPak 或 Mokawa 服务器。</small></div></aside></div>
</template>
