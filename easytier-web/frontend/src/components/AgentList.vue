<template>
  <div class="agent-list">
    <div class="flex justify-between items-center mb-4">
      <h2 class="text-2xl font-bold">{{ t('web.agent.title') }}</h2>
      <Button icon="pi pi-refresh" @click="loadAgents" :loading="refreshing" />
    </div>

    <div class="card">
      <DataTable :value="agents" :loading="loading" paginator :rows="10" tableStyle="min-width: 80rem">
        <Column field="hostname" :header="t('web.agent.hostname')" sortable></Column>
        <Column field="public_ip" :header="t('web.agent.public_ip')" sortable></Column>
        <Column field="arch" :header="t('web.agent.arch')" sortable></Column>
        <Column field="os" :header="t('web.agent.os')" sortable></Column>
        <Column :header="t('web.agent.status')" sortable field="status">
          <template #body="{ data }">
            <Tag :value="data.status" :severity="getStatusSeverity(data.status)" />
          </template>
        </Column>
        <Column :header="t('web.agent.core_status')">
          <template #body="{ data }">
            <div class="flex flex-col gap-1">
              <Tag v-if="data.core_installed" value="Installed" severity="success" />
              <Tag v-else value="Not Installed" severity="danger" />
              <Tag v-if="data.core_running" value="Running" severity="info" />
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
            <div class="flex gap-2">
              <Button icon="pi pi-play" size="small" @click="openCommandDialog(data, 'install')" />
              <Button icon="pi pi-refresh" size="small" @click="openCommandDialog(data, 'restart')" />
              <Button icon="pi pi-times-circle" size="small" severity="danger" @click="openCommandDialog(data, 'uninstall')" />
              <Button icon="pi pi-trash" size="small" severity="danger" @click="confirmDelete(data)" />
              <Button icon="pi pi-stop" size="small" severity="warning" @click="openCommandDialog(data, 'stop')" />
            </div>
          </template>
        </Column>
      </DataTable>
    </div>

    <Dialog v-model:visible="commandDialogVisible" modal :header="getCommandTitle()" :style="{ width: '28rem' }">
      <div class="flex flex-col gap-4">
        <div v-if="selectedAgent" class="text-sm">
          <div><strong>Hostname:</strong> {{ selectedAgent.hostname }}</div>
          <div><strong>IP:</strong> {{ selectedAgent.public_ip }}</div>
          <div><strong>Status:</strong> {{ selectedAgent.status }}</div>
        </div>

        <div v-if="commandType === 'install'" class="flex flex-col gap-2">
          <label class="text-sm font-semibold">Version</label>
          <InputText v-model="installVersion" placeholder="e.g. v2.5.0 or leave empty for latest" />
        </div>

        <div v-if="commandType === 'uninstall'" class="flex flex-col gap-2">
          <label class="text-sm font-semibold">Options</label>
          <div class="flex items-center gap-2">
            <Checkbox id="keep-config" v-model="keepConfig" :binary="true" />
            <label for="keep-config" class="text-sm">Keep configuration files</label>
          </div>
        </div>

        <div class="flex gap-2 justify-end">
          <Button label="Cancel" @click="commandDialogVisible = false" />
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
import { Button, Column, DataTable, Dialog, InputText, Tag, Checkbox } from 'primevue';
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
const keepConfig = ref(true);
const sending = ref(false);
const deleting = ref(false);

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

const openCommandDialog = (agent: any, type: string) => {
  selectedAgent.value = agent;
  commandType.value = type;
  installVersion.value = '';
  keepConfig.value = true;
  commandDialogVisible.value = true;
};

const confirmDelete = (agent: any) => {
  selectedAgent.value = agent;
  deleteDialogVisible.value = true;
};

const getCommandTitle = () => {
  switch (commandType.value) {
    case 'install': return 'Install EasyTier Core';
    case 'restart': return 'Restart EasyTier Core';
    case 'uninstall': return 'Uninstall EasyTier Core';
    case 'stop': return 'Stop EasyTier Core';
    default: return 'Command';
  }
};

const getCommandButtonLabel = () => {
  switch (commandType.value) {
    case 'install': return 'Install';
    case 'restart': return 'Restart';
    case 'uninstall': return 'Uninstall';
    case 'stop': return 'Stop';
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
        payload = { version: installVersion.value || null };
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
