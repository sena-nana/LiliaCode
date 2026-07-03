package com.lilia.voice

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.lilia.remote.PairingUriParser
import com.lilia.remote.RemoteGateway
import com.lilia.remote.RemoteTaskDetail
import com.lilia.remote.RemoteTaskSummary
import com.lilia.remote.SavedPc
import com.lilia.remote.shouldClearActivePcForFailure
import com.lilia.remote.taskRunBlockInfo
import com.lilia.remote.taskStatusLabel
import com.lilia.remote.withRelatedTasks
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private var pairingIntentUri by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        pairingIntentUri = intent?.dataString
        setContent {
            LiliaVoiceApp(RemoteGateway(applicationContext), pairingIntentUri)
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        pairingIntentUri = intent.dataString
    }
}

@Composable
fun LiliaVoiceApp(
    gateway: RemoteGateway? = null,
    initialPairingUri: String? = null,
) {
    val runtime = remember { VoiceRuntime() }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var activePc by remember { mutableStateOf(gateway?.activePc()) }
    var voiceState by remember { mutableStateOf(VoiceRuntimeState()) }
    var pairingUri by remember { mutableStateOf(initialPairingUri.orEmpty()) }
    var pairingBusy by remember { mutableStateOf(false) }
    var loading by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    var lastTranscript by remember { mutableStateOf("") }
    var listening by remember { mutableStateOf(false) }
    var recordAudioGranted by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) ==
                PackageManager.PERMISSION_GRANTED,
        )
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        recordAudioGranted = granted
        listening = granted
        if (!granted) error = "Microphone permission is required for voice commands."
    }

    fun clearTrustedPcIfNeeded(err: Throwable): Boolean {
        if (!shouldClearActivePcForFailure(err)) return false
        gateway?.clearActivePc()
        activePc = null
        voiceState = VoiceRuntimeState(notice = "Pair this device again.")
        return true
    }

    fun loadInbox(pc: SavedPc) {
        val remoteGateway = gateway ?: return
        loading = true
        error = ""
        scope.launch {
            val accepted = remoteGateway.resume(pc).getOrElse {
                if (!clearTrustedPcIfNeeded(it)) error = it.message ?: "Remote session unavailable."
                loading = false
                return@launch
            }
            if (!accepted) {
                remoteGateway.clearActivePc()
                activePc = null
                voiceState = VoiceRuntimeState(notice = "Pair this device again.")
                loading = false
                return@launch
            }
            remoteGateway.listTasks(pc)
                .onSuccess { tasks ->
                    voiceState = runtime.withTasks(voiceState, tasks)
                }
                .onFailure {
                    if (!clearTrustedPcIfNeeded(it)) error = it.message ?: "Failed to load projects."
                }
            loading = false
        }
    }

    fun refreshSelectedTask() {
        val pc = activePc ?: return
        val detail = voiceState.selectedTask ?: return
        val remoteGateway = gateway ?: return
        loading = true
        error = ""
        scope.launch {
            remoteGateway.taskDetail(pc, detail.task.taskId)
                .onSuccess { updated ->
                    voiceState = runtime.withTaskDetail(
                        voiceState,
                        updated.withRelatedTasks(voiceState.tasks),
                    )
                }
                .onFailure {
                    if (!clearTrustedPcIfNeeded(it)) error = it.message ?: "Failed to refresh project."
                }
            loading = false
        }
    }

    fun openTask(taskId: String) {
        val pc = activePc ?: return
        val remoteGateway = gateway ?: return
        loading = true
        error = ""
        scope.launch {
            remoteGateway.taskDetail(pc, taskId)
                .onSuccess { detail ->
                    remoteGateway.setActiveTaskId(taskId)
                    voiceState = runtime.withTaskDetail(
                        voiceState,
                        detail.withRelatedTasks(voiceState.tasks),
                    )
                }
                .onFailure {
                    if (!clearTrustedPcIfNeeded(it)) error = it.message ?: "Failed to open project."
                }
            loading = false
        }
    }

    fun executeAction(action: VoiceAction) {
        val pc = activePc ?: return
        val detail = voiceState.selectedTask
        val remoteGateway = gateway ?: return
        when (action) {
            VoiceAction.None -> Unit
            VoiceAction.RefreshInbox -> {
                if (detail == null) loadInbox(pc) else refreshSelectedTask()
            }
            is VoiceAction.OpenTask -> openTask(action.taskId)
            is VoiceAction.StartProcess -> {
                if (detail == null) return
                loading = true
                error = ""
                scope.launch {
                    remoteGateway.startProcess(pc, detail, action.command)
                        .onSuccess { refreshSelectedTask() }
                        .onFailure { error = it.message ?: "Failed to start process." }
                    loading = false
                }
            }
            is VoiceAction.StopProcess -> {
                loading = true
                error = ""
                scope.launch {
                    remoteGateway.stopProcess(pc, action.taskId)
                        .onSuccess { refreshSelectedTask() }
                        .onFailure { error = it.message ?: "Failed to stop process." }
                    loading = false
                }
            }
        }
    }

    fun handleTranscript(text: String) {
        lastTranscript = text
        val result = runtime.reduce(voiceState, text)
        voiceState = result.state
        executeAction(result.action)
    }

    fun pair(uri: String) {
        val remoteGateway = gateway ?: return
        pairingBusy = true
        error = ""
        PairingUriParser.parse(uri)
            .onFailure {
                error = it.message ?: "Invalid pairing link."
                pairingBusy = false
            }
            .onSuccess { ticket ->
                scope.launch {
                    remoteGateway.pair(ticket)
                        .onSuccess { pc ->
                            activePc = pc
                            voiceState = VoiceRuntimeState()
                            loadInbox(pc)
                        }
                        .onFailure { error = it.message ?: "Pairing failed." }
                    pairingBusy = false
                }
            }
    }

    LaunchedEffect(initialPairingUri) {
        val uri = initialPairingUri?.takeIf { it.isNotBlank() } ?: return@LaunchedEffect
        pairingUri = uri
        pair(uri)
    }

    LaunchedEffect(activePc?.endpointId, activePc?.bridgeUrl) {
        activePc?.let { loadInbox(it) }
    }

    VoiceSpeechRecognizer(
        active = listening && recordAudioGranted,
        onTranscript = {
            listening = false
            handleTranscript(it)
        },
        onError = { message ->
            listening = false
            error = message
        },
    )

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
            if (activePc == null) {
                PairingScreen(
                    pairingUri = pairingUri,
                    busy = pairingBusy,
                    error = error,
                    onPairingUriChange = {
                        pairingUri = it
                        error = ""
                    },
                    onPair = { pair(pairingUri) },
                )
            } else {
                VoiceRuntimeScreen(
                    pc = activePc!!,
                    state = voiceState,
                    manifest = runtime.manifestFor(voiceState),
                    loading = loading,
                    error = error,
                    lastTranscript = lastTranscript,
                    listening = listening,
                    recordAudioGranted = recordAudioGranted,
                    onListen = {
                        if (recordAudioGranted) {
                            listening = true
                            error = ""
                        } else {
                            permissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
                        }
                    },
                    onRefresh = { loadInbox(activePc!!) },
                )
            }
        }
    }
}

@Composable
private fun VoiceSpeechRecognizer(
    active: Boolean,
    onTranscript: (String) -> Unit,
    onError: (String) -> Unit,
) {
    val context = LocalContext.current
    val recognizer = remember {
        if (SpeechRecognizer.isRecognitionAvailable(context)) {
            SpeechRecognizer.createSpeechRecognizer(context)
        } else {
            null
        }
    }
    DisposableEffect(recognizer) {
        if (recognizer == null) {
            onDispose {}
        } else {
            recognizer.setRecognitionListener(
                object : RecognitionListener {
                    override fun onReadyForSpeech(params: Bundle?) = Unit
                    override fun onBeginningOfSpeech() = Unit
                    override fun onRmsChanged(rmsdB: Float) = Unit
                    override fun onBufferReceived(buffer: ByteArray?) = Unit
                    override fun onEndOfSpeech() = Unit
                    override fun onPartialResults(partialResults: Bundle?) = Unit
                    override fun onEvent(eventType: Int, params: Bundle?) = Unit

                    override fun onError(error: Int) {
                        val message = when (error) {
                            SpeechRecognizer.ERROR_NO_MATCH -> "No command heard."
                            SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "Listening timed out."
                            else -> "Voice recognition failed."
                        }
                        onError(message)
                    }

                    override fun onResults(results: Bundle?) {
                        val text = results
                            ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                            ?.firstOrNull()
                            .orEmpty()
                        if (text.isBlank()) {
                            onError("No command heard.")
                        } else {
                            onTranscript(text)
                        }
                    }
                },
            )
            onDispose {
                recognizer.destroy()
            }
        }
    }
    LaunchedEffect(active, recognizer) {
        if (!active || recognizer == null) return@LaunchedEffect
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, false)
        }
        recognizer.startListening(intent)
    }
}

@Composable
private fun PairingScreen(
    pairingUri: String,
    busy: Boolean,
    error: String,
    onPairingUriChange: (String) -> Unit,
    onPair: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(20.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Text("LiliaVoice", color = MaterialTheme.colorScheme.onBackground, fontWeight = FontWeight.SemiBold)
            Text("Pair this device with a trusted PC to control Lilia projects by voice.", color = Muted)
        }
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedTextField(
                value = pairingUri,
                onValueChange = onPairingUriChange,
                modifier = Modifier.fillMaxWidth(),
                minLines = 3,
                label = { Text("Pairing link") },
                placeholder = { Text("lilia-remote://pair?...") },
            )
            if (error.isNotBlank()) {
                Text(error, color = Danger, style = MaterialTheme.typography.bodySmall)
            }
            Button(
                onClick = onPair,
                modifier = Modifier.fillMaxWidth(),
                enabled = !busy && pairingUri.isNotBlank(),
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(if (busy) "Pairing..." else "Pair PC")
            }
        }
    }
}

@Composable
private fun VoiceRuntimeScreen(
    pc: SavedPc,
    state: VoiceRuntimeState,
    manifest: VoicePageManifest,
    loading: Boolean,
    error: String,
    lastTranscript: String,
    listening: Boolean,
    recordAudioGranted: Boolean,
    onListen: () -> Unit,
    onRefresh: () -> Unit,
) {
    val detail = state.selectedTask
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Header(pc, manifest)
        if (error.isNotBlank()) {
            Text(error, color = Danger, style = MaterialTheme.typography.bodySmall)
        }
        if (state.notice.isNotBlank()) {
            Text(state.notice, color = MaterialTheme.colorScheme.secondary, style = MaterialTheme.typography.bodySmall)
        }
        if (lastTranscript.isNotBlank()) {
            Text("Heard: $lastTranscript", color = Muted, style = MaterialTheme.typography.bodySmall)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = onListen,
                enabled = !loading,
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(
                    when {
                        listening -> "Listening"
                        !recordAudioGranted -> "Enable voice"
                        else -> "Listen"
                    },
                )
            }
            OutlinedButton(onClick = onRefresh, enabled = !loading, shape = RoundedCornerShape(8.dp)) {
                Text(if (loading) "Refreshing" else "Refresh")
            }
        }
        FocusPanel(manifest)
        when (state.pageId) {
            VoicePageId.HUB -> HubContent(state.tasks)
            VoicePageId.PROJECT_SELECTION -> TaskListContent(state.tasks, state.selectedIndex)
            VoicePageId.SEARCH_DICTATION -> DictationContent("Say the project name or search text.")
            VoicePageId.SEARCH_RESULTS -> TaskListContent(state.searchResults, state.selectedIndex)
            VoicePageId.PROJECT_DETAIL -> detail?.let { ProjectDetailContent(it) }
            VoicePageId.LOG -> detail?.let { LogContent(it, state.logErrorsOnly, state.logPaused) }
            VoicePageId.START_COMMAND_DICTATION -> DictationContent("Say the command to start.")
            VoicePageId.CONFIRM -> ConfirmContent(state.pendingConfirmation?.message.orEmpty())
        }
    }
}

@Composable
private fun Header(
    pc: SavedPc,
    manifest: VoicePageManifest,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Column(modifier = Modifier.weight(1f)) {
                Text(manifest.title, color = MaterialTheme.colorScheme.onBackground, fontWeight = FontWeight.SemiBold)
                Text(pc.displayName, color = Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Text(manifest.focus.label, color = MaterialTheme.colorScheme.secondary)
        }
    }
}

@Composable
private fun FocusPanel(manifest: VoicePageManifest) {
    Panel {
        Text(manifest.focus.label, color = MaterialTheme.colorScheme.onSurface, fontWeight = FontWeight.Medium)
        manifest.commands.take(6).forEach { command ->
            Text(command.phrases.first(), color = Muted, style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
private fun HubContent(tasks: List<RemoteTaskSummary>) {
    val running = tasks.count { it.status == "running" }
    Panel {
        Text("${tasks.size} projects, $running running", color = MaterialTheme.colorScheme.onSurface)
        Text("Open, search, or review running projects.", color = Muted, style = MaterialTheme.typography.bodySmall)
    }
}

@Composable
private fun TaskListContent(tasks: List<RemoteTaskSummary>, selectedIndex: Int) {
    LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (tasks.isEmpty()) {
            item { Text("No projects available.", color = Muted) }
        }
        itemsIndexed(tasks) { index, task ->
            val selected = index == selectedIndex
            Panel(
                selected = selected,
            ) {
                Text("${index + 1}. ${task.title}", color = MaterialTheme.colorScheme.onSurface, fontWeight = FontWeight.Medium)
                Text(task.projectName ?: "Standalone", color = Muted, style = MaterialTheme.typography.bodySmall)
                Text(taskStatusLabel(task.status), color = if (task.status == "blocked") Danger else MaterialTheme.colorScheme.secondary)
            }
        }
    }
}

@Composable
private fun ProjectDetailContent(detail: RemoteTaskDetail) {
    val blockInfo = taskRunBlockInfo(detail.task, detail.relatedTasks)
    Panel {
        Text(detail.task.title, color = MaterialTheme.colorScheme.onSurface, fontWeight = FontWeight.SemiBold)
        Text("Status: ${taskStatusLabel(detail.task.status)}", color = Muted)
        Text("Runtime: ${detail.runtimePhase}", color = Muted)
        if (detail.processSessionId != null) {
            Text("Process running", color = MaterialTheme.colorScheme.primary)
        }
        if (blockInfo != null) {
            Text(blockInfo.reason, color = Danger, style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
private fun LogContent(detail: RemoteTaskDetail, errorsOnly: Boolean, paused: Boolean) {
    val timeline = if (errorsOnly) {
        detail.timeline.filter { item ->
            val status = item.status.lowercase()
            status.contains("fail") || status.contains("error") || status.contains("cancel")
        }
    } else {
        detail.timeline
    }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        item {
            Text(if (paused) "Paused" else "Live", color = MaterialTheme.colorScheme.secondary)
        }
        if (timeline.isEmpty()) {
            item { Text("No log events.", color = Muted) }
        }
        itemsIndexed(timeline) { _, item ->
            Panel {
                Text(item.summary, color = MaterialTheme.colorScheme.onSurface)
                Text(item.status, color = Muted, style = MaterialTheme.typography.bodySmall)
            }
        }
    }
}

@Composable
private fun DictationContent(text: String) {
    Panel {
        Text(text, color = MaterialTheme.colorScheme.onSurface)
    }
}

@Composable
private fun ConfirmContent(message: String) {
    Panel(selected = true) {
        Text(message, color = MaterialTheme.colorScheme.onSurface, fontWeight = FontWeight.SemiBold)
        Text("Say confirm or cancel.", color = Muted, style = MaterialTheme.typography.bodySmall)
    }
}

@Composable
private fun Panel(
    selected: Boolean = false,
    content: @Composable () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(if (selected) Color(0xFF26362F) else MaterialTheme.colorScheme.surface)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        content()
    }
}

private val Muted = Color(0xFFB8C2BC)
private val Danger = Color(0xFFE07A6F)

@Preview
@Composable
private fun LiliaVoiceAppPreview() {
    LiliaVoiceApp()
}
