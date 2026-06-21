package com.lilia.remote

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private var pairingIntentUri by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val repository = RemoteRepository(applicationContext)
        pairingIntentUri = intent?.dataString
        setContent {
            LiliaRemoteApp(repository, pairingIntentUri)
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        pairingIntentUri = intent.dataString
    }
}

@Composable
fun LiliaRemoteApp(repository: RemoteRepository? = null, initialPairingUri: String? = null) {
    val fallbackRepository = remember { repository }
    val client = remember(fallbackRepository) { fallbackRepository?.let { RemoteHttpClient(it) } }
    val scope = rememberCoroutineScope()
    val initialPc = remember { fallbackRepository?.activePc() }
    var activePc by remember { mutableStateOf(initialPc) }
    var selectedTask by remember { mutableStateOf<RemoteTaskDetail?>(null) }
    var inboxTasks by remember { mutableStateOf<List<RemoteTaskSummary>>(emptyList()) }
    var inboxLoading by remember { mutableStateOf(false) }
    var detailLoading by remember { mutableStateOf(false) }
    var remoteError by remember { mutableStateOf("") }
    var bridgeTone by remember { mutableStateOf("Listening") }
    var bridgeStatus by remember { mutableStateOf<RemoteBridgeStatus?>(null) }
    var providerStatus by remember { mutableStateOf<RemoteProviderStatus?>(null) }
    var pairingBusy by remember { mutableStateOf(false) }
    var pairingError by remember { mutableStateOf("") }

    fun acceptPairedPc(pc: SavedPc) {
        activePc = pc
        selectedTask = null
        inboxTasks = emptyList()
        pairingError = ""
    }

    fun pairWithTicket(ticket: RemotePairingTicket) {
        val remoteClient = client
        pairingBusy = true
        pairingError = ""
        remoteError = ""
        if (remoteClient == null) {
            acceptPairedPc(
                SavedPc(
                    endpointId = ticket.endpointId,
                    displayName = ticket.pcName,
                    protocolVersion = ticket.protocolVersion,
                    pairingUri = ticket.rawUri,
                    bridgeUrl = ticket.bridgeUrl,
                    lastActiveAt = System.currentTimeMillis(),
                ),
            )
            pairingBusy = false
            return
        }
        scope.launch {
            remoteClient.pair(ticket)
                .onSuccess { acceptPairedPc(it) }
                .onFailure {
                    val message = it.message ?: "Pairing failed"
                    pairingError = message
                    remoteError = message
                }
            pairingBusy = false
        }
    }

    fun clearUntrustedPc(pc: SavedPc?, message: String) {
        if (pc != null && shouldIgnorePcResult(activePc, pc)) {
            return
        }
        fallbackRepository?.clearActivePc()
        activePc = null
        selectedTask = null
        inboxTasks = emptyList()
        bridgeStatus = null
        providerStatus = null
        bridgeTone = "Pair again"
        remoteError = message
        pairingError = message
    }

    fun handleRemoteFailure(err: Throwable, fallback: String, pc: SavedPc? = null) {
        if (pc != null && shouldIgnorePcResult(activePc, pc)) {
            return
        }
        if (err is RemoteBridgeException && err.code == "unauthorized") {
            clearUntrustedPc(pc, "This PC no longer trusts this phone. Pair again.")
            return
        }
        remoteError = err.message ?: fallback
    }

    suspend fun refreshProviderStatus(pc: SavedPc, remoteClient: RemoteHttpClient) {
        remoteClient.providerStatus(pc)
            .onSuccess {
                if (shouldIgnorePcResult(activePc, pc)) return@onSuccess
                providerStatus = it
            }
            .onFailure {
                if (shouldIgnorePcResult(activePc, pc)) return@onFailure
                providerStatus = null
            }
    }

    suspend fun loadInbox(pc: SavedPc, remoteClient: RemoteHttpClient) {
        val accepted = remoteClient.resume(pc).getOrElse {
            if (shouldIgnorePcResult(activePc, pc)) return
            bridgeTone = "Offline"
            handleRemoteFailure(it, "Failed to resume remote session", pc)
            return
        }
        if (shouldIgnorePcResult(activePc, pc)) return
        if (!accepted) {
            clearUntrustedPc(pc, "This PC no longer trusts this phone. Pair again.")
            return
        }
        val status = remoteClient.bridgeStatus(pc)
            .onSuccess {
                if (shouldIgnorePcResult(activePc, pc)) return@onSuccess
                bridgeStatus = it
                bridgeTone = bridgeToneLabel(it)
            }
            .onFailure {
                if (shouldIgnorePcResult(activePc, pc)) return@onFailure
                bridgeStatus = null
                bridgeTone = "Offline"
            }
            .getOrNull()
        if (shouldIgnorePcResult(activePc, pc)) return
        refreshProviderStatus(pc, remoteClient)
        if (shouldIgnorePcResult(activePc, pc)) return
        if (status?.capabilities?.supportsTaskInbox == false) {
            inboxTasks = emptyList()
            remoteError = taskInboxUnsupportedMessage()
            return
        }
        remoteClient.listTasks(pc)
            .onSuccess {
                if (shouldIgnorePcResult(activePc, pc)) return@onSuccess
                inboxTasks = it
            }
            .onFailure { handleRemoteFailure(it, "Failed to load tasks", pc) }
    }

    fun openTask(pc: SavedPc, taskId: String) {
        val remoteClient = client ?: return
        detailLoading = true
        remoteError = ""
        scope.launch {
            refreshProviderStatus(pc, remoteClient)
            remoteClient.taskDetail(pc, taskId)
                .onSuccess {
                    if (shouldIgnorePcResult(activePc, pc)) return@onSuccess
                    selectedTask = it
                }
                .onFailure { handleRemoteFailure(it, "Failed to load task", pc) }
            if (!shouldIgnorePcResult(activePc, pc)) {
                detailLoading = false
            }
        }
    }

    fun runTaskAction(
        pc: SavedPc,
        taskId: String,
        failureMessage: String,
        clearError: Boolean = true,
        action: suspend (RemoteHttpClient) -> Result<Unit>,
    ) {
        val remoteClient = client ?: return
        detailLoading = true
        if (clearError) remoteError = ""
        scope.launch {
            val result = action(remoteClient)
            result.onFailure { handleRemoteFailure(it, failureMessage, pc) }
            if (result.isSuccess) {
                remoteClient.taskDetail(pc, taskId)
                    .onSuccess {
                        if (shouldIgnorePcResult(activePc, pc)) return@onSuccess
                        selectedTask = it
                    }
                    .onFailure { handleRemoteFailure(it, "Failed to load task", pc) }
            }
            if (!shouldIgnorePcResult(activePc, pc)) {
                detailLoading = false
            }
        }
    }

    fun refreshInbox(pc: SavedPc) {
        val remoteClient = client ?: return
        inboxLoading = true
        remoteError = ""
        scope.launch {
            loadInbox(pc, remoteClient)
            inboxLoading = false
        }
    }

    LaunchedEffect(initialPairingUri) {
        val uri = initialPairingUri?.takeIf { it.isNotBlank() } ?: return@LaunchedEffect
        PairingUriParser.parse(uri)
            .onSuccess { pairWithTicket(it) }
            .onFailure {
                val message = it.message ?: "Invalid pairing URI"
                pairingError = message
                remoteError = message
            }
    }

    LaunchedEffect(activePc?.endpointId, activePc?.bridgeUrl) {
        val pc = activePc ?: return@LaunchedEffect
        val remoteClient = client ?: return@LaunchedEffect
        inboxLoading = true
        remoteError = ""
        loadInbox(pc, remoteClient)
        inboxLoading = false
    }

    MaterialTheme(
        colorScheme = MaterialTheme.colorScheme.copy(
            background = Color(0xFF151716),
            surface = Color(0xFF202321),
            primary = Color(0xFF74C7A4),
            secondary = Color(0xFFE3B35B),
            onBackground = Color(0xFFE7ECE8),
            onSurface = Color(0xFFE7ECE8),
            onPrimary = Color(0xFF10221A),
        ),
    ) {
        Surface(
            modifier = Modifier.fillMaxSize(),
            color = MaterialTheme.colorScheme.background,
        ) {
            when {
                activePc == null -> PairingScreen(
                    initialPairingUri = initialPairingUri,
                    busy = pairingBusy,
                    externalError = pairingError,
                    onPair = { ticket ->
                        pairWithTicket(ticket)
                    },
                )
                selectedTask != null -> TaskDetailScreen(
                    pc = activePc!!,
                    detail = selectedTask!!,
                    capabilities = bridgeStatus?.capabilities ?: RemoteCapabilities(),
                    providerStatus = providerStatus,
                    loading = detailLoading,
                    error = remoteError,
                    onBack = { selectedTask = null },
                    onRefresh = { openTask(activePc!!, selectedTask!!.task.taskId) },
                    onInterrupt = {
                        val pc = activePc!!
                        val taskId = selectedTask!!.task.taskId
                        runTaskAction(pc, taskId, "Interrupt failed", clearError = false) { remoteClient ->
                            remoteClient.interrupt(pc, taskId)
                        }
                    },
                    onResolveInteraction = { interaction, approve, responseText, selectedOptions ->
                        val pc = activePc!!
                        val taskId = selectedTask!!.task.taskId
                        runTaskAction(pc, taskId, "Interaction response failed") { remoteClient ->
                            remoteClient.resolveInteraction(
                                pc,
                                interaction,
                                approve,
                                responseText,
                                selectedOptions,
                            )
                        }
                    },
                    onSend = { message ->
                        val pc = activePc!!
                        val taskId = selectedTask!!.task.taskId
                        runTaskAction(pc, taskId, "Send failed") { remoteClient ->
                            remoteClient.sendMessage(pc, taskId, message)
                        }
                    },
                )
                else -> InboxScreen(
                    pc = activePc!!,
                    tasks = inboxTasks,
                    loading = inboxLoading,
                    error = remoteError,
                    bridgeTone = bridgeTone,
                    onRefresh = { refreshInbox(activePc!!) },
                    onOpenTask = { openTask(activePc!!, it.taskId) },
                    onForgetPc = {
                        fallbackRepository?.clearActivePc()
                        activePc = null
                        bridgeStatus = null
                        providerStatus = null
                    },
                )
            }
        }
    }
}

internal fun savedPcConnectionMatches(current: SavedPc?, candidate: SavedPc): Boolean =
    current != null && activePcConnectionMatches(current, candidate)

internal fun shouldIgnorePcResult(current: SavedPc?, candidate: SavedPc): Boolean =
    current == null || !savedPcConnectionMatches(current, candidate)

@Composable
private fun PairingScreen(
    initialPairingUri: String?,
    busy: Boolean,
    externalError: String,
    onPair: (RemotePairingTicket) -> Unit,
) {
    var pairingUri by remember { mutableStateOf(initialPairingUri.orEmpty()) }
    var error by remember { mutableStateOf("") }
    var scannerVisible by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (granted) {
            scannerVisible = true
            error = ""
        } else {
            error = "Camera permission is required to scan the pairing QR code."
        }
    }

    LaunchedEffect(initialPairingUri) {
        val uri = initialPairingUri?.takeIf { it.isNotBlank() } ?: return@LaunchedEffect
        pairingUri = uri
        error = ""
    }

    Box(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 20.dp, vertical = 18.dp),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
                RemoteHeader(title = "Active PC", subtitle = "Not connected", tone = "Ready")
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = "Lilia Remote",
                        style = MaterialTheme.typography.headlineMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onBackground,
                    )
                    Text(
                        text = "Pair this phone with a trusted PC to review tasks, continue chats, and handle approvals.",
                        style = MaterialTheme.typography.bodyLarge,
                        color = Muted,
                    )
                }
                StatusPanel(
                    rows = listOf(
                        "Android shell" to "Compose remote client",
                        "Transport" to "LAN HTTP bridge",
                        "Remote protocol" to "Awaiting pairing",
                    ),
                )
            }

            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = pairingUri,
                    onValueChange = {
                        pairingUri = it
                        error = ""
                    },
                    modifier = Modifier.fillMaxWidth(),
                    minLines = 3,
                    label = { Text("Pairing URI") },
                    placeholder = { Text("lilia-remote://pair?...") },
                )
                val displayError = error.ifBlank { externalError }
                if (displayError.isNotBlank()) {
                    Text(displayError, color = Danger, style = MaterialTheme.typography.bodySmall)
                }
                OutlinedButton(
                    onClick = {
                        val granted = ContextCompat.checkSelfPermission(
                            context,
                            Manifest.permission.CAMERA,
                        ) == PackageManager.PERMISSION_GRANTED
                        if (granted) {
                            scannerVisible = true
                        } else {
                            cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !busy,
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text("Scan QR")
                }
                Button(
                    onClick = {
                        PairingUriParser.parse(pairingUri)
                            .onSuccess { onPair(it) }
                            .onFailure { error = it.message ?: "Invalid pairing URI" }
                    },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !busy,
                    shape = RoundedCornerShape(8.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.primary,
                        contentColor = MaterialTheme.colorScheme.onPrimary,
                    ),
                ) {
                    Text(if (busy) "Pairing..." else "Pair PC")
                }
            }
        }

        if (scannerVisible) {
            PairingQrScanner(
                onPairingUri = { uri ->
                    scannerVisible = false
                    pairingUri = uri
                    PairingUriParser.parse(uri)
                        .onSuccess { onPair(it) }
                        .onFailure { error = it.message ?: "Invalid pairing QR code" }
                },
                onClose = { scannerVisible = false },
                onError = { error = it },
            )
        }
    }
}

@Composable
private fun InboxScreen(
    pc: SavedPc,
    tasks: List<RemoteTaskSummary>,
    loading: Boolean,
    error: String,
    bridgeTone: String,
    onRefresh: () -> Unit,
    onOpenTask: (RemoteTaskSummary) -> Unit,
    onForgetPc: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 16.dp, vertical = 18.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        RemoteHeader(title = "Active PC", subtitle = pc.displayName, tone = bridgeTone)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onForgetPc, shape = RoundedCornerShape(8.dp)) {
                Text("Forget PC")
            }
            OutlinedButton(onClick = onRefresh, shape = RoundedCornerShape(8.dp)) {
                Text(if (loading) "Refreshing" else "Refresh")
            }
        }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            item {
                Text(
                    text = "Task Inbox",
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onBackground,
                )
            }
            if (error.isNotBlank()) {
                item { Text(error, color = Danger, style = MaterialTheme.typography.bodySmall) }
            }
            if (loading && tasks.isEmpty()) {
                item { Text("Loading tasks...", color = Muted, style = MaterialTheme.typography.bodyMedium) }
            }
            if (!loading && tasks.isEmpty() && error.isBlank()) {
                item { Text("No tasks from the active PC.", color = Muted, style = MaterialTheme.typography.bodyMedium) }
            }
            items(tasks, key = { it.taskId }) { task ->
                TaskCard(task = task, onClick = { onOpenTask(task) })
            }
        }
    }
}

@Composable
private fun TaskDetailScreen(
    pc: SavedPc,
    detail: RemoteTaskDetail,
    capabilities: RemoteCapabilities,
    providerStatus: RemoteProviderStatus?,
    loading: Boolean,
    error: String,
    onBack: () -> Unit,
    onRefresh: () -> Unit,
    onInterrupt: () -> Unit,
    onResolveInteraction: (PendingInteraction, Boolean, String, Map<String, List<String>>) -> Unit,
    onSend: (String) -> Unit,
) {
    var draft by remember(detail.task.taskId) { mutableStateOf("") }
    val pendingInteraction = detail.pendingInteraction
    val providerReady = providerStatus?.ready != false
    var interactionResponse by remember(pendingInteraction?.requestId) { mutableStateOf("") }
    var selectedInteractionOptions by remember(pendingInteraction?.requestId) { mutableStateOf<Set<String>>(emptySet()) }
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 16.dp, vertical = 18.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        RemoteHeader(title = pc.displayName, subtitle = detail.task.title, tone = detail.runtimePhase)
        OutlinedButton(onClick = onBack, shape = RoundedCornerShape(8.dp)) {
            Text("Back")
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onRefresh, shape = RoundedCornerShape(8.dp)) {
                Text(if (loading) "Refreshing" else "Refresh")
            }
            OutlinedButton(
                onClick = onInterrupt,
                enabled = !loading && capabilities.supportsInterrupt,
                shape = RoundedCornerShape(8.dp),
            ) {
                Text("Interrupt")
            }
        }
        providerStatus?.let {
            Text(
                text = "Provider ${it.backend}: ${if (it.ready) "ready" else "not ready"}",
                color = if (it.ready) MaterialTheme.colorScheme.primary else Danger,
                style = MaterialTheme.typography.bodySmall,
            )
        }
        if (error.isNotBlank()) {
            Text(error, color = Danger, style = MaterialTheme.typography.bodySmall)
        }
        pendingInteraction?.let { interaction ->
            val optionQuestion = interactionOptionQuestion(interaction)
            val canResolveInteraction = interactionCanSubmit(
                optionQuestion,
                selectedInteractionOptions,
                interactionResponse,
            )
            Panel {
                Text(interaction.title, color = MaterialTheme.colorScheme.onSurface, fontWeight = FontWeight.SemiBold)
                Text(interaction.body, color = Muted, style = MaterialTheme.typography.bodyMedium)
                if (optionQuestion != null) {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        optionQuestion.options.forEach { option ->
                            val selected = selectedInteractionOptions.contains(option.id)
                            val onClick = {
                                selectedInteractionOptions = if (optionQuestion.mode == "multi") {
                                    if (selected) {
                                        selectedInteractionOptions - option.id
                                    } else if (
                                        optionQuestion.maxSelections != null &&
                                        selectedInteractionOptions.size >= optionQuestion.maxSelections
                                    ) {
                                        selectedInteractionOptions
                                    } else {
                                        selectedInteractionOptions + option.id
                                    }
                                } else {
                                    setOf(option.id)
                                }
                            }
                            if (selected) {
                                Button(onClick = onClick, enabled = !loading, shape = RoundedCornerShape(8.dp)) {
                                    Text(option.label)
                                }
                            } else {
                                OutlinedButton(onClick = onClick, enabled = !loading, shape = RoundedCornerShape(8.dp)) {
                                    Text(option.label)
                                }
                            }
                            option.description?.let {
                                Text(it, color = Muted, style = MaterialTheme.typography.bodySmall)
                            }
                        }
                    }
                }
                if (interactionAcceptsTextResponse(interaction)) {
                    OutlinedTextField(
                        value = interactionResponse,
                        onValueChange = { interactionResponse = it },
                        modifier = Modifier.fillMaxWidth(),
                        minLines = 2,
                        label = { Text("Response") },
                    )
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(
                        onClick = {
                            val response = interactionResponse.trim()
                            val selectedOptions = selectedOptionsByQuestion(optionQuestion, selectedInteractionOptions)
                            interactionResponse = ""
                            selectedInteractionOptions = emptySet()
                            onResolveInteraction(interaction, true, response, selectedOptions)
                        },
                        enabled = !loading && capabilities.supportsInteractionResponse && canResolveInteraction,
                        shape = RoundedCornerShape(8.dp),
                    ) {
                        Text(interactionPrimaryActionLabel(interaction, interactionResponse))
                    }
                    OutlinedButton(
                        onClick = {
                            interactionResponse = ""
                            selectedInteractionOptions = emptySet()
                            onResolveInteraction(interaction, false, "", emptyMap())
                        },
                        enabled = !loading && capabilities.supportsInteractionResponse,
                        shape = RoundedCornerShape(8.dp),
                    ) {
                        Text(interaction.declineLabel)
                    }
                }
            }
        }
        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(detail.timeline, key = { it.id }) { item ->
                TimelineRow(item)
            }
        }
        OutlinedTextField(
            value = draft,
            onValueChange = { draft = it },
            modifier = Modifier.fillMaxWidth(),
            label = { Text("Message") },
            placeholder = { Text("Send through the PC runner") },
        )
        Button(
            onClick = {
                val message = draft.trim()
                if (message.isNotEmpty()) {
                    draft = ""
                    onSend(message)
                }
            },
            modifier = Modifier.fillMaxWidth(),
            enabled = !loading && providerReady && capabilities.supportsChatSend && draft.trim().isNotEmpty(),
            shape = RoundedCornerShape(8.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = MaterialTheme.colorScheme.primary,
                contentColor = MaterialTheme.colorScheme.onPrimary,
            ),
        ) {
            Text(
                when {
                    loading -> "Working..."
                    !providerReady -> "Provider not ready"
                    !capabilities.supportsChatSend -> "Send unsupported"
                    else -> "Send"
                },
            )
        }
    }
}

@Composable
private fun RemoteHeader(title: String, subtitle: String, tone: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column {
            Text(title, style = MaterialTheme.typography.labelMedium, color = Muted)
            Text(
                subtitle,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onBackground,
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .size(8.dp)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.secondary),
            )
            Text(
                text = tone,
                modifier = Modifier.padding(start = 8.dp),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.secondary,
            )
        }
    }
}

@Composable
private fun StatusPanel(rows: List<Pair<String, String>>) {
    Panel {
        rows.forEach { row ->
            StatusRow(label = row.first, value = row.second)
        }
    }
}

@Composable
private fun Panel(content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surface)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
        content = content,
    )
}

@Composable
private fun StatusRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, color = Color(0xFF9BA49E), style = MaterialTheme.typography.bodyMedium)
        Text(value, color = MaterialTheme.colorScheme.onSurface, style = MaterialTheme.typography.bodyMedium)
    }
}

@Composable
private fun TaskCard(task: RemoteTaskSummary, onClick: () -> Unit) {
    Panel {
        Column(
            modifier = Modifier.clickable(onClick = onClick),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.Top,
            ) {
                Text(task.title, color = MaterialTheme.colorScheme.onSurface, fontWeight = FontWeight.SemiBold)
                Text(task.status, color = MaterialTheme.colorScheme.secondary, style = MaterialTheme.typography.labelMedium)
            }
            Text(task.projectName ?: "Standalone chat", color = Muted, style = MaterialTheme.typography.bodySmall)
            task.pendingAction?.let {
                Text(it, color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodySmall)
            }
            Text(task.lastActivity, color = Color(0xFF87918B), style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
private fun TimelineRow(item: RemoteTimelineItem) {
    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(item.title, color = MaterialTheme.colorScheme.onSurface, fontWeight = FontWeight.Medium)
            Text(item.status, color = Muted, style = MaterialTheme.typography.labelMedium)
        }
        Text(item.summary, color = Muted, style = MaterialTheme.typography.bodyMedium)
    }
}

private val Muted = Color(0xFFB8C2BC)
private val Danger = Color(0xFFE07A6F)

private fun bridgeToneLabel(status: RemoteBridgeStatus): String {
    if (!status.hostEnabled) return "Disabled"
    return when (status.state) {
        "pairing" -> "Pairing"
        "listening", "connected" -> "Listening"
        "disconnected" -> "Offline"
        else -> status.state.replaceFirstChar { it.uppercase() }
    }
}

private fun interactionAcceptsTextResponse(interaction: PendingInteraction): Boolean {
    if (interaction.kind != "ask_user" && interaction.kind != "plan_approval") return false
    val questions = interaction.raw.optJSONObject("payload")?.optJSONArray("questions") ?: return false
    for (index in 0 until questions.length()) {
        val question = questions.optJSONObject(index) ?: continue
        if (question.optString("mode") == "confirm") return true
        if (question.optBoolean("allowOther")) return true
        val options = question.optJSONArray("options")
        if (options == null || options.length() == 0) return true
    }
    return false
}

private fun interactionPrimaryActionLabel(interaction: PendingInteraction, responseText: String): String {
    if (responseText.trim().isEmpty()) return interaction.approveLabel
    val question = interaction.raw
        .optJSONObject("payload")
        ?.optJSONArray("questions")
        ?.optJSONObject(0)
        ?: return interaction.approveLabel
    if (question.optString("mode") != "confirm") return interaction.approveLabel
    return if (interaction.kind == "plan_approval") "Send revision" else "Send response"
}

internal data class InteractionOptionQuestion(
    val questionId: String,
    val mode: String,
    val minSelections: Int,
    val maxSelections: Int?,
    val options: List<InteractionOption>,
)

internal data class InteractionOption(
    val id: String,
    val label: String,
    val description: String?,
)

internal fun interactionOptionQuestion(interaction: PendingInteraction): InteractionOptionQuestion? {
    if (interaction.kind != "ask_user" && interaction.kind != "plan_approval") return null
    val question = interaction.raw
        .optJSONObject("payload")
        ?.optJSONArray("questions")
        ?.optJSONObject(0)
        ?: return null
    val options = question.optJSONArray("options") ?: return null
    if (options.length() == 0) return null
    val mode = question.optString("mode").ifBlank { "single" }
    if (mode != "single" && mode != "multi") return null
    val parsed = buildList {
        for (index in 0 until options.length()) {
            val option = options.optJSONObject(index) ?: continue
            val label = option.optString("label")
                .ifBlank { option.optString("id") }
                .ifBlank { "Option ${index + 1}" }
            val id = option.optString("id")
                .ifBlank { label }
            add(
                InteractionOption(
                    id = id,
                    label = label,
                    description = option.optString("description").takeIf { it.isNotBlank() },
                ),
            )
        }
        if (question.optBoolean("allowOther")) {
            add(InteractionOption(id = "other", label = "Other", description = null))
        }
    }
    if (parsed.isEmpty()) return null
    val questionId = question.optString("id").ifBlank { "question-0" }
    val minSelections = if (mode == "multi") {
        question.optInt("minSelections", 1).coerceAtLeast(0)
    } else {
        1
    }
    val maxSelections = if (mode == "multi" && question.has("maxSelections")) {
        question.optInt("maxSelections", parsed.size).coerceAtLeast(minSelections)
    } else {
        null
    }
    return InteractionOptionQuestion(
        questionId = questionId,
        mode = mode,
        minSelections = minSelections,
        maxSelections = maxSelections,
        options = parsed,
    )
}

internal fun selectedOptionsByQuestion(
    question: InteractionOptionQuestion?,
    selectedOptions: Set<String>,
): Map<String, List<String>> {
    if (question == null || selectedOptions.isEmpty()) return emptyMap()
    val ordered = question.options
        .map { it.id }
        .filter { selectedOptions.contains(it) }
    if (ordered.isEmpty()) return emptyMap()
    return mapOf(question.questionId to ordered)
}

internal fun interactionCanSubmit(
    question: InteractionOptionQuestion?,
    selectedOptions: Set<String>,
    responseText: String,
): Boolean {
    if (question == null) return true
    if (selectedOptions.size < question.minSelections) return false
    if (question.maxSelections != null && selectedOptions.size > question.maxSelections) return false
    if (selectedOptions.contains("other") && responseText.trim().isEmpty()) return false
    return true
}

internal fun taskInboxUnsupportedMessage(): String =
    "Task inbox is not supported by this PC bridge."

@Preview
@Composable
private fun LiliaRemoteAppPreview() {
    LiliaRemoteApp()
}
