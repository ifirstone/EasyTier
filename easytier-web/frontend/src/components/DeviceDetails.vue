<script setup lang="ts">
import { computed, ref } from 'vue';
import { Button, Dialog, Dropdown, InputText, ProgressSpinner } from 'primevue';
import { useToast } from 'primevue/usetoast';
import { Utils } from 'easytier-frontend-lib';
import { useI18n } from 'vue-i18n';
import { decryptHostnameSuffix } from '../modules/hostname-decrypt';
import { fetchLatestRelease, getArchKey, matchCoreAsset, proxyDownloadUrl, type Release } from '../modules/github-releases';

const { t } = useI18n();
const toast = useToast();

const props = defineProps<{
    device: Utils.DeviceInfo;
    containerClass?: string;
    compact?: boolean;
}>();

const showDecryptDialog = ref(false);
const decryptKey = ref('');
const decryptResult = ref<{ ip: string; mask: string; subnet: string } | null>(null);
const decryptError = ref('');

const hostnameSuffix = computed(() => {
    const parts = props.device.hostname.split('-');
    return parts.length > 1 ? parts.pop()! : '';
});

const openDecryptDialog = () => {
    decryptKey.value = '';
    decryptResult.value = null;
    decryptError.value = '';
    showDecryptDialog.value = true;
};

const doDecrypt = () => {
    decryptError.value = '';
    if (!decryptKey.value.trim()) {
        decryptError.value = '请输入密钥';
        return;
    }
    const result = decryptHostnameSuffix(hostnameSuffix.value, decryptKey.value.trim());
    if (!result) {
        decryptError.value = '解密失败，请检查密钥或密文是否完整';
        return;
    }
    decryptResult.value = result;
};

const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
        toast.add({ severity: 'success', summary: '已复制', life: 1000 });
    });
};

const showInstallDialog = ref(false);
const installLoading = ref(false);
const release = ref<Release | null>(null);
const selectedArch = ref(getArchKey());
const matchedAsset = computed(() => {
    if (!release.value || selectedArch.value === 'unknown') return null;
    return matchCoreAsset(release.value, selectedArch.value);
});
const proxyUrl = computed(() => {
    if (!matchedAsset.value) return '';
    return proxyDownloadUrl(matchedAsset.value.browser_download_url);
});

const openInstallDialog = async () => {
    release.value = null;
    showInstallDialog.value = true;
    if (!release.value) {
        installLoading.value = true;
        try {
            const data = await fetchLatestRelease();
            release.value = data;
        } finally {
            installLoading.value = false;
        }
    }
};
</script>

<template>
  <div :class="['device-details', containerClass, { 'compact': compact }]">
    <div class="detail-item hostname">
      <div class="detail-label">{{ t('web.device.hostname') }}</div>
      <div class="detail-value" style="display: flex; align-items: center; gap: 0.5rem;">
        <span>{{ device.hostname }}</span>
        <Button
          icon="pi pi-unlock"
          severity="secondary"
          text
          rounded
          size="small"
          v-tooltip.top="'解码主机名'"
          @click="openDecryptDialog"
        />
      </div>
    </div>
    <div class="detail-item public-ip">
      <div class="detail-label">{{ t('web.device.public_ip') }}</div>
      <div class="detail-value">{{ device.public_ip }}</div>
    </div>
    <div class="detail-item running-networks">
      <div class="detail-label">{{ t('web.device.networks') }}</div>
      <div class="detail-value">{{ device.running_network_count }}</div>
    </div>
    <div class="detail-item last-report">
      <div class="detail-label">{{ t('web.device.last_report') }}</div>
      <div class="detail-value">{{ device.report_time }}</div>
    </div>
    <div class="detail-item version">
      <div class="detail-label">{{ t('web.device.version') }}</div>
      <div class="detail-value">{{ device.easytier_version }}</div>
    </div>
    <div class="detail-item machine-id">
      <div class="detail-label">{{ t('web.device.machine_id') }}</div>
      <div class="detail-value">
        <span class="machine-id-value" :title="device.machine_id">{{ device.machine_id }}</span>
      </div>
    </div>
    <div class="detail-item install-client">
      <div class="detail-label">客户端安装</div>
      <div class="detail-value">
        <Button
          icon="pi pi-download"
          severity="secondary"
          text
          rounded
          size="small"
          v-tooltip.top="'安装客户端'"
          @click="openInstallDialog"
        />
      </div>
    </div>
  </div>

  <Dialog v-model:visible="showDecryptDialog" modal :style="{ width: '28rem' }" :header="'解码主机名'">
    <div class="flex flex-col gap-4">
      <div class="text-sm text-color-secondary">
        <div>主机名后缀（密文）：{{ hostnameSuffix }}</div>
      </div>
      <div class="flex flex-col gap-2">
        <label for="decrypt-key" class="text-sm font-semibold">密钥</label>
        <InputText id="decrypt-key" v-model="decryptKey" placeholder="请输入密钥" @keyup.enter="doDecrypt" />
      </div>
      <Button label="解密" @click="doDecrypt" />
      <div v-if="decryptError" class="text-red-500 text-sm">{{ decryptError }}</div>
      <div v-if="decryptResult" class="flex flex-col gap-2 border-t pt-3">
        <div class="flex items-center justify-between">
          <span class="text-sm"><strong>解密 IP：</strong>{{ decryptResult.ip }}/{{ decryptResult.mask }}</span>
          <Button icon="pi pi-copy" text size="small" @click="copyToClipboard(decryptResult.ip + '/' + decryptResult.mask)" />
        </div>
        <div class="flex items-center justify-between">
          <span class="text-sm"><strong>推算网段：</strong>{{ decryptResult.subnet }}/{{ decryptResult.mask }}</span>
          <Button icon="pi pi-copy" text size="small" @click="copyToClipboard(decryptResult.subnet + '/' + decryptResult.mask)" />
        </div>
      </div>
    </div>
  </Dialog>

  <Dialog v-model:visible="showInstallDialog" modal :style="{ width: '32rem' }" :header="'安装 EasyTier 客户端'">
    <div class="flex flex-col gap-4">
      <div v-if="installLoading" class="flex justify-center">
        <ProgressSpinner />
      </div>
      <template v-else-if="release">
        <div class="text-sm text-color-secondary">
          <div><strong>最新版本：</strong>{{ release.tag_name }}</div>
          <div><strong>预发布：</strong>{{ release.prerelease ? '是' : '否' }}</div>
        </div>
        <div class="flex flex-col gap-2">
          <label for="install-arch" class="text-sm font-semibold">架构</label>
          <Dropdown
            id="install-arch"
            v-model="selectedArch"
            :options="[
                { label: 'Linux x86_64', value: 'linux-x86_64' },
                { label: 'Linux aarch64', value: 'linux-aarch64' },
                { label: 'Windows x86_64', value: 'windows-x86_64' },
                { label: 'macOS x86_64', value: 'macos-x86_64' },
                { label: 'macOS aarch64', value: 'macos-aarch64' },
                { label: '自动检测', value: getArchKey() },
            ]"
            optionLabel="label"
            optionValue="value"
            placeholder="选择架构"
          />
        </div>
        <div v-if="matchedAsset" class="flex flex-col gap-2 border-t pt-3">
          <div class="text-sm"><strong>匹配文件：</strong>{{ matchedAsset.name }}</div>
          <div class="text-xs text-color-secondary break-all">下载地址：{{ proxyUrl }}</div>
          <div class="flex gap-2">
            <Button label="复制下载地址" icon="pi pi-copy" @click="copyToClipboard(proxyUrl)" />
            <Button label="下载" icon="pi pi-download" @click="window.open(proxyUrl, '_blank')" />
          </div>
        </div>
        <div v-else class="text-red-500 text-sm">
          未找到匹配当前架构的客户端文件，请检查 releases 页面。
        </div>
      </template>
      <div v-else class="text-red-500 text-sm">获取 release 信息失败。</div>
    </div>
  </Dialog>
</template>

<style scoped>
/* 基础布局 */
.device-details {
  display: grid;
  grid-template-columns: 1fr;
  gap: 0.75rem;
}

/* 标准布局的详情项样式 */
.detail-item {
  position: relative;
  border-bottom: 1px solid var(--surface-border, #e9ecef);
  padding-bottom: 0.75rem;
  transition: all 0.2s;
  border-radius: 0.25rem;
}

.detail-item:hover {
  background-color: var(--surface-hover, rgba(245, 247, 250, 0.5));
}

.detail-item:last-child {
  border-bottom: none;
}

.detail-label {
  font-weight: 600;
  color: var(--text-color, #334155);
  font-size: 0.95rem;
  margin-bottom: 0.375rem;
  display: flex;
  align-items: center;
}

/* 紧凑布局样式 */
.device-details.compact {
  gap: 0.4rem;
}

.compact .detail-item {
  padding: 0.3rem 0.2rem;
  display: grid;
  grid-template-columns: 40% 60%;
  align-items: center;
}

.compact .detail-label {
  margin-bottom: 0;
}

.detail-label::before {
  content: "";
  display: inline-block;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background-color: #3b82f6;
  margin-right: 0.5rem;
}

.detail-value {
  color: var(--text-color-secondary, #475569);
  word-break: break-all;
  padding-left: 1rem;
  line-height: 1.4;
  font-size: 0.95rem;
}

/* 紧凑布局的标签和值样式 */
.compact .detail-label::before {
  width: 3px;
  height: 3px;
  margin-right: 0.3rem;
}

.compact .detail-value {
  padding-left: 0.3rem;
  line-height: 1.2;
}

/* 特定字段的样式 */
.hostname .detail-label::before {
  background-color: #3b82f6;
  /* 蓝色 */
}

.public-ip .detail-label::before {
  background-color: #10b981;
  /* 绿色 */
}

.running-networks .detail-label::before {
  background-color: #f59e0b;
  /* 橙色 */
}

.last-report .detail-label::before {
  background-color: #8b5cf6;
  /* 紫色 */
}

.version .detail-label::before {
  background-color: #ec4899;
  /* 粉色 */
}

.machine-id .detail-label::before {
  background-color: #6b7280;
  /* 灰色 */
}

.install-client .detail-label::before {
  background-color: #10b981;
  /* 绿色 */
}

/* 机器ID特殊样式 */
.machine-id-value {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.95rem;
  background-color: var(--surface-ground, #f1f5f9);
  color: var(--text-color, #1f2937);
  padding: 0.25rem 0.5rem;
  border-radius: 0.25rem;
  border: 1px solid var(--surface-border, #e2e8f0);
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 紧凑布局下的机器ID样式 */
.compact .machine-id-value {
  font-size: 0.75rem;
  padding: 0.15rem 0.3rem;
  border-radius: 0.2rem;
}

/* 暗黑模式适配 */
@media (prefers-color-scheme: dark) {
  .detail-item {
    border-bottom: 1px solid var(--surface-border, #334155);
  }

  .detail-item:last-child {
    border-bottom: none;
  }

  .detail-item:hover {
    background-color: var(--surface-hover, rgba(30, 41, 59, 0.4));
  }

  .detail-value {
    color: var(--text-color-secondary, #cbd5e1);
  }

  .detail-label {
    color: var(--text-color, #e2e8f0);
  }

  .machine-id-value {
    background-color: var(--surface-ground, #1e293b);
    color: var(--text-color, #f1f5f9);
    border-color: var(--surface-border, #334155);
  }
}
</style>
