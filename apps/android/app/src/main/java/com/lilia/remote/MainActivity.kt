package com.lilia.remote

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            LiliaRemoteApp()
        }
    }
}

@Composable
fun LiliaRemoteApp() {
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
            RemoteInboxEmptyState()
        }
    }
}

@Composable
private fun RemoteInboxEmptyState() {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .statusBarsPadding()
            .padding(horizontal = 20.dp, vertical = 18.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        Column {
            RemoteHeader()
            Spacer(modifier = Modifier.height(28.dp))
            Text(
                text = "Lilia Remote",
                style = MaterialTheme.typography.headlineMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onBackground,
            )
            Spacer(modifier = Modifier.height(10.dp))
            Text(
                text = "Connect this phone to a trusted PC to review tasks, continue chats, and handle pending approvals.",
                style = MaterialTheme.typography.bodyLarge,
                color = Color(0xFFB8C2BC),
            )
            Spacer(modifier = Modifier.height(28.dp))
            StatusPanel()
        }

        Column {
            Button(
                onClick = {},
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(8.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary,
                ),
            ) {
                Text("Pair PC")
            }
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = "Pairing and live task sync will be wired after the remote protocol lands.",
                style = MaterialTheme.typography.bodySmall,
                color = Color(0xFF87918B),
            )
        }
    }
}

@Composable
private fun RemoteHeader() {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column {
            Text(
                text = "Active PC",
                style = MaterialTheme.typography.labelMedium,
                color = Color(0xFF87918B),
            )
            Text(
                text = "Not connected",
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
                    .background(Color(0xFFE3B35B)),
            )
            Text(
                text = "Ready",
                modifier = Modifier.padding(start = 8.dp),
                style = MaterialTheme.typography.labelLarge,
                color = Color(0xFFE3B35B),
            )
        }
    }
}

@Composable
private fun StatusPanel() {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surface)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        StatusRow(label = "Android shell", value = "Compose debug app")
        StatusRow(label = "Transport", value = "iroh dependency linked")
        StatusRow(label = "Remote protocol", value = "Not connected")
    }
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

@Preview
@Composable
private fun LiliaRemoteAppPreview() {
    LiliaRemoteApp()
}
