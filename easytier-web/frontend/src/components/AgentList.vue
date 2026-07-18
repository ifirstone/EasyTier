<template>
  <div class="agent-list">
    <div class="flex justify-between items-center mb-4">
      <h2 class="text-2xl font-bold">{{ t('web.agent.title') }}</h2>
      <Button :label="t('web.agent.tooltip_refresh')" @click="loadAgents" :loading="refreshing" />
    </div>

    <div class="card">
      <DataTable :value="agents" :loading="loading" paginator :rows="10" tableStyle="min-width: 80rem">
        <Column field="hostname" :header="t('web.agent.hostname')" sortable></Column>
        <Column field="public_ip" :header="t('web.agent.public_ip')" sortable>
          <template #body="{ data }">
            <div class="flex items-center gap-2">
              <span class="text-sm">{{ getDisplayIP(data) }}</span>
              <Button 
                :icon="isIPUnlocked(data.agent_id) ? 'pi pi-lock-open' : 'pi pi-lock'" 
                size="small" 
                text 
                :title="isIPUnlocked(data.agent_id) ? t('web.agent.ip_unlocked') : t('web.agent.ip_locked')"
                @click="toggleIPLock(data)" 
              />
            </div>
          </template>
        </Column>
        <Column field="arch" :header="t('web.agent.arch')" sortable></Column>
        <Column field="os" :header="t('web.agent.os')" sortable></Column>
        <Column :header="t('web.agent.status')" sortable field="status">
          <template #body="{ data }">
            <Tag :value="data.status" :severity="getStatusSeverity(data.status)" />
          </template>
        </Column>
        <Column :header="t('web.agent.core_status')">
          <template #body="{ data }">
            <div v-if="data.status === 'offline'" class="flex flex-col gap-1">
              <Tag value="Unknown" severity="warning" />
              <Tag value="Unknown" severity="warning" />
            </div>
            <div v-else class="flex flex-col gap-1">
              <Tag v-if="data.core_installed" value="Installed" severity="success" />
              <Tag v-else value="Not Installed" severity="danger" />
              <Tag v-if="data.core_running" value="Running" severity="info" />
              <Tag v-else-if="data.core_installed" value="Stopped" severity="danger" />
              <span v-if="data.core_version" class="text-sm text-gray-500">{{ data.core_version }}</span>
            </div>
          </template>
        </Column>
        <Column :header="t('web.agent.core_uri')" field="core_uri">
          <template #body="{ data }">
            <span v-if="data.core_uri" class="text-sm break-all">{{ data.core_uri }}</span>
            <span v-else class="text-sm text-gray-400">-</span>
          </template>
        </Column>
        <Column :header="t('web.agent.last_heartbeat')" field="last_heartbeat" sortable>
          <template #body="{ data }">
            <span class="text-sm">{{ formatTime(data.last_heartbeat) }}</span>
          </template>
        </Column>
        <Column :header="t('web.agent.actions')">
          <template #body="{ data }">
            <div class="flex flex-wrap gap-1">
              <Button :label="t('web.agent.install')" size="small" @click="openCommandDialog(data, 'install')" />
              <Button :label="t('web.agent.restart')" size="small" @click="openCommandDialog(data, 'restart')" />
              <Button :label="t('web.agent.stop')" size="small" @click="openCommandDialog(data, 'stop')" />
              <Button :label="t('web.agent.uninstall')" size="small" severity="danger" @click="openCommandDialog(data, 'uninstall')" />
              <Button :label="t('web.agent.delete')" size="small" severity="danger" @click="confirmDelete(data)" />
              <Button :label="t('web.agent.uninstall_agent')" size="small" severity="danger" @click="openCommandDialog(data, 'uninstall_agent')" />
            </div>
          </template>
        </Column>
      </DataTable>
    </div>

    <Dialog v-model:visible="commandDialogVisible" modal :header="getCommandTitle()" :style="{ width: '28rem' }">
      <div class="flex flex-col gap-4">
        <div v-if="selectedAgent" class="text-sm">
          <div><strong>Hostname:</strong> {{ selectedAgent.hostname }}</div>
          <div><strong>IP:</strong> {{ getDisplayIP(selectedAgent) }}</div>
          <div><strong>Status:</strong> {{ selectedAgent.status }}</div>
        </div>

        <div v-if="commandType === 'install'" class="flex flex-col gap-2">
          <label class="text-sm font-semibold">{{ t('web.agent.install_select_version') }}</label>
          <Dropdown
            v-model="selectedVersion"
            :options="releases"
            optionLabel="label"
            optionValue="value"
            :loading="fetchingReleases"
            :placeholder="t('web.agent.install_select_version')"
          />
          <div v-if="fetchReleasesError" class="text-red-500 text-sm">{{ fetchReleasesError }}</div>
        </div>

        <div v-if="commandType === 'uninstall'" class="flex flex-col gap-2">
          <label class="text-sm font-semibold">Options</label>
          <div class="flex items-center gap-2">
            <Checkbox id="keep-config" v-model="keepConfig" :binary="true" />
            <label for="keep-config" class="text-sm">Keep configuration files</label>
          </div>
        </div>

        <div v-if="commandType === 'uninstall_agent'" class="text-sm text-red-600">
          This will uninstall the agent software and remove its systemd service. easytier-core will remain installed.
        </div>

        <div class="flex gap-2 justify-end">
          <Button :label="t('web.common.cancel')" @click="commandDialogVisible = false" />
          <Button :label="getCommandButtonLabel()" @click="sendCommand" :loading="sending" />
        </div>
      </div>
    </Dialog>

    <Dialog v-model:visible="deleteDialogVisible" modal :header="t('web.agent.delete')" :style="{ width: '24rem' }">
      <div class="flex flex-col gap-4">
        <div class="text-sm">{{ t('web.agent.delete_confirm') }}</div>
        <div class="flex gap-2 justify-end">
          <Button :label="t('web.common.cancel')" @click="deleteDialogVisible = false" />
          <Button :label="t('web.common.delete')" severity="danger" @click="deleteAgent" :loading="deleting" />
        </div>
      </div>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { Button, Column, DataTable, Dialog, Dropdown, Tag, Checkbox } from 'primevue';
import ApiClient from '../modules/api';

const { t } = useI18n();
const props = defineProps({
    api: ApiClient,
});
const api = computed(() => props.api as ApiClient);
const loading = ref(false);
const refreshing = ref(false);
const agents = ref([]);
const commandDialogVisible = ref(false);
const deleteDialogVisible = ref(false);
const selectedAgent = ref<any>(null);
const commandType = ref('');
const installVersion = ref('');
const selectedVersion = ref('');
const keepConfig = ref(true);
const sending = ref(false);
const deleting = ref(false);
const ipKey = ref('easytier');
const unlockedIps = ref<Set<string>>(new Set());
const releases = ref<Array<{ label: string; value: string; prerelease: boolean }>>([]);
const fetchingReleases = ref(false);
const fetchReleasesError = ref('');

const encryptIP = (ip: string, key: string): string => {
  if (!ip || !key) return ip;
  const parts = ip.split('.').map(p => parseInt(p, 10)).filter(n => !isNaN(n));
  const keyBytes = new TextEncoder().encode(key);
  let hex = '';
  for (let i = 0; i < parts.length; i++) {
    const k = keyBytes[i % keyBytes.length];
    const x = parts[i] ^ k;
    hex += x.toString(16).padStart(2, '0').toUpperCase();
  }
  return hex || ip;
};

const decryptIP = (hex: string, key: string): string => {
  if (!hex || !key) return hex;
  const keyBytes = new TextEncoder().encode(key);
  const parts: number[] = [];
  for (let i = 0; i < hex.length; i += 2) {
    const x = parseInt(hex.substr(i, 2), 16);
    const k = keyBytes[(i / 2) % keyBytes.length];
    parts.push(x ^ k);
  }
  return parts.join('.');
};

const getDisplayIP = (agent: any): string => {
  if (!agent || !agent.public_ip) return '-';
  if (unlockedIps.value.has(agent.agent_id)) {
    return agent.public_ip;
  }
  return encryptIP(agent.public_ip, ipKey.value);
};

const isIPUnlocked = (agentId: string): boolean => {
  return unlockedIps.value.has(agentId);
};

const toggleIPLock = async (agent: any) => {
  if (!agent) return;
  if (unlockedIps.value.has(agent.agent_id)) {
    unlockedIps.value.delete(agent.agent_id);
  } else {
    const key = window.prompt(t('web.agent.enter_decrypt_key'));
    if (key !== null) {
      ipKey.value = key || 'easytier';
      unlockedIps.value.add(agent.agent_id);
    }
  }
};

const loadAgents = async (silent = false) => {
  if (!silent) {
    loading.value = true;
  } else {
    refreshing.value = true;
  }
  try {
    const data = await api.value.getAgentList();
    agents.value = data.agents || [];
  } catch (e) {
    console.error('load agents failed', e);
  } finally {
    loading.value = false;
    refreshing.value = false;
  }
};

const getStatusSeverity = (status: string) => {
  switch (status) {
    case 'online': return 'success';
    case 'offline': return 'danger';
    default: return 'warning';
  }
};

const formatTime = (timeStr: string) => {
  if (!timeStr) return '-';
  try {
    const date = new Date(timeStr);
    if (isNaN(date.getTime())) return timeStr;
    return date.toLocaleString();
  } catch {
    return timeStr;
  }
};

const fetchReleases = async () => {
  fetchingReleases.value = true;
  fetchReleasesError.value = '';
  try {
    const resp = await fetch('https://api.github.com/repos/EasyTier/EasyTier/releases?per_page=10');
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const data = await resp.json();
    releases.value = data.map((r: any) => ({
      label: r.tag_name + (r.prerelease ? ` (${t('web.agent.prerelease')})` : ''),
      value: r.tag_name,
      prerelease: r.prerelease,
    }));
    if (releases.value.length > 0) {
      selectedVersion.value = releases.value[0].value;
    }
  } catch (e: any) {
    fetchReleasesError.value = t('web.agent.fetch_releases_failed') + ': ' + (e?.message || e);
  } finally {
    fetchingReleases.value = false;
  }
};

const openCommandDialog = (agent: any, type: string) => {
  selectedAgent.value = agent;
  commandType.value = type;
  installVersion.value = '';
  selectedVersion.value = '';
  keepConfig.value = true;
  releases.value = [];
  fetchReleasesError.value = '';
  commandDialogVisible.value = true;
  if (type === 'install') {
    fetchReleases();
  }
};

const confirmDelete = (agent: any) => {
  selectedAgent.value = agent;
  deleteDialogVisible.value = true;
};

const getCommandTitle = () => {
  switch (commandType.value) {
    case 'install': return t('web.agent.install') + ' EasyTier Core';
    case 'restart': return t('web.agent.restart') + ' EasyTier Core';
    case 'uninstall': return t('web.agent.uninstall') + ' EasyTier Core';
    case 'stop': return t('web.agent.stop') + ' EasyTier Core';
    case 'uninstall_agent': return t('web.agent.uninstall_agent');
    default: return 'Command';
  }
};

const getCommandButtonLabel = () => {
  switch (commandType.value) {
    case 'install': return t('web.agent.install');
    case 'restart': return t('web.agent.restart');
    case 'uninstall': return t('web.agent.uninstall');
    case 'stop': return t('web.agent.stop');
    case 'uninstall_agent': return t('web.agent.uninstall_agent');
    default: return 'Send';
  }
};

const sendCommand = async () => {
  if (!selectedAgent.value) return;
  sending.value = true;
  try {
    let payload: any = {};
    switch (commandType.value) {
      case 'install':
        payload = { version: selectedVersion.value || null };
        break;
      case 'uninstall':
        payload = { keep_config: keepConfig.value };
        break;
      case 'restart':
        payload = {};
        break;
      case 'stop':
        payload = {};
        break;
      case 'uninstall_agent':
        payload = {};
        break;
    }
    await api.value.sendAgentCommand(selectedAgent.value.agent_id, commandType.value, payload);
    commandDialogVisible.value = false;
    await loadAgents(true);
  } catch (e) {
    console.error('send command failed', e);
  } finally {
    sending.value = false;
  }
};

const deleteAgent = async () => {
  if (!selectedAgent.value) return;
  deleting.value = true;
  try {
    await api.value.deleteAgent(selectedAgent.value.agent_id);
    deleteDialogVisible.value = false;
    await loadAgents(true);
  } catch (e) {
    console.error('delete agent failed', e);
  } finally {
    deleting.value = false;
  }
};

onMounted(() => {
  loadAgents();
  setInterval(() => loadAgents(true), 5000);
});
</script>

<style scoped>
.agent-list {
  padding: 1rem;
}
</style>