<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  MonitorSmartphone,
  QrCode,
  RotateCw,
  ShieldCheck,
  Smartphone,
  Trash2,
} from "lucide-vue-next";
import { toDataURL } from "qrcode";
import type { RemoteControlStatus, RemotePairingTicket } from "@lilia/contracts";
import {
  cancelRemoteControlPairing,
  getRemoteControlStatus,
  revokeRemoteControlDevice,
  setRemoteControlHostEnabled,
  setRemoteControlPcName,
  startRemoteControlPairing,
} from "../../services/chat";

const status = ref<RemoteControlStatus | null>(null);
const pairingTicket = ref<RemotePairingTicket | null>(null);
const qrDataUrl = ref("");
const pcNameDraft = ref("");
const loading = ref(false);
const savingName = ref(false);
const pairing = ref(false);
const errorText = ref("");

const hostEnabled = computed(() => status.value?.hostEnabled ?? false);
const trustedDevices = computed(() => status.value?.trustedDevices ?? []);
const activeTicket = computed(() => pairingTicket.value ?? status.value?.activeTicket ?? null);
const connectionLabel = computed(() => {
  switch (status.value?.state) {
    case "pairing":
      return "等待扫码";
    case "listening":
      return "可连接";
    case "connected":
      return "已连接";
    case "disabled":
      return "未启用";
    default:
      return "未初始化";
  }
});

watch(
  () => activeTicket.value?.pairingUri,
  async (uri) => {
    qrDataUrl.value = "";
    if (!uri) return;
    qrDataUrl.value = await toDataURL(uri, {
      width: 220,
      margin: 1,
      color: {
        dark: "#18211c",
        light: "#f4f7f3",
      },
    });
  },
  { immediate: true },
);

async function refreshRemoteControl() {
  loading.value = true;
  errorText.value = "";
  try {
    status.value = await getRemoteControlStatus();
    pcNameDraft.value = status.value.pcName;
    pairingTicket.value = status.value.activeTicket;
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;
  }
}

async function toggleHost() {
  const next = !hostEnabled.value;
  loading.value = true;
  errorText.value = "";
  try {
    status.value = await setRemoteControlHostEnabled(next);
    pairingTicket.value = status.value.activeTicket;
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;
  }
}

async function savePcName() {
  savingName.value = true;
  errorText.value = "";
  try {
    status.value = await setRemoteControlPcName(pcNameDraft.value);
    pcNameDraft.value = status.value.pcName;
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
  } finally {
    savingName.value = false;
  }
}

async function beginPairing() {
  pairing.value = true;
  errorText.value = "";
  try {
    pairingTicket.value = await startRemoteControlPairing();
    status.value = await getRemoteControlStatus();
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
  } finally {
    pairing.value = false;
  }
}

async function cancelPairing() {
  pairing.value = true;
  errorText.value = "";
  try {
    await cancelRemoteControlPairing();
    pairingTicket.value = null;
    status.value = await getRemoteControlStatus();
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
  } finally {
    pairing.value = false;
  }
}

async function revokeDevice(deviceId: string) {
  loading.value = true;
  errorText.value = "";
  try {
    status.value = await revokeRemoteControlDevice(deviceId);
  } catch (err) {
    errorText.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;
  }
}

function formatTime(value: number | null | undefined): string {
  if (!value) return "从未连接";
  return new Date(value).toLocaleString();
}

onMounted(refreshRemoteControl);
</script>

<template>
  <div class="card remote-control-card">
    <h2>
      <span class="card-h2__title">
        <MonitorSmartphone :size="14" aria-hidden="true" />
        Android 远控
      </span>
    </h2>

    <div class="settings-row">
      <div class="settings-row__label">主机</div>
      <div class="settings-row__control">
        <button
          type="button"
          class="ui-button"
          :class="{ 'ui-button--ghost': hostEnabled }"
          :disabled="loading"
          @click="toggleHost"
        >
          <ShieldCheck :size="12" aria-hidden="true" />
          {{ hostEnabled ? "关闭远控" : "启用远控" }}
        </button>
        <span class="settings-row__status-text muted">{{ connectionLabel }}</span>
        <button type="button" class="ui-button ui-button--ghost" :disabled="loading" @click="refreshRemoteControl">
          <RotateCw :size="11" aria-hidden="true" />
          刷新
        </button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">PC 名称</div>
      <div class="settings-row__control">
        <input v-model="pcNameDraft" type="text" class="ui-input" placeholder="Lilia PC" />
        <button
          type="button"
          class="ui-button ui-button--ghost"
          :disabled="savingName"
          aria-label="保存 PC 名称"
          @click="savePcName"
        >
          保存
        </button>
      </div>
    </div>

    <div class="settings-row settings-row--stacked">
      <div class="settings-row__label">扫码配对</div>
      <div class="remote-pairing">
        <div class="remote-pairing__qr">
          <img v-if="qrDataUrl" :src="qrDataUrl" alt="Lilia Android pairing QR code" />
          <QrCode v-else :size="72" aria-hidden="true" />
        </div>
        <div class="remote-pairing__body">
          <div class="remote-pairing__title">
            {{ activeTicket ? "Android 扫码后将加入可信设备" : "生成一次性二维码" }}
          </div>
          <div class="remote-pairing__meta muted">
            <template v-if="activeTicket">
              过期时间：{{ formatTime(activeTicket.expiresAt) }}
            </template>
            <template v-else>
              二维码只用于建立 trusted device，后续重连不需要重新扫码。
            </template>
          </div>
          <div class="remote-pairing__actions">
            <button type="button" class="ui-button ui-button--ghost" :disabled="pairing" @click="beginPairing">
              <QrCode :size="12" aria-hidden="true" />
              {{ activeTicket ? "重新生成" : "生成二维码" }}
            </button>
            <button
              v-if="activeTicket"
              type="button"
              class="ui-button ui-button--ghost"
              :disabled="pairing"
              @click="cancelPairing"
            >
              取消
            </button>
          </div>
          <code v-if="activeTicket" class="remote-pairing__uri">{{ activeTicket.pairingUri }}</code>
        </div>
      </div>
    </div>

    <div class="settings-row settings-row--stacked">
      <div class="settings-row__label">可信设备</div>
      <div v-if="trustedDevices.length === 0" class="settings-row__status muted">
        还没有配对的 Android 设备。
      </div>
      <div v-else class="remote-devices">
        <div v-for="device in trustedDevices" :key="device.id" class="remote-device">
          <div class="remote-device__main">
            <Smartphone :size="14" aria-hidden="true" />
            <div>
              <div class="remote-device__name">{{ device.displayName }}</div>
              <div class="remote-device__meta muted">
                {{ device.trusted ? "可信" : "已撤销" }} · 最近连接 {{ formatTime(device.lastSeenAt) }}
              </div>
            </div>
          </div>
          <button
            type="button"
            class="ui-button ui-button--ghost"
            :disabled="!device.trusted || loading"
            title="撤销设备"
            @click="revokeDevice(device.id)"
          >
            <Trash2 :size="12" aria-hidden="true" />
            撤销
          </button>
        </div>
      </div>
    </div>

    <div v-if="errorText" class="settings-row__status remote-error">
      {{ errorText }}
    </div>
  </div>
</template>

<style scoped>
.remote-control-card {
  margin-top: 14px;
}

.remote-pairing {
  display: grid;
  grid-template-columns: 148px minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}

.remote-pairing__qr {
  display: grid;
  min-height: 148px;
  place-items: center;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--panel) 82%, white 18%);
  color: var(--muted);
}

.remote-pairing__qr img {
  width: 132px;
  height: 132px;
  border-radius: 4px;
}

.remote-pairing__body {
  display: grid;
  min-width: 0;
  gap: 8px;
}

.remote-pairing__title,
.remote-device__name {
  color: var(--text);
  font-weight: 600;
}

.remote-pairing__actions,
.remote-device,
.remote-device__main {
  display: flex;
  align-items: center;
  gap: 10px;
}

.remote-pairing__uri {
  display: block;
  max-width: 100%;
  overflow: hidden;
  padding: 7px 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remote-devices {
  display: grid;
  gap: 8px;
}

.remote-device {
  justify-content: space-between;
  padding: 9px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
}

.remote-device__main {
  min-width: 0;
}

.remote-device__meta {
  font-size: 12px;
}

.remote-error {
  color: var(--danger);
}

@media (max-width: 680px) {
  .remote-pairing {
    grid-template-columns: 1fr;
  }

  .remote-pairing__qr {
    min-height: 180px;
  }
}
</style>
