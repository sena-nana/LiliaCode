package com.lilia.remote

import org.json.JSONObject

data class RemotePairingTicket(
    val protocolVersion: Int,
    val ticketId: String,
    val challenge: String,
    val endpointId: String,
    val pcName: String,
    val bridgeUrl: String,
    val rawUri: String,
)

data class SavedPc(
    val endpointId: String,
    val displayName: String,
    val protocolVersion: Int,
    val pairingUri: String,
    val bridgeUrl: String,
    val lastActiveAt: Long,
)

data class RemoteTaskSummary(
    val taskId: String,
    val title: String,
    val projectName: String?,
    val status: String,
    val lastActivity: String,
    val pendingAction: String?,
)

data class RemoteTimelineItem(
    val id: String,
    val title: String,
    val summary: String,
    val status: String,
)

data class PendingInteraction(
    val taskId: String,
    val requestId: String,
    val kind: String,
    val backend: String,
    val title: String,
    val body: String,
    val approveLabel: String,
    val declineLabel: String,
    val raw: JSONObject,
)

data class RemoteTaskDetail(
    val task: RemoteTaskSummary,
    val runtimePhase: String,
    val timeline: List<RemoteTimelineItem>,
    val pendingInteraction: PendingInteraction?,
)

data class RemoteBridgeStatus(
    val hostEnabled: Boolean,
    val state: String,
    val pcName: String,
    val capabilities: RemoteCapabilities = RemoteCapabilities(),
)

data class RemoteCapabilities(
    val supportsTaskInbox: Boolean = true,
    val supportsChatSend: Boolean = true,
    val supportsInteractionResponse: Boolean = true,
    val supportsInterrupt: Boolean = true,
)

data class RemoteProviderStatus(
    val backend: String,
    val ready: Boolean,
)
